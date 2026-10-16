#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <setjmp.h>
#include <pthread.h>
#include <unistd.h>
#include <cmocka.h>

#include <obs.h>
#include <obs-module.h>
#include <util/base.h>
#include <util/platform.h>

#include <pulse/stream.h>

#include "pulse-reconnect.h"
#include "pulse-wrapper.h"

/* End to end for obsproject/obs-studio#10072 (reported by foresto): an Audio
 * Input Capture stays silent for good once its own stream dies while the
 * connection to the sound server stays up. The reporter switched to another
 * virtual console, which takes the device away from the session; when they
 * came back the device returned under the same name, but OBS never recorded
 * from it again. Unplugging and replugging a USB microphone, or the server
 * killing the stream, end the same way.
 *
 * Runs the linux-pulseaudio capture code against a real sound server: a
 * private PipeWire + WirePlumber + pipewire-pulse stack started by
 * test_pulse_monitor_device.sh (no hardware, its own runtime dir and D-Bus),
 * with virtual sources this test loads itself. libobs's source plumbing is
 * replaced by the stubs below (linked with --wrap), so no display is needed:
 * each test source counts the audio frames the plugin hands to OBS, and UI
 * tasks run on a thread of their own like the frontend's. Without that stack
 * the wrapper exits 77 and ctest reports a skip. */

#define MIC_A "obs_test_mic_a"
#define MIC_B "obs_test_mic_b"
#define MIC_LATE "obs_test_mic_late"
#define MIC_GONE "obs_test_mic_gone"
#define OTHER "obs_test_other"

/* 100 ms of audio at 48 kHz */
#define SOME_FRAMES 4800

extern struct obs_source_info pulse_input_capture;

struct test_source {
	const char *name;
	void *data;
	volatile bool destroyed;
	volatile uint64_t frames;
};

/* ------------------------------------------------------------------------ */
/* libobs stubs                                                             */

static volatile long weak_refs;

const char *obs_module_text(const char *lookup)
{
	return lookup;
}

const char *__wrap_obs_source_get_name(const obs_source_t *source)
{
	return ((const struct test_source *)source)->name;
}

void __wrap_obs_source_output_audio(obs_source_t *source, const struct obs_source_audio *audio)
{
	__atomic_add_fetch(&((struct test_source *)source)->frames, audio->frames, __ATOMIC_SEQ_CST);
}

void __wrap_obs_source_set_async_compensation(obs_source_t *source, bool enable)
{
	(void)source;
	(void)enable;
}

void __wrap_obs_source_audio_output_capture_device_changed(obs_source_t *source, const char *device)
{
	(void)source;
	(void)device;
}

obs_weak_source_t *__wrap_obs_source_get_weak_source(obs_source_t *source)
{
	__atomic_add_fetch(&weak_refs, 1, __ATOMIC_SEQ_CST);
	return (obs_weak_source_t *)source;
}

void __wrap_obs_weak_source_addref(obs_weak_source_t *weak)
{
	(void)weak;
	__atomic_add_fetch(&weak_refs, 1, __ATOMIC_SEQ_CST);
}

void __wrap_obs_weak_source_release(obs_weak_source_t *weak)
{
	(void)weak;
	__atomic_sub_fetch(&weak_refs, 1, __ATOMIC_SEQ_CST);
}

obs_source_t *__wrap_obs_weak_source_get_source(obs_weak_source_t *weak)
{
	struct test_source *source = (struct test_source *)weak;
	return source->destroyed ? NULL : (obs_source_t *)source;
}

void __wrap_obs_source_release(obs_source_t *source)
{
	(void)source;
}

void *__wrap_obs_obj_get_data(void *obj)
{
	return ((struct test_source *)obj)->data;
}

/* Stream restarts that fail on demand (follow-up of #378): the next
 * fail_stream_new streams cannot be created (pa_stream_new returning NULL, a
 * synchronous failure while the device is there), and while fail_connected
 * is set every record stream is ended on the pulse mainloop right after it
 * was connected, as by a server that keeps failing it. */
static volatile int fail_stream_new;
static volatile bool fail_connected;
static volatile int stream_new_calls, connect_calls;

/* When each record stream was connected while fail_connected was set (the
 * mainloop is locked then, so one writer at a time). */
#define MAX_CONNECT_TIMES 256
static uint64_t connect_times[MAX_CONNECT_TIMES];
static volatile int connect_times_num;

pa_stream *__real_pa_stream_new_with_proplist(pa_context *c, const char *name, const pa_sample_spec *ss,
					      const pa_channel_map *map, pa_proplist *p);
int __real_pa_stream_connect_record(pa_stream *s, const char *dev, const pa_buffer_attr *attr, pa_stream_flags_t flags);

pa_stream *__wrap_pa_stream_new_with_proplist(pa_context *c, const char *name, const pa_sample_spec *ss,
					      const pa_channel_map *map, pa_proplist *p)
{
	__atomic_add_fetch(&stream_new_calls, 1, __ATOMIC_SEQ_CST);
	int left = __atomic_load_n(&fail_stream_new, __ATOMIC_SEQ_CST);
	while (left > 0) {
		if (__atomic_compare_exchange_n(&fail_stream_new, &left, left - 1, false, __ATOMIC_SEQ_CST,
						__ATOMIC_SEQ_CST))
			return NULL;
	}
	return __real_pa_stream_new_with_proplist(c, name, ss, map, p);
}

static void unref_stream(void *userdata)
{
	pa_stream_unref(userdata);
}

/* On the pulse mainloop: end the stream once it is ready, unless OBS
 * already did. */
static void end_stream(void *userdata)
{
	pa_stream *s = userdata;
	pa_stream_state_t state = pa_stream_get_state(s);
	if (state == PA_STREAM_CREATING)
		pulse_call_later(end_stream, pa_stream_ref(s), unref_stream, 2000);
	else if (state == PA_STREAM_READY)
		pa_stream_disconnect(s);
}

/* Called with the mainloop locked. */
int __wrap_pa_stream_connect_record(pa_stream *s, const char *dev, const pa_buffer_attr *attr, pa_stream_flags_t flags)
{
	__atomic_add_fetch(&connect_calls, 1, __ATOMIC_SEQ_CST);
	int ret = __real_pa_stream_connect_record(s, dev, attr, flags);
	if (ret == 0 && __atomic_load_n(&fail_connected, __ATOMIC_SEQ_CST)) {
		int n = __atomic_load_n(&connect_times_num, __ATOMIC_SEQ_CST);
		if (n < MAX_CONNECT_TIMES) {
			connect_times[n] = os_gettime_ns();
			__atomic_store_n(&connect_times_num, n + 1, __ATOMIC_SEQ_CST);
		}
		pulse_call_later(end_stream, pa_stream_ref(s), unref_stream, 2000);
	}
	return ret;
}

static int calls(volatile int *counter)
{
	return __atomic_load_n(counter, __ATOMIC_SEQ_CST);
}

/* UI tasks run in order on one thread, as on the frontend's UI thread. */
struct ui_task {
	obs_task_t task;
	void *param;
	struct ui_task *next;
};

static pthread_mutex_t ui_mutex = PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t ui_cond = PTHREAD_COND_INITIALIZER;
static struct ui_task *ui_head;
static struct ui_task **ui_tail = &ui_head;
static bool ui_busy;
static bool ui_quit;
static pthread_t ui_thread;

void __wrap_obs_queue_task(enum obs_task_type type, obs_task_t task, void *param, bool wait)
{
	(void)wait;
	if (type != OBS_TASK_UI) {
		fprintf(stderr, "unexpected task type %d\n", (int)type);
		abort();
	}
	struct ui_task *t = calloc(1, sizeof(*t));
	t->task = task;
	t->param = param;
	pthread_mutex_lock(&ui_mutex);
	*ui_tail = t;
	ui_tail = &t->next;
	pthread_cond_broadcast(&ui_cond);
	pthread_mutex_unlock(&ui_mutex);
}

static void *ui_thread_main(void *unused)
{
	(void)unused;
	pthread_mutex_lock(&ui_mutex);
	for (;;) {
		while (!ui_head && !ui_quit)
			pthread_cond_wait(&ui_cond, &ui_mutex);
		if (!ui_head)
			break;
		struct ui_task *t = ui_head;
		ui_head = t->next;
		if (!ui_head)
			ui_tail = &ui_head;
		ui_busy = true;
		pthread_mutex_unlock(&ui_mutex);
		t->task(t->param);
		free(t);
		pthread_mutex_lock(&ui_mutex);
		ui_busy = false;
		pthread_cond_broadcast(&ui_cond);
	}
	pthread_mutex_unlock(&ui_mutex);
	return NULL;
}

/* Waits until every queued UI task (and any it queued) has run. */
static void drain_ui_tasks(void)
{
	pthread_mutex_lock(&ui_mutex);
	while (ui_head || ui_busy)
		pthread_cond_wait(&ui_cond, &ui_mutex);
	pthread_mutex_unlock(&ui_mutex);
}

/* ------------------------------------------------------------------------ */
/* Log: count stream starts per device                                      */

static pthread_mutex_t log_mutex = PTHREAD_MUTEX_INITIALIZER;
static int starts_a, starts_b, attempts_gone, gave_up;

static void test_log(int lvl, const char *fmt, va_list args, void *p)
{
	(void)p;
	char msg[1024];
	vsnprintf(msg, sizeof(msg), fmt, args);

	pthread_mutex_lock(&log_mutex);
	if (strstr(msg, "Started recording from '" MIC_A "'"))
		starts_a++;
	if (strstr(msg, "Started recording from '" MIC_B "'"))
		starts_b++;
	if (strstr(msg, "Unable to get source info") || strstr(msg, "error occurred while getting the source info"))
		attempts_gone++;
	if (strstr(msg, "Giving up restarting"))
		gave_up++;
	pthread_mutex_unlock(&log_mutex);

	if (lvl <= LOG_WARNING || getenv("TEST_VERBOSE"))
		fprintf(stderr, "%s\n", msg);
}

static int count(int *counter)
{
	pthread_mutex_lock(&log_mutex);
	int n = *counter;
	pthread_mutex_unlock(&log_mutex);
	return n;
}

/* ------------------------------------------------------------------------ */
/* Sound server helpers                                                     */

/* Runs a command and returns the first line of its output in buf. */
static bool run(const char *cmd, char *buf, size_t size)
{
	FILE *p = popen(cmd, "r");
	if (!p)
		return false;
	if (buf && size) {
		buf[0] = 0;
		if (!fgets(buf, (int)size, p))
			buf[0] = 0;
		buf[strcspn(buf, "\n")] = 0;
	}
	char rest[256];
	while (fgets(rest, sizeof(rest), p))
		;
	return pclose(p) == 0;
}

/* Modules and sources still loaded, so a failed case can clean up. */
#define MAX_TRACKED 16
static int loaded_modules[MAX_TRACKED];
static struct test_source *live_sources[MAX_TRACKED];

static void unload_module(int *module);

/* Loads a virtual microphone (an Audio/Source/Virtual null node). */
static int load_source(const char *name)
{
	char cmd[256], out[64];
	snprintf(cmd, sizeof(cmd),
		 "pactl load-module module-null-sink sink_name=%s media.class=Audio/Source/Virtual 2>/dev/null", name);
	if (!run(cmd, out, sizeof(out)) || !*out) {
		fprintf(stderr, "could not load virtual source %s\n", name);
		fail();
	}
	int module = atoi(out);
	for (size_t i = 0; i < MAX_TRACKED; i++) {
		if (loaded_modules[i] < 0) {
			loaded_modules[i] = module;
			break;
		}
	}
	return module;
}

static void unload_module(int *module)
{
	char cmd[128];
	if (*module < 0)
		return;
	snprintf(cmd, sizeof(cmd), "pactl unload-module %d 2>/dev/null", *module);
	run(cmd, NULL, 0);
	for (size_t i = 0; i < MAX_TRACKED; i++) {
		if (loaded_modules[i] == *module)
			loaded_modules[i] = -1;
	}
	*module = -1;
}

/* PipeWire node id of the record stream named stream_name, or -1. */
static int stream_node_id(const char *stream_name)
{
	FILE *p = popen("pactl list source-outputs 2>/dev/null", "r");
	if (!p)
		return -1;

	char line[512], want[256];
	int id = -1, found = -1;
	bool match = false;
	snprintf(want, sizeof(want), "media.name = \"%s\"", stream_name);
	while (fgets(line, sizeof(line), p)) {
		if (strncmp(line, "Source Output #", 15) == 0) {
			if (match && id >= 0)
				found = id;
			id = -1;
			match = false;
		}
		char *s = strstr(line, "object.id = \"");
		if (s)
			id = atoi(s + 13);
		if (strstr(line, want))
			match = true;
	}
	if (match && id >= 0)
		found = id;
	pclose(p);
	return found;
}

/* ------------------------------------------------------------------------ */
/* Sources                                                                  */

static struct test_source *create_capture(const char *name, const char *device)
{
	struct test_source *source = calloc(1, sizeof(*source));
	source->name = name;
	for (size_t i = 0; i < MAX_TRACKED; i++) {
		if (!live_sources[i]) {
			live_sources[i] = source;
			break;
		}
	}

	obs_data_t *settings = obs_data_create();
	obs_data_set_string(settings, "device_id", device);
	source->data = pulse_input_capture.create(settings, (obs_source_t *)source);
	obs_data_release(settings);
	return source;
}

/* Sources are leaked on purpose: queued tasks may still hold weak refs. */
static void destroy_capture(struct test_source *source)
{
	drain_ui_tasks();
	source->destroyed = true;
	pulse_input_capture.destroy(source->data);
	source->data = NULL;
	for (size_t i = 0; i < MAX_TRACKED; i++) {
		if (live_sources[i] == source)
			live_sources[i] = NULL;
	}
}

static uint64_t frames(struct test_source *source)
{
	return __atomic_load_n(&source->frames, __ATOMIC_SEQ_CST);
}

/* Waits up to timeout_ms for at least SOME_FRAMES new frames. */
static bool gets_audio(struct test_source *source, int timeout_ms)
{
	uint64_t start = frames(source);
	for (int t = 0; t < timeout_ms; t += 20) {
		if (frames(source) - start >= SOME_FRAMES)
			return true;
		usleep(20000);
	}
	return false;
}

static void expect_audio(struct test_source *source, int timeout_ms, const char *when)
{
	if (!gets_audio(source, timeout_ms)) {
		fprintf(stderr, "%s: '%s' produced no audio within %d ms\n", when, source->name, timeout_ms);
		fail();
	}
}

/* ------------------------------------------------------------------------ */
/* Cases                                                                    */

/* The reporter's case: the device goes away and comes back under the same
 * name (console switch, USB replug). */
static void replugged_source_resumes(void **state)
{
	(void)state;
	int mic_a = load_source(MIC_A);
	int mic_b = load_source(MIC_B);
	struct test_source *a = create_capture("capture A", MIC_A);
	struct test_source *b = create_capture("capture B", MIC_B);
	expect_audio(a, 3000, "after start");
	expect_audio(b, 3000, "after start");
	int b_starts = count(&starts_b);

	unload_module(&mic_a);
	usleep(500000);
	mic_a = load_source(MIC_A);

	/* Control: the other microphone is not touched at all. */
	expect_audio(b, 1000, "while the other device was replugged");
	expect_audio(a, 3000, "after its device was replugged");
	if (count(&starts_b) != b_starts) {
		fprintf(stderr, "the unaffected source restarted its stream (%d starts, expected %d)\n",
			count(&starts_b), b_starts);
		fail();
	}

	destroy_capture(a);
	destroy_capture(b);
	unload_module(&mic_a);
	unload_module(&mic_b);
}

/* The server kills OBS's stream; the device itself stays. */
static void killed_stream_restarts(void **state)
{
	(void)state;
	if (!run("command -v pw-cli", NULL, 0))
		skip();
	int mic_a = load_source(MIC_A);
	struct test_source *a = create_capture("capture A", MIC_A);
	expect_audio(a, 3000, "after start");

	int node = stream_node_id("capture A");
	if (node < 0) {
		fprintf(stderr, "could not find the stream of 'capture A'\n");
		fail();
	}
	char cmd[64];
	snprintf(cmd, sizeof(cmd), "pw-cli destroy %d >/dev/null 2>&1", node);
	assert_true(run(cmd, NULL, 0));
	usleep(300000);

	expect_audio(a, 3000, "after the server killed its stream");

	destroy_capture(a);
	unload_module(&mic_a);
}

/* A source whose device is not there when OBS starts records once it is
 * plugged in. */
static void late_source_starts(void **state)
{
	(void)state;
	struct test_source *late = create_capture("capture late", MIC_LATE);
	usleep(300000);
	int mic = load_source(MIC_LATE);

	expect_audio(late, 3000, "after its device was plugged in");

	destroy_capture(late);
	unload_module(&mic);
}

/* Control: a device that is deleted for good. The source must not keep
 * retrying (no busy loop, no log flood) and must not crash, also when other
 * devices come and go and when it is destroyed. */
static void deleted_source_stays_quiet(void **state)
{
	(void)state;
	int mic = load_source(MIC_GONE);
	struct test_source *gone = create_capture("capture gone", MIC_GONE);
	expect_audio(gone, 3000, "after start");

	unload_module(&mic);
	usleep(500000);
	drain_ui_tasks();
	int before = count(&attempts_gone);
	uint64_t frames_before = frames(gone);

	/* Unrelated devices come and go: each may cause at most one look. */
	for (int i = 0; i < 3; i++) {
		int other = load_source(OTHER);
		usleep(200000);
		unload_module(&other);
		usleep(200000);
	}
	usleep(1500000);
	drain_ui_tasks();

	int attempts = count(&attempts_gone) - before;
	if (attempts > 3) {
		fprintf(stderr, "a source whose device is gone tried %d times in 3 s\n", attempts);
		fail();
	}
	if (frames(gone) != frames_before) {
		fprintf(stderr, "a source whose device is gone produced audio\n");
		fail();
	}

	destroy_capture(gone);
}

/* Kills the record stream of source; the device stays. */
static void kill_stream(struct test_source *source)
{
	int node = stream_node_id(source->name);
	if (node < 0) {
		fprintf(stderr, "could not find the stream of '%s'\n", source->name);
		fail();
	}
	char cmd[64];
	snprintf(cmd, sizeof(cmd), "pw-cli destroy %d >/dev/null 2>&1", node);
	assert_true(run(cmd, NULL, 0));
}

/* The stream is killed and creating it again fails a few times while the
 * device is there and the server is connected. No device comes or goes, so
 * only a retry brings the audio back. */
static void failed_restart_is_retried(void **state)
{
	(void)state;
	if (!run("command -v pw-cli", NULL, 0))
		skip();
	int mic_a = load_source(MIC_A);
	struct test_source *a = create_capture("capture A", MIC_A);
	expect_audio(a, 3000, "after start");

	int before = calls(&stream_new_calls);
	__atomic_store_n(&fail_stream_new, 3, __ATOMIC_SEQ_CST);
	kill_stream(a);

	expect_audio(a, 3000, "after its stream was killed and could not be created 3 times");
	int tries = calls(&stream_new_calls) - before;
	if (tries != 4) {
		fprintf(stderr, "the stream was created %d times, expected 3 failures and 1 success\n", tries);
		fail();
	}

	destroy_capture(a);
	unload_module(&mic_a);
}

/* The server keeps failing the restarted stream while the device is there.
 * The source must not restart it in a tight loop, must give up after
 * PULSE_RESTART_MAX_ATTEMPTS, and must then start again once a device
 * appears. */
static void failing_stream_is_paced_then_given_up(void **state)
{
	(void)state;
	if (!run("command -v pw-cli", NULL, 0))
		skip();
	int mic_a = load_source(MIC_A);
	struct test_source *a = create_capture("capture A", MIC_A);
	expect_audio(a, 3000, "after start");

	int before = calls(&connect_calls);
	int gave_up_before = count(&gave_up);
	__atomic_store_n(&connect_times_num, 0, __ATOMIC_SEQ_CST);
	__atomic_store_n(&fail_connected, true, __ATOMIC_SEQ_CST);
	kill_stream(a);

	/* About one attempt per PULSE_RESTART_RETRY_USEC */
	usleep(1000000);
	int first_second = calls(&connect_calls) - before;
	if (first_second < 2) {
		fprintf(stderr, "the failing stream was restarted %d times in 1 s, expected about 10\n", first_second);
		fail();
	}
	/* Each restart after a failure waits PULSE_RESTART_RETRY_USEC; only
	 * the first one is at once, and it comes before the first gap. */
	int n = __atomic_load_n(&connect_times_num, __ATOMIC_SEQ_CST);
	uint64_t min_gap = UINT64_MAX;
	for (int i = 1; i < n; i++) {
		if (connect_times[i] - connect_times[i - 1] < min_gap)
			min_gap = connect_times[i] - connect_times[i - 1];
	}
	if (n < 2 || min_gap < PULSE_RESTART_RETRY_USEC * 1000 * 9 / 10) {
		fprintf(stderr,
			"the failing stream was restarted %d times in 1 s, %.0f ms apart at the least, expected %d ms\n",
			first_second, n < 2 ? 0.0 : min_gap / 1e6, PULSE_RESTART_RETRY_USEC / 1000);
		fail();
	}

	for (int t = 0; t < 15000 && count(&gave_up) == gave_up_before; t += 50)
		usleep(50000);
	if (count(&gave_up) == gave_up_before) {
		fprintf(stderr, "still restarting the failing stream after 16 s (%d attempts)\n",
			calls(&connect_calls) - before);
		fail();
	}
	usleep(500000);
	drain_ui_tasks();
	int total = calls(&connect_calls) - before;
	usleep(500000);
	drain_ui_tasks();
	if (calls(&connect_calls) - before != total) {
		fprintf(stderr, "the stream was restarted again after giving up\n");
		fail();
	}
	if (total > PULSE_RESTART_MAX_ATTEMPTS + 1) {
		fprintf(stderr, "%d restarts before giving up, expected at most %d\n", total,
			PULSE_RESTART_MAX_ATTEMPTS + 1);
		fail();
	}

	/* Given up, the source looks again when a device appears. */
	__atomic_store_n(&fail_connected, false, __ATOMIC_SEQ_CST);
	int other = load_source(OTHER);
	expect_audio(a, 3000, "after giving up and a device appearing");

	destroy_capture(a);
	unload_module(&other);
	unload_module(&mic_a);
}

/* Destroying a source while its restart is pending leaks nothing (the
 * group teardown checks the weak references). */
static void source_destroyed_while_retrying(void **state)
{
	(void)state;
	if (!run("command -v pw-cli", NULL, 0))
		skip();
	int mic_a = load_source(MIC_A);
	int mic_b = load_source(MIC_B);
	struct test_source *a = create_capture("capture A", MIC_A);
	/* Keeps the server connection up while A is destroyed. */
	struct test_source *b = create_capture("capture B", MIC_B);
	expect_audio(a, 3000, "after start");

	__atomic_store_n(&fail_stream_new, 1000, __ATOMIC_SEQ_CST);
	kill_stream(a);
	usleep(250000);
	destroy_capture(a);
	__atomic_store_n(&fail_stream_new, 0, __ATOMIC_SEQ_CST);
	usleep(300000);
	drain_ui_tasks();

	expect_audio(b, 1000, "after the other source was destroyed");
	destroy_capture(b);
	unload_module(&mic_a);
	unload_module(&mic_b);
}

/* ------------------------------------------------------------------------ */

/* Also runs after a failed case: destroy what it left behind. */
static int case_teardown(void **state)
{
	(void)state;
	__atomic_store_n(&fail_stream_new, 0, __ATOMIC_SEQ_CST);
	__atomic_store_n(&fail_connected, false, __ATOMIC_SEQ_CST);
	for (size_t i = 0; i < MAX_TRACKED; i++) {
		if (live_sources[i])
			destroy_capture(live_sources[i]);
	}
	for (size_t i = 0; i < MAX_TRACKED; i++) {
		int module = loaded_modules[i];
		unload_module(&module);
	}
	return 0;
}

static int setup(void **state)
{
	(void)state;
	for (size_t i = 0; i < MAX_TRACKED; i++)
		loaded_modules[i] = -1;
	base_set_log_handler(test_log, NULL);
	return pthread_create(&ui_thread, NULL, ui_thread_main, NULL) == 0 ? 0 : -1;
}

static int teardown(void **state)
{
	(void)state;
	drain_ui_tasks();
	pthread_mutex_lock(&ui_mutex);
	ui_quit = true;
	pthread_cond_broadcast(&ui_cond);
	pthread_mutex_unlock(&ui_mutex);
	pthread_join(ui_thread, NULL);

	long refs = __atomic_load_n(&weak_refs, __ATOMIC_SEQ_CST);
	if (refs != 0) {
		fprintf(stderr, "%ld weak source references leaked\n", refs);
		return -1;
	}
	return 0;
}

int main(void)
{
	if (!getenv("PULSE_SERVER")) {
		fprintf(stderr, "PULSE_SERVER is not set: run through test_pulse_monitor_device.sh\n");
		return 77;
	}

	const struct CMUnitTest tests[] = {
		cmocka_unit_test_teardown(replugged_source_resumes, case_teardown),
		cmocka_unit_test_teardown(killed_stream_restarts, case_teardown),
		cmocka_unit_test_teardown(late_source_starts, case_teardown),
		cmocka_unit_test_teardown(deleted_source_stays_quiet, case_teardown),
		cmocka_unit_test_teardown(failed_restart_is_retried, case_teardown),
		cmocka_unit_test_teardown(failing_stream_is_paced_then_given_up, case_teardown),
		cmocka_unit_test_teardown(source_destroyed_while_retrying, case_teardown),
	};

	return cmocka_run_group_tests(tests, setup, teardown);
}
