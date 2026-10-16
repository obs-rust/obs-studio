#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <setjmp.h>
#include <pthread.h>
#include <signal.h>
#include <unistd.h>
#include <dirent.h>
#include <fcntl.h>
#include <sys/prctl.h>
#include <sys/types.h>
#include <sys/wait.h>
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
 * the wrapper exits 77 and ctest reports a skip.
 *
 * Since #532 each stream has a server connection of its own besides the
 * shared one that pulse_init() opens. The server can drop just the stream's
 * connection (pw-cli destroy on its client here): the source must restart
 * like after a killed stream, and a lost shared connection or a server
 * restart must still restart each source once, through the reconnection. */

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
static int starts_a, starts_b, attempts_gone, gave_up, reconnects;

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
	if (strstr(msg, "Reconnected to the server"))
		reconnects++;
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

/* A numeric property (key = "N") of the record stream named stream_name in
 * `pactl list source-outputs`, or -1. */
static int stream_property(const char *stream_name, const char *key)
{
	FILE *p = popen("pactl list source-outputs 2>/dev/null", "r");
	if (!p)
		return -1;

	char line[512], want[256], prop[64];
	int id = -1, found = -1;
	bool match = false;
	snprintf(want, sizeof(want), "media.name = \"%s\"", stream_name);
	snprintf(prop, sizeof(prop), "%s = \"", key);
	while (fgets(line, sizeof(line), p)) {
		if (strncmp(line, "Source Output #", 15) == 0) {
			if (match && id >= 0)
				found = id;
			id = -1;
			match = false;
		}
		char *s = strstr(line, prop);
		if (s)
			id = atoi(s + strlen(prop));
		if (strstr(line, want))
			match = true;
	}
	if (match && id >= 0)
		found = id;
	pclose(p);
	return found;
}

/* PipeWire node id of the record stream named stream_name, or -1. */
static int stream_node_id(const char *stream_name)
{
	return stream_property(stream_name, "object.id");
}

/* PipeWire id of the client (the server connection) the record stream named
 * stream_name was created on, or -1. */
static int stream_client_id(const char *stream_name)
{
	return stream_property(stream_name, "client.id");
}

/* The PipeWire ids of this process's clients (server connections), and how
 * many there are. */
#define MAX_CLIENTS 16
static int own_clients(int *ids)
{
	FILE *p = popen("pactl list clients 2>/dev/null", "r");
	if (!p)
		return 0;

	char line[512], want[64];
	int n = 0, id = -1;
	bool mine = false;
	snprintf(want, sizeof(want), "application.process.id = \"%d\"", (int)getpid());
	while (fgets(line, sizeof(line), p)) {
		if (strncmp(line, "Client #", 8) == 0) {
			if (mine && id >= 0 && n < MAX_CLIENTS)
				ids[n++] = id;
			id = -1;
			mine = false;
		}
		char *s = strstr(line, "object.id = \"");
		if (s)
			id = atoi(s + 13);
		if (strstr(line, want))
			mine = true;
	}
	if (mine && id >= 0 && n < MAX_CLIENTS)
		ids[n++] = id;
	pclose(p);
	return n;
}

/* PipeWire id of the shared connection (the one pulse_init() opens, which
 * has no record stream), or -1. */
static int shared_client_id(const char **stream_names, size_t num)
{
	int ids[MAX_CLIENTS], found = -1;
	int n = own_clients(ids);
	for (int i = 0; i < n; i++) {
		bool has_stream = false;
		for (size_t j = 0; j < num; j++) {
			if (stream_client_id(stream_names[j]) == ids[i])
				has_stream = true;
		}
		if (!has_stream) {
			if (found >= 0)
				return -1;
			found = ids[i];
		}
	}
	return found;
}

/* The server drops a client connection (pw-cli destroy, as the sound server
 * does when it gives up on a client). */
static void drop_client(int id)
{
	char cmd[64];
	snprintf(cmd, sizeof(cmd), "pw-cli destroy %d >/dev/null 2>&1", id);
	assert_true(run(cmd, NULL, 0));
}

/* The pipewire-pulse that test_pulse_monitor_device.sh started (our sibling),
 * or the one this test started instead; 0 if none. */
static pid_t own_pulse_server;

static pid_t pulse_server_pid(void)
{
	if (own_pulse_server)
		return own_pulse_server;

	DIR *d = opendir("/proc");
	if (!d)
		return 0;
	pid_t found = 0;
	struct dirent *e;
	while (!found && (e = readdir(d))) {
		char path[300], buf[512];
		int pid = atoi(e->d_name);
		if (pid <= 0)
			continue;
		snprintf(path, sizeof(path), "/proc/%d/stat", pid);
		FILE *f = fopen(path, "r");
		if (!f)
			continue;
		size_t len = fread(buf, 1, sizeof(buf) - 1, f);
		fclose(f);
		buf[len] = 0;
		/* pid (comm) state ppid ... */
		char *close_paren = strrchr(buf, ')');
		char state;
		int ppid;
		if (!close_paren || sscanf(close_paren + 1, " %c %d", &state, &ppid) != 2)
			continue;
		if (ppid == getppid() && state != 'Z' && strstr(buf, "(pipewire-pulse)"))
			found = pid;
	}
	closedir(d);
	return found;
}

static bool process_gone(pid_t pid)
{
	char path[64], buf[512];
	snprintf(path, sizeof(path), "/proc/%d/stat", (int)pid);
	FILE *f = fopen(path, "r");
	if (!f)
		return true;
	size_t len = fread(buf, 1, sizeof(buf) - 1, f);
	fclose(f);
	buf[len] = 0;
	char *close_paren = strrchr(buf, ')');
	return !close_paren || close_paren[2] == 'Z';
}

static void stop_own_pulse_server(void)
{
	if (!own_pulse_server)
		return;
	kill(own_pulse_server, SIGTERM);
	waitpid(own_pulse_server, NULL, 0);
	own_pulse_server = 0;
}

/* Stops pipewire-pulse and starts a new one on the same socket, as when the
 * user restarts the sound server: every connection is lost, and the virtual
 * sources it held go with it. */
static void restart_pulse_server(void)
{
	pid_t old = pulse_server_pid();
	if (!old) {
		fprintf(stderr, "could not find pipewire-pulse\n");
		fail();
	}
	assert_int_equal(kill(old, SIGTERM), 0);
	if (old == own_pulse_server) {
		waitpid(old, NULL, 0);
		own_pulse_server = 0;
	}
	for (int t = 0; t < 5000 && !process_gone(old); t += 20)
		usleep(20000);
	assert_true(process_gone(old));
	for (size_t i = 0; i < MAX_TRACKED; i++)
		loaded_modules[i] = -1;

	pid_t pid = fork();
	assert_true(pid >= 0);
	if (pid == 0) {
		prctl(PR_SET_PDEATHSIG, SIGTERM);
		int null_fd = open("/dev/null", O_WRONLY);
		if (null_fd >= 0) {
			dup2(null_fd, STDOUT_FILENO);
			dup2(null_fd, STDERR_FILENO);
		}
		execlp("pipewire-pulse", "pipewire-pulse", (char *)NULL);
		_exit(127);
	}
	own_pulse_server = pid;
	for (int t = 0; t < 5000 && !run("pactl info >/dev/null 2>&1", NULL, 0); t += 100)
		usleep(100000);
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

/* How long one failed restart may take on top of PULSE_RESTART_RETRY_USEC
 * (stream creation, the server's reply, the UI task) on a busy host, and how
 * long the source may go without a restart before it counts as having
 * stopped without giving up. */
#define RESTART_SLACK_MS 300
#define RESTART_STALL_MS 5000

/* The server keeps failing the restarted stream while the device is there.
 * The source must not restart it in a tight loop, must give up after
 * PULSE_RESTART_MAX_ATTEMPTS, and must then start again once a device
 * appears. Judged by the restarts themselves, not by how long they took: a
 * busy host spreads them out, which is fine. */
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

	/* The killed stream is the first failure; each restart fails again,
	 * and the failure that reaches PULSE_RESTART_MAX_ATTEMPTS gives up. */
	const int expected = PULSE_RESTART_MAX_ATTEMPTS - 1;
	const uint64_t bound_ns =
		(uint64_t)PULSE_RESTART_MAX_ATTEMPTS * (PULSE_RESTART_RETRY_USEC / 1000 + RESTART_SLACK_MS) * 1000000;
	uint64_t start = os_gettime_ns(), last_change = start;
	int seen = 0;
	while (count(&gave_up) == gave_up_before) {
		int n = calls(&connect_calls) - before;
		uint64_t now = os_gettime_ns();
		if (n > expected) {
			fprintf(stderr, "the failing stream was restarted %d times without giving up, expected %d\n", n,
				expected);
			fail();
		}
		if (n != seen) {
			seen = n;
			last_change = now;
		} else if (now - last_change > (uint64_t)RESTART_STALL_MS * 1000000) {
			fprintf(stderr, "the failing stream stopped being restarted after %d times without giving up\n",
				n);
			fail();
		}
		if (now - start > bound_ns) {
			fprintf(stderr, "still restarting the failing stream after %.1f s (%d attempts, expected %d)\n",
				(now - start) / 1e9, n, expected);
			fail();
		}
		usleep(20000);
	}
	uint64_t took = os_gettime_ns() - start;

	usleep(500000);
	drain_ui_tasks();
	int total = calls(&connect_calls) - before;
	usleep(500000);
	drain_ui_tasks();
	if (calls(&connect_calls) - before != total) {
		fprintf(stderr, "the stream was restarted again after giving up\n");
		fail();
	}
	if (total != expected) {
		fprintf(stderr, "%d restarts before giving up, expected %d\n", total, expected);
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
			"the failing stream was restarted %d times, %.0f ms apart at the least, expected %d ms\n", n,
			n < 2 ? 0.0 : min_gap / 1e6, PULSE_RESTART_RETRY_USEC / 1000);
		fail();
	}
	fprintf(stderr, "gave up after %d restarts in %.1f s, %.0f ms apart at the least\n", total, took / 1e9,
		min_gap / 1e6);

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

/* The server drops the connection of one source's stream (follow-up of
 * #532, which gives each stream a connection of its own); the shared
 * connection and the device stay. That source must record again, and the
 * other one must not be touched. */
static void dropped_connection_restarts(void **state)
{
	(void)state;
	if (!run("command -v pw-cli", NULL, 0))
		skip();
	int mic_a = load_source(MIC_A);
	int mic_b = load_source(MIC_B);
	struct test_source *a = create_capture("capture A", MIC_A);
	struct test_source *b = create_capture("capture B", MIC_B);
	expect_audio(a, 3000, "after start");
	expect_audio(b, 3000, "after start");
	int b_starts = count(&starts_b);
	int reconnects_before = count(&reconnects);

	int client = stream_client_id("capture A");
	if (client < 0 || client == stream_client_id("capture B")) {
		fprintf(stderr, "could not find the connection of 'capture A' (%d)\n", client);
		fail();
	}
	drop_client(client);
	usleep(300000);

	expect_audio(b, 1000, "while the other source's connection was dropped");
	expect_audio(a, 3000, "after the server dropped its stream's connection");
	drain_ui_tasks();
	if (count(&starts_b) != b_starts || count(&reconnects) != reconnects_before) {
		fprintf(stderr, "the other source restarted (%d starts, expected %d; %d reconnects)\n",
			count(&starts_b), b_starts, count(&reconnects) - reconnects_before);
		fail();
	}

	destroy_capture(a);
	destroy_capture(b);
	unload_module(&mic_a);
	unload_module(&mic_b);
}

/* The stream's connection is dropped again and again. Each time the source
 * records again, never gives up (the counter resets once audio flows), and
 * the dropped connections are not leaked. */
static void repeated_connection_drops(void **state)
{
	(void)state;
	if (!run("command -v pw-cli", NULL, 0))
		skip();
	int mic_a = load_source(MIC_A);
	struct test_source *a = create_capture("capture A", MIC_A);
	expect_audio(a, 3000, "after start");
	int gave_up_before = count(&gave_up);
	int ids[MAX_CLIENTS];
	int clients_before = own_clients(ids);

	for (int i = 0; i < 5; i++) {
		int client = stream_client_id("capture A");
		if (client < 0) {
			fprintf(stderr, "drop %d: 'capture A' has no stream\n", i + 1);
			fail();
		}
		drop_client(client);
		usleep(100000);
		char when[64];
		snprintf(when, sizeof(when), "after its connection was dropped %d times", i + 1);
		expect_audio(a, 3000, when);
	}
	/* Twice in a row, before the restarted stream has delivered audio */
	drop_client(stream_client_id("capture A"));
	for (int t = 0; t < 3000 && stream_client_id("capture A") < 0; t += 20)
		usleep(20000);
	int client = stream_client_id("capture A");
	if (client >= 0)
		drop_client(client);
	expect_audio(a, 3000, "after two drops in a row");

	drain_ui_tasks();
	if (count(&gave_up) != gave_up_before) {
		fprintf(stderr, "the source gave up after repeated connection drops\n");
		fail();
	}
	int clients = own_clients(ids);
	if (clients != clients_before) {
		fprintf(stderr, "%d server connections after the drops, expected %d\n", clients, clients_before);
		fail();
	}

	destroy_capture(a);
	unload_module(&mic_a);
}

/* A source destroyed while the restart after a dropped connection is
 * pending (its stream cannot be created) leaks nothing; the group teardown
 * checks the weak references. */
static void source_destroyed_after_connection_drop(void **state)
{
	(void)state;
	if (!run("command -v pw-cli", NULL, 0))
		skip();
	int mic_a = load_source(MIC_A);
	int mic_b = load_source(MIC_B);
	struct test_source *a = create_capture("capture A", MIC_A);
	struct test_source *b = create_capture("capture B", MIC_B);
	expect_audio(a, 3000, "after start");
	int ids[MAX_CLIENTS];
	int clients_before = own_clients(ids);

	__atomic_store_n(&fail_stream_new, 1000, __ATOMIC_SEQ_CST);
	drop_client(stream_client_id("capture A"));
	usleep(250000);
	destroy_capture(a);
	__atomic_store_n(&fail_stream_new, 0, __ATOMIC_SEQ_CST);
	usleep(300000);
	drain_ui_tasks();

	expect_audio(b, 1000, "after the other source was destroyed");
	int clients = own_clients(ids);
	if (clients != clients_before - 1) {
		fprintf(stderr, "%d server connections after destroying A, expected %d\n", clients, clients_before - 1);
		fail();
	}
	destroy_capture(b);
	unload_module(&mic_a);
	unload_module(&mic_b);
}

/* Control: only the shared connection is dropped; the streams' own
 * connections stay. The reconnection restarts each source once. */
static void lost_shared_connection_restarts_once(void **state)
{
	(void)state;
	if (!run("command -v pw-cli", NULL, 0))
		skip();
	int mic_a = load_source(MIC_A);
	int mic_b = load_source(MIC_B);
	struct test_source *a = create_capture("capture A", MIC_A);
	struct test_source *b = create_capture("capture B", MIC_B);
	expect_audio(a, 3000, "after start");
	expect_audio(b, 3000, "after start");
	int a_starts = count(&starts_a), b_starts = count(&starts_b);
	int reconnects_before = count(&reconnects);

	const char *names[] = {"capture A", "capture B"};
	int shared = shared_client_id(names, 2);
	if (shared < 0) {
		fprintf(stderr, "could not find the shared connection\n");
		fail();
	}
	drop_client(shared);
	for (int t = 0; t < 5000 && count(&reconnects) == reconnects_before; t += 20)
		usleep(20000);
	if (count(&reconnects) == reconnects_before) {
		fprintf(stderr, "no reconnection after the shared connection was dropped\n");
		fail();
	}
	expect_audio(a, 3000, "after the shared connection was dropped");
	expect_audio(b, 3000, "after the shared connection was dropped");
	usleep(500000);
	drain_ui_tasks();
	if (count(&starts_a) - a_starts != 1 || count(&starts_b) - b_starts != 1) {
		fprintf(stderr, "restarts after the shared connection was dropped: A %d, B %d, expected 1 each\n",
			count(&starts_a) - a_starts, count(&starts_b) - b_starts);
		fail();
	}

	destroy_capture(a);
	destroy_capture(b);
	unload_module(&mic_a);
	unload_module(&mic_b);
}

/* Control: the sound server restarts. Every connection is lost at once, the
 * shared one and the streams' own; each source is restarted once, by the
 * reconnection, when its device is back. */
static void server_restart_restarts_once(void **state)
{
	(void)state;
	int mic_a = load_source(MIC_A);
	int mic_b = load_source(MIC_B);
	struct test_source *a = create_capture("capture A", MIC_A);
	struct test_source *b = create_capture("capture B", MIC_B);
	expect_audio(a, 3000, "after start");
	expect_audio(b, 3000, "after start");
	int a_starts = count(&starts_a), b_starts = count(&starts_b);
	int reconnects_before = count(&reconnects);

	restart_pulse_server();
	mic_a = load_source(MIC_A);
	mic_b = load_source(MIC_B);

	expect_audio(a, 5000, "after the server restarted");
	expect_audio(b, 5000, "after the server restarted");
	usleep(500000);
	drain_ui_tasks();
	if (count(&reconnects) - reconnects_before != 1) {
		fprintf(stderr, "%d reconnections after the server restarted, expected 1\n",
			count(&reconnects) - reconnects_before);
		fail();
	}
	if (count(&starts_a) - a_starts != 1 || count(&starts_b) - b_starts != 1) {
		fprintf(stderr, "restarts after the server restarted: A %d, B %d, expected 1 each\n",
			count(&starts_a) - a_starts, count(&starts_b) - b_starts);
		fail();
	}

	destroy_capture(a);
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
	stop_own_pulse_server();

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
		cmocka_unit_test_teardown(dropped_connection_restarts, case_teardown),
		cmocka_unit_test_teardown(repeated_connection_drops, case_teardown),
		cmocka_unit_test_teardown(source_destroyed_after_connection_drop, case_teardown),
		cmocka_unit_test_teardown(lost_shared_connection_restarts_once, case_teardown),
		cmocka_unit_test_teardown(server_restart_restarts_once, case_teardown),
	};

	return cmocka_run_group_tests(tests, setup, teardown);
}
