/* linux-alsa: an Audio Capture Device (ALSA) source must come back after its
 * USB device is unplugged and plugged in again (obs-rust/obs-studio#2924).
 *
 * Once the device is gone, snd_pcm_readi() fails with -ENODEV, which
 * snd_pcm_recover() cannot recover. The capture thread then called
 * snd_pcm_wait() and snd_pcm_readi() on the dead handle for as long as the
 * source existed: at full CPU (about 28,000 failing calls per second on a
 * real MiraBox), silent, and with no log line. The device that comes back is
 * a new PCM, so only a new snd_pcm_open() reaches it.
 *
 * The real capture code (plugins/linux-alsa/alsa-input.c) runs on ALSA's null
 * PCM, which needs no sound card. snd_pcm_open and snd_pcm_readi are wrapped
 * to act like a USB device: while "unplugged", every read on a handle opened
 * before the unplug and every open fail with -ENODEV; after the "replug", a
 * new open works and its reads deliver silence, one period per 5 ms.
 * libobs's source calls are stubbed (--wrap), so no display is needed. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <cmocka.h>

#include <dirent.h>
#include <errno.h>
#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <alsa/asoundlib.h>
#include <obs-module.h>
#include <util/base.h>
#include <util/platform.h>
#include <util/threading.h>

extern struct obs_source_info alsa_input_capture;

/* What linux-alsa.c's module boilerplate gives the capture code. */
const char *obs_module_text(const char *val)
{
	return val;
}

void __wrap_obs_source_set_async_compensation(obs_source_t *source, bool enable)
{
	(void)source;
	(void)enable;
}

/* ------------------------------------------------------------------------- */
/* the device */

static pthread_mutex_t device_mutex = PTHREAD_MUTEX_INITIALIZER;
static bool unplugged;
/* the handle the device answers on: the last one opened while plugged in */
static snd_pcm_t *live_pcm;
static long opens_failed;
static long reads_lost;
static volatile long audio_packets;

static void device_reset(void)
{
	pthread_mutex_lock(&device_mutex);
	unplugged = false;
	live_pcm = NULL;
	opens_failed = 0;
	reads_lost = 0;
	pthread_mutex_unlock(&device_mutex);
	os_atomic_set_long(&audio_packets, 0);
}

static void device_unplug(void)
{
	pthread_mutex_lock(&device_mutex);
	unplugged = true;
	live_pcm = NULL;
	pthread_mutex_unlock(&device_mutex);
}

static void device_replug(void)
{
	pthread_mutex_lock(&device_mutex);
	unplugged = false;
	pthread_mutex_unlock(&device_mutex);
}

static long device_reads_lost(void)
{
	pthread_mutex_lock(&device_mutex);
	long n = reads_lost;
	pthread_mutex_unlock(&device_mutex);
	return n;
}

static long device_opens_failed(void)
{
	pthread_mutex_lock(&device_mutex);
	long n = opens_failed;
	pthread_mutex_unlock(&device_mutex);
	return n;
}

int __real_snd_pcm_open(snd_pcm_t **pcm, const char *name, snd_pcm_stream_t stream, int mode);
int __wrap_snd_pcm_open(snd_pcm_t **pcm, const char *name, snd_pcm_stream_t stream, int mode)
{
	pthread_mutex_lock(&device_mutex);
	if (unplugged) {
		opens_failed++;
		pthread_mutex_unlock(&device_mutex);
		return -ENODEV;
	}
	int err = __real_snd_pcm_open(pcm, name, stream, mode);
	if (err == 0)
		live_pcm = *pcm;
	pthread_mutex_unlock(&device_mutex);
	return err;
}

snd_pcm_sframes_t __wrap_snd_pcm_readi(snd_pcm_t *pcm, void *buffer, snd_pcm_uframes_t size)
{
	pthread_mutex_lock(&device_mutex);
	bool lost = unplugged || pcm != live_pcm;
	if (lost)
		reads_lost++;
	pthread_mutex_unlock(&device_mutex);
	if (lost)
		return -ENODEV;

	snd_pcm_hw_params_t *params;
	snd_pcm_hw_params_alloca(&params);
	unsigned int channels = 2;
	snd_pcm_format_t format = SND_PCM_FORMAT_S16_LE;
	if (snd_pcm_hw_params_current(pcm, params) == 0) {
		snd_pcm_hw_params_get_channels(params, &channels);
		snd_pcm_hw_params_get_format(params, &format);
	}
	memset(buffer, 0, size * channels * (size_t)snd_pcm_format_physical_width(format) / 8);
	os_sleep_ms(5);
	return (snd_pcm_sframes_t)size;
}

void __wrap_obs_source_output_audio(obs_source_t *source, const struct obs_source_audio *audio)
{
	(void)source;
	(void)audio;
	os_atomic_inc_long(&audio_packets);
}

/* ------------------------------------------------------------------------- */
/* the log: one warning when the device goes, none for the attempts */

static pthread_mutex_t log_mutex = PTHREAD_MUTEX_INITIALIZER;
static long log_lines;

static void count_log(int level, const char *format, va_list args, void *param)
{
	char line[512];
	(void)param;
	vsnprintf(line, sizeof(line), format, args);
	if (level > LOG_WARNING || strncmp(line, "alsa-input: ", 12) != 0)
		return;
	pthread_mutex_lock(&log_mutex);
	log_lines++;
	pthread_mutex_unlock(&log_mutex);
}

static long warnings_logged(void)
{
	pthread_mutex_lock(&log_mutex);
	long n = log_lines;
	pthread_mutex_unlock(&log_mutex);
	return n;
}

/* ------------------------------------------------------------------------- */
/* helpers */

static int thread_count(void)
{
	DIR *dir = opendir("/proc/self/task");
	int count = 0;
	struct dirent *entry;

	if (!dir)
		return -1;
	while ((entry = readdir(dir)) != NULL) {
		if (entry->d_name[0] != '.')
			count++;
	}
	closedir(dir);
	return count;
}

/* A joined thread can stay listed in /proc/self/task for a moment after
 * pthread_join() returns: wait up to 500 ms for the count to settle. */
static int thread_count_settled(int expected)
{
	int count = thread_count();
	for (int i = 0; i < 50 && count != expected; i++) {
		os_sleep_ms(10);
		count = thread_count();
	}
	return count;
}

static void *create_source(unsigned int rate)
{
	obs_data_t *settings = obs_data_create();
	obs_data_set_string(settings, "device_id", "null");
	obs_data_set_int(settings, "rate", rate);
	void *data = alsa_input_capture.create(settings, NULL);
	obs_data_release(settings);
	return data;
}

static void update_rate(void *data, unsigned int rate)
{
	obs_data_t *settings = obs_data_create();
	obs_data_set_string(settings, "device_id", "null");
	obs_data_set_int(settings, "rate", rate);
	alsa_input_capture.update(data, settings);
	obs_data_release(settings);
}

/* Waits up to timeout_ms for audio packets beyond after; true if any came. */
static bool wait_audio_after(long after, int timeout_ms)
{
	for (int t = 0; t < timeout_ms / 10; t++) {
		if (os_atomic_load_long(&audio_packets) > after)
			return true;
		os_sleep_ms(10);
	}
	return os_atomic_load_long(&audio_packets) > after;
}

struct destroy_call {
	void *data;
	volatile bool done;
};

static void *destroy_thread(void *arg)
{
	struct destroy_call *call = arg;
	alsa_input_capture.destroy(call->data);
	os_atomic_set_bool(&call->done, true);
	return NULL;
}

/* Destroys the source and returns how long it took, or -1 when destroy did
 * not return within 2 s (the helper thread is then left behind). */
static long destroy_with_deadline(void *data)
{
	/* Left to leak when destroy does not return. */
	struct destroy_call *call = calloc(1, sizeof(*call));
	pthread_t thread;
	uint64_t start = os_gettime_ns();

	call->data = data;
	if (pthread_create(&thread, NULL, destroy_thread, call) != 0)
		return -1;
	while (!os_atomic_load_bool(&call->done)) {
		if (os_gettime_ns() - start > 2000000000ULL) {
			pthread_detach(thread);
			return -1;
		}
		os_sleep_ms(1);
	}
	long elapsed_ms = (long)((os_gettime_ns() - start) / 1000000);
	pthread_join(thread, NULL);
	free(call);
	return elapsed_ms;
}

static void assert_destroy_returned(long elapsed_ms)
{
	if (elapsed_ms < 0)
		fail_msg("destroy did not return within 2 s");
	if (elapsed_ms >= 500)
		fail_msg("destroy took %ld ms", elapsed_ms);
}

/* Captures until OBS gets audio (the capture code holds audio back for
 * 500 ms after the start). */
static void *start_capturing(void)
{
	device_reset();
	void *data = create_source(48000);
	assert_non_null(data);
	if (!wait_audio_after(0, 3000))
		fail_msg("no audio reached OBS within 3 s of the start");
	return data;
}

/* ------------------------------------------------------------------------- */

/* The report: unplug, plug in again, the audio comes back by itself. */
static void audio_comes_back_after_replug(void **state)
{
	(void)state;
	void *data = start_capturing();

	device_unplug();
	os_sleep_ms(300);
	device_replug();
	long at_replug = os_atomic_load_long(&audio_packets);
	bool back = wait_audio_after(at_replug, 6000);

	long elapsed_ms = destroy_with_deadline(data);
	assert_destroy_returned(elapsed_ms);
	if (!back)
		fail_msg("no audio within 6 s of the replug (%ld reads failed on the old handle)", device_reads_lost());
}

/* While the device is away the capture thread waits: it does not spin on the
 * dead handle, and it tries to open the device only now and then. */
static void no_busy_loop_while_away(void **state)
{
	(void)state;
	void *data = start_capturing();

	device_unplug();
	os_sleep_ms(2500);
	long reads = device_reads_lost();
	long opens = device_opens_failed();

	long elapsed_ms = destroy_with_deadline(data);
	assert_destroy_returned(elapsed_ms);
	if (reads > 10)
		fail_msg("%ld reads on the dead handle in 2.5 s: the capture thread spins", reads);
	if (opens > 5)
		fail_msg("%ld opens in 2.5 s: the retries do not back off", opens);
}

/* One warning when the device goes, however many attempts follow before it
 * captures again (the attempts and the recovery log below LOG_WARNING). */
static void one_line_each_way(void **state)
{
	(void)state;
	base_set_log_handler(count_log, NULL);
	void *data = start_capturing();
	long before = warnings_logged();

	device_unplug();
	os_sleep_ms(3500);
	device_replug();
	bool back = wait_audio_after(os_atomic_load_long(&audio_packets), 6000);
	long during = warnings_logged() - before;

	long elapsed_ms = destroy_with_deadline(data);
	base_set_log_handler(NULL, NULL);
	assert_destroy_returned(elapsed_ms);
	assert_true(back);
	if (during != 1)
		fail_msg("%ld alsa-input lines at LOG_WARNING or above while the device was away and back, "
			 "expected 1 (the loss)",
			 during);
}

/* Destroying the source while the device is away returns at once and leaves
 * no thread behind. */
static void destroy_while_away(void **state)
{
	(void)state;
	int before = thread_count();
	void *data = start_capturing();

	device_unplug();
	os_sleep_ms(200);
	long elapsed_ms = destroy_with_deadline(data);

	assert_destroy_returned(elapsed_ms);
	assert_int_equal(thread_count_settled(before), before);
}

/* Changing the settings while the device is away leaves one capture or
 * reopen thread, not two, and the audio still comes back. */
static void update_while_away(void **state)
{
	(void)state;
	int before = thread_count();
	void *data = start_capturing();

	device_unplug();
	os_sleep_ms(200);
	update_rate(data, 44100);
	os_sleep_ms(200);
	int during = thread_count();
	device_replug();
	bool back = wait_audio_after(os_atomic_load_long(&audio_packets), 6000);

	long elapsed_ms = destroy_with_deadline(data);
	assert_destroy_returned(elapsed_ms);
	assert_true(back);
	assert_int_equal(during, before + 1);
	assert_int_equal(thread_count_settled(before), before);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(audio_comes_back_after_replug),
		cmocka_unit_test(no_busy_loop_while_away),
		cmocka_unit_test(one_line_each_way),
		cmocka_unit_test(destroy_while_away),
		cmocka_unit_test(update_while_away),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
