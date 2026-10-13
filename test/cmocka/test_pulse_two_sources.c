#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <inttypes.h>
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

/* Two Audio Input Capture sources on the same device, with the PipeWire graph
 * running at a quantum of 2048 or more (an app asked for a large latency):
 * the second source was mostly silent in recordings (56-79 % of a 30 s
 * recording on the obs-rust release; 85-96 % on stock OBS 32.2.2), with
 * "exceeded TS_SMOOTHING_THRESHOLD" lines. #194 fixed how packets that come
 * in bursts are timed, but this was left.
 *
 * The cause is the delivery: every source's record stream was created on
 * linux-pulseaudio's one shared connection, and pipewire-pulse delivers the
 * second record stream of a connection unevenly at such quanta: nothing for
 * seconds at a time (measured up to 6.9 s), then everything at once. OBS can
 * buffer at most about a second of audio, so most of it is too late. Two
 * streams on two connections are delivered evenly.
 *
 * Runs the linux-pulseaudio capture code against a private PipeWire +
 * WirePlumber + pipewire-pulse stack started by test_pulse_monitor_device.sh
 * (no hardware), with libobs's source plumbing stubbed (--wrap) like
 * test_pulse_stream_restart. Without that stack the wrapper exits 77. */

#define DEVICE "obs_test_two_sources"
#define MONITOR DEVICE ".monitor"
#define NAME_A "capture A"
#define NAME_B "capture B"

/* 100 ms of audio at 48 kHz */
#define SOME_FRAMES 4800

extern struct obs_source_info pulse_input_capture;

struct test_source {
	const char *name;
	void *data;
	volatile bool destroyed;
	volatile uint64_t frames;
	/* Arrival of the last packet, and the longest wait between two
	 * packets once measure_from_ns is set. */
	volatile uint64_t last_ns;
	volatile uint64_t max_gap_ns;
};

static volatile uint64_t measure_from_ns;

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

/* Called on the PulseAudio mainloop thread, one packet at a time. */
void __wrap_obs_source_output_audio(obs_source_t *source, const struct obs_source_audio *audio)
{
	struct test_source *s = (struct test_source *)source;
	uint64_t now = os_gettime_ns();
	uint64_t from = __atomic_load_n(&measure_from_ns, __ATOMIC_SEQ_CST);

	if (from && s->last_ns >= from && now - s->last_ns > s->max_gap_ns)
		s->max_gap_ns = now - s->last_ns;
	s->last_ns = now;
	__atomic_add_fetch(&s->frames, audio->frames, __ATOMIC_SEQ_CST);
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
/* Log                                                                      */

static void test_log(int lvl, const char *fmt, va_list args, void *p)
{
	(void)p;
	char msg[1024];
	vsnprintf(msg, sizeof(msg), fmt, args);
	if (lvl <= LOG_WARNING || getenv("TEST_VERBOSE"))
		fprintf(stderr, "%s\n", msg);
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

/* Loads a null sink; its monitor is the device both captures record. */
static int load_sink(const char *name)
{
	char cmd[256], out[64];
	snprintf(cmd, sizeof(cmd), "pactl load-module module-null-sink sink_name=%s rate=48000 channels=2 2>/dev/null",
		 name);
	if (!run(cmd, out, sizeof(out)) || !*out) {
		fprintf(stderr, "could not load the null sink %s\n", name);
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

/* Index of the client (connection) that owns the record stream named
 * stream_name, or -1. */
static int stream_client(const char *stream_name)
{
	FILE *p = popen("pactl list source-outputs 2>/dev/null", "r");
	if (!p)
		return -1;

	char line[512], want[256];
	int client = -1, found = -1;
	bool match = false;
	snprintf(want, sizeof(want), "media.name = \"%s\"", stream_name);
	while (fgets(line, sizeof(line), p)) {
		if (strncmp(line, "Source Output #", 15) == 0) {
			if (match)
				found = client;
			client = -1;
			match = false;
		}
		char *s = strstr(line, "Client: ");
		if (s)
			client = atoi(s + 8);
		if (strstr(line, want))
			match = true;
	}
	if (match)
		found = client;
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

static bool set_quantum(int quantum)
{
	char cmd[128];
	snprintf(cmd, sizeof(cmd), "pw-metadata -n settings 0 clock.force-quantum %d >/dev/null 2>&1", quantum);
	return run(cmd, NULL, 0);
}

/* ------------------------------------------------------------------------ */
/* Cases                                                                    */

/* Each capture records over a connection of its own. */
static void captures_use_separate_connections(void **state)
{
	(void)state;
	int sink = load_sink(DEVICE);
	struct test_source *a = create_capture(NAME_A, MONITOR);
	struct test_source *b = create_capture(NAME_B, MONITOR);
	expect_audio(a, 3000, "after start");
	expect_audio(b, 3000, "after start");

	int client_a = stream_client(NAME_A);
	int client_b = stream_client(NAME_B);
	fprintf(stderr, "client of '%s': %d, of '%s': %d\n", NAME_A, client_a, NAME_B, client_b);
	assert_true(client_a >= 0 && client_b >= 0);
	if (client_a == client_b) {
		fprintf(stderr, "both captures record over the same connection\n");
		fail();
	}

	destroy_capture(b);
	destroy_capture(a);
	unload_module(&sink);
}

/* The reported case: quantum 2048, both captures on one monitor, 10 s. */
static void second_capture_is_not_held_back(void **state)
{
	(void)state;
	if (!set_quantum(2048)) {
		fprintf(stderr, "pw-metadata could not force the quantum: skipping\n");
		skip();
	}
	int sink = load_sink(DEVICE);
	struct test_source *a = create_capture(NAME_A, MONITOR);
	struct test_source *b = create_capture(NAME_B, MONITOR);
	expect_audio(a, 3000, "after start");
	expect_audio(b, 3000, "after start");

	__atomic_store_n(&measure_from_ns, os_gettime_ns(), __ATOMIC_SEQ_CST);
	uint64_t frames_a = frames(a), frames_b = frames(b);
	usleep(10 * 1000000);
	frames_a = frames(a) - frames_a;
	frames_b = frames(b) - frames_b;

	uint64_t gap_a = __atomic_load_n(&a->max_gap_ns, __ATOMIC_SEQ_CST);
	uint64_t gap_b = __atomic_load_n(&b->max_gap_ns, __ATOMIC_SEQ_CST);
	fprintf(stderr,
		"in 10 s: '%s' %" PRIu64 " frames, longest wait %" PRIu64 " ms; '%s' %" PRIu64
		" frames, longest wait %" PRIu64 " ms\n",
		NAME_A, frames_a, gap_a / 1000000, NAME_B, frames_b, gap_b / 1000000);

	destroy_capture(b);
	destroy_capture(a);
	unload_module(&sink);
	set_quantum(0);

	/* A 2048-frame cycle is 43 ms; packets are 25 ms. Half a second
	 * without a packet is OBS's audio running out. */
	assert_true(gap_a < 500 * 1000000ULL);
	if (gap_b >= 500 * 1000000ULL) {
		fprintf(stderr, "the second capture got no audio for %" PRIu64 " ms\n", gap_b / 1000000);
		fail();
	}
}

/* ------------------------------------------------------------------------ */

/* Also runs after a failed case: destroy what it left behind. */
static int case_teardown(void **state)
{
	(void)state;
	for (size_t i = 0; i < MAX_TRACKED; i++) {
		if (live_sources[i])
			destroy_capture(live_sources[i]);
	}
	for (size_t i = 0; i < MAX_TRACKED; i++) {
		int module = loaded_modules[i];
		unload_module(&module);
	}
	set_quantum(0);
	__atomic_store_n(&measure_from_ns, 0, __ATOMIC_SEQ_CST);
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
	return 0;
}

int main(void)
{
	if (!getenv("PULSE_SERVER")) {
		fprintf(stderr, "PULSE_SERVER is not set: run through test_pulse_monitor_device.sh\n");
		return 77;
	}

	const struct CMUnitTest tests[] = {
		cmocka_unit_test_teardown(captures_use_separate_connections, case_teardown),
		cmocka_unit_test_teardown(second_capture_is_not_held_back, case_teardown),
	};

	return cmocka_run_group_tests(tests, setup, teardown);
}
