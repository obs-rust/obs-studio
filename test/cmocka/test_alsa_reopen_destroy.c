/* linux-alsa: an Audio Capture Device (ALSA) source whose device cannot be
 * opened (a USB microphone that is unplugged) keeps a reopen thread that
 * retries every second. Destroying the source (removing it, switching scene
 * collections, quitting OBS) freed the source's data and its abort event
 * while that thread was still waiting on them, and OBS crashed about a
 * second later in _alsa_reopen() -> os_event_timedwait() (residue of
 * obsproject/obs-studio#9322; crash reported in obsproject/obs-studio#13480).
 *
 * The real capture code (plugins/linux-alsa/alsa-input.c) is run on an ALSA
 * device name that does not exist; libobs's source calls are stubbed
 * (--wrap), so no display and no sound card are needed. The threads of the
 * process are counted in /proc/self/task. destroy runs on a helper thread
 * with a deadline: with libobs's C os_event (glibc) destroying the event under
 * the waiting reopen thread does not return at all. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <cmocka.h>

#include <dirent.h>
#include <pthread.h>
#include <stdlib.h>
#include <obs-module.h>
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

void __wrap_obs_source_output_audio(obs_source_t *source, const struct obs_source_audio *audio)
{
	(void)source;
	(void)audio;
}

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

static void *create_on_missing_device(void)
{
	obs_data_t *settings = obs_data_create();
	obs_data_set_string(settings, "device_id", "hw:CARD=ObsTestNoSuchCard");
	obs_data_set_int(settings, "rate", 48000);
	void *data = alsa_input_capture.create(settings, NULL);
	obs_data_release(settings);
	return data;
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

/* A source on a missing device has a reopen thread; destroying the source
 * ends it, without waiting for its one-second retry. */
static void destroy_ends_reopen_thread(void **state)
{
	(void)state;
	int before = thread_count();

	void *data = create_on_missing_device();
	assert_non_null(data);
	os_sleep_ms(100);
	assert_int_equal(thread_count(), before + 1);

	long elapsed_ms = destroy_with_deadline(data);

	assert_destroy_returned(elapsed_ms);
	assert_int_equal(thread_count_settled(before), before);
}

/* Destroying right after creating, before the reopen thread has started
 * waiting: the thread still ends at once, every time. */
static void destroy_right_after_create(void **state)
{
	(void)state;
	int before = thread_count();

	for (int i = 0; i < 20; i++) {
		void *data = create_on_missing_device();
		assert_non_null(data);

		long elapsed_ms = destroy_with_deadline(data);

		assert_destroy_returned(elapsed_ms);
		assert_int_equal(thread_count_settled(before), before);
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(destroy_ends_reopen_thread),
		cmocka_unit_test(destroy_right_after_create),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
