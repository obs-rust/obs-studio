#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <setjmp.h>
#include <errno.h>
#include <fcntl.h>
#include <pthread.h>
#include <signal.h>
#include <unistd.h>
#include <cmocka.h>

#include <obs.h>
#include <obs-module.h>
#include <util/base.h>
#include <util/platform.h>
#include <util/threading.h>

/* End to end for obs-rust/obs-studio#2260 (upstream obsproject/obs-studio
 * #4828, #3211, #2316): an Audio Output Capture set to "Default" must record
 * the monitor of whatever the default output is, and never the microphone.
 * On PulseAudio it kept recording the old output's monitor after the default
 * output changed (silence: the applications' playback had moved to the new
 * one), and when the default output was unplugged the server moved it to
 * the default *source*, the microphone, and kept it there.
 *
 * Loads the real linux-pulseaudio module into the real libobs and runs it
 * against a private sound server: a PulseAudio daemon started by
 * test_pulse_pulseaudio.sh, or pipewire-pulse started by
 * test_pulse_monitor_device.sh (it already followed the default there). No
 * audio reaches a real device: the outputs are null sinks this test loads.
 *
 * Each sample tells where it came from. The "desktop" is a pacat playback
 * stream on the default output that plays a constant +0.25, and moves with
 * the default like any application's playback. The "microphone" is a pipe
 * source, the default source, fed a constant -0.5. A capture that records
 * the microphone, even mixed with the desktop, gets negative samples; one
 * left on an output without the desktop gets silence. Which source the
 * capture's stream records from is read back with pactl. */

#define SINK_A "obs_test_a"
#define SINK_B "obs_test_b"
#define SINK_C "obs_test_c"
#define SINK_D "obs_test_d"
#define MIC "obs_test_mic"
#define CAPTURE "obs_test_capture"

#define DESKTOP_LEVEL 0.25f
#define MIC_LEVEL -0.5f

/* How long the capture may take to follow the default. */
#define FOLLOW_TIMEOUT_MS 5000

static bool obs_up;
static bool module_up;
static char fifo_path[512];

/* ---- UI task thread, as the frontend's main thread: the plugin restarts
 * its streams from OBS_TASK_UI tasks. ---- */

struct ui_task {
	obs_task_t task;
	void *param;
	os_event_t *done;
	struct ui_task *next;
};

static pthread_mutex_t ui_mutex = PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t ui_cond = PTHREAD_COND_INITIALIZER;
static struct ui_task *ui_head, *ui_tail;
static bool ui_quit;
static bool ui_running_task;
static pthread_t ui_thread;
static __thread bool on_ui_thread;

static void *ui_main(void *param)
{
	(void)param;
	on_ui_thread = true;
	pthread_mutex_lock(&ui_mutex);
	for (;;) {
		while (!ui_head && !ui_quit)
			pthread_cond_wait(&ui_cond, &ui_mutex);
		struct ui_task *t = ui_head;
		if (!t)
			break;
		ui_head = t->next;
		if (!ui_head)
			ui_tail = NULL;
		ui_running_task = true;
		pthread_mutex_unlock(&ui_mutex);

		t->task(t->param);
		if (t->done)
			os_event_signal(t->done);
		else
			free(t);

		pthread_mutex_lock(&ui_mutex);
		ui_running_task = false;
		pthread_cond_broadcast(&ui_cond);
	}
	pthread_mutex_unlock(&ui_mutex);
	return NULL;
}

static void ui_task_handler(obs_task_t task, void *param, bool wait)
{
	if (wait && on_ui_thread) {
		task(param);
		return;
	}

	struct ui_task *t = calloc(1, sizeof(*t));
	t->task = task;
	t->param = param;
	if (wait)
		os_event_init(&t->done, OS_EVENT_TYPE_MANUAL);

	pthread_mutex_lock(&ui_mutex);
	if (ui_tail)
		ui_tail->next = t;
	else
		ui_head = t;
	ui_tail = t;
	pthread_cond_broadcast(&ui_cond);
	pthread_mutex_unlock(&ui_mutex);

	if (wait) {
		os_event_wait(t->done);
		os_event_destroy(t->done);
		free(t);
	}
}

/* Waits until the UI thread has nothing left to do. */
static void ui_drain(void)
{
	pthread_mutex_lock(&ui_mutex);
	while (ui_head || ui_running_task)
		pthread_cond_wait(&ui_cond, &ui_mutex);
	pthread_mutex_unlock(&ui_mutex);
}

/* ---- pactl ---- */

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

static void pactl(const char *fmt, ...)
{
	char args[512], cmd[600];
	va_list ap;

	va_start(ap, fmt);
	vsnprintf(args, sizeof(args), fmt, ap);
	va_end(ap);
	snprintf(cmd, sizeof(cmd), "LC_ALL=C pactl %s >/dev/null 2>&1", args);
	if (!run(cmd, NULL, 0)) {
		fprintf(stderr, "pactl %s failed\n", args);
		fail();
	}
}

static int load_module(const char *fmt, ...)
{
	char args[512], cmd[600], out[64];
	va_list ap;

	va_start(ap, fmt);
	vsnprintf(args, sizeof(args), fmt, ap);
	va_end(ap);
	snprintf(cmd, sizeof(cmd), "LC_ALL=C pactl load-module %s 2>/dev/null", args);
	if (!run(cmd, out, sizeof(out)) || !*out) {
		fprintf(stderr, "pactl load-module %s failed\n", args);
		fail();
	}
	return atoi(out);
}

static void unload_module(int *module)
{
	char cmd[128];

	if (*module < 0)
		return;
	snprintf(cmd, sizeof(cmd), "LC_ALL=C pactl unload-module %d >/dev/null 2>&1", *module);
	run(cmd, NULL, 0);
	*module = -1;
}

static int load_sink(const char *name)
{
	return load_module("module-null-sink sink_name=%s sink_properties=device.description=%s", name, name);
}

/* The server's default sink, or "" */
static void default_sink(char *name, size_t size)
{
	char line[256];
	FILE *p = popen("LC_ALL=C pactl info 2>/dev/null", "r");

	name[0] = 0;
	if (!p)
		return;
	while (fgets(line, sizeof(line), p)) {
		if (strncmp(line, "Default Sink: ", 14) == 0) {
			line[strcspn(line, "\n")] = 0;
			snprintf(name, size, "%s", line + 14);
		}
	}
	pclose(p);
}

/* Name of the source that the stream named `stream` records from, "" if
 * there is no such stream, or "(unlinked)" if it records from nothing. */
static void stream_source(const char *stream, char *name, size_t size)
{
	char line[512], media[256];
	long source = -1, found = -2;
	FILE *p = popen("LC_ALL=C pactl list source-outputs 2>/dev/null", "r");

	name[0] = 0;
	if (!p)
		return;
	while (fgets(line, sizeof(line), p)) {
		const char *l = line + strspn(line, " \t");
		if (strncmp(line, "Source Output #", 15) == 0)
			source = -1;
		else if (strncmp(l, "Source: ", 8) == 0)
			source = l[8] >= '0' && l[8] <= '9' ? strtol(l + 8, NULL, 10) : -1;
		else if (sscanf(line, " media.name = \"%255[^\"]\"", media) == 1 && strcmp(media, stream) == 0)
			found = source;
	}
	pclose(p);

	if (found == -2)
		return;
	if (found < 0) {
		snprintf(name, size, "(unlinked)");
		return;
	}

	p = popen("LC_ALL=C pactl list short sources 2>/dev/null", "r");
	if (!p)
		return;
	while (fgets(line, sizeof(line), p)) {
		long idx;
		char source_name[256];
		if (sscanf(line, "%ld %255s", &idx, source_name) == 2 && idx == found) {
			snprintf(name, size, "%s", source_name);
			break;
		}
	}
	pclose(p);
}

/* ---- signals ---- */

/* The desktop: an application's playback on the default output. */
static FILE *player;
static pthread_t player_thread;
static volatile bool player_quit;

static void *player_main(void *param)
{
	(void)param;
	float buf[480];
	for (size_t i = 0; i < 480; i++)
		buf[i] = DESKTOP_LEVEL;
	/* pacat paces the writes: it reads as fast as it plays. */
	while (!player_quit && fwrite(buf, sizeof(buf), 1, player) == 1)
		;
	return NULL;
}

static void start_player(void)
{
	player_quit = false;
	player = popen("exec pacat --playback --raw --format=float32le --rate=48000 --channels=1 "
		       "--latency-msec=40 --client-name=obs-test-desktop --stream-name=obs-test-desktop",
		       "w");
	assert_non_null(player);
	pthread_create(&player_thread, NULL, player_main, NULL);
}

static void stop_player(void)
{
	if (!player)
		return;
	player_quit = true;
	pthread_join(player_thread, NULL);
	pclose(player);
	player = NULL;
}

/* The microphone: a pipe source fed in real time. */
static int mic_fd = -1;
static pthread_t mic_thread;
static volatile bool mic_quit;

static void *mic_main(void *param)
{
	(void)param;
	float buf[480];
	uint64_t t = os_gettime_ns();

	for (size_t i = 0; i < 480; i++)
		buf[i] = MIC_LEVEL;
	while (!mic_quit) {
		/* Nobody may be reading: drop what does not fit. */
		if (write(mic_fd, buf, sizeof(buf)) < 0 && errno != EAGAIN)
			break;
		t += 10000000;
		os_sleepto_ns(t);
	}
	return NULL;
}

static void start_mic(void)
{
	mic_quit = false;
	mic_fd = open(fifo_path, O_WRONLY | O_NONBLOCK);
	if (mic_fd < 0) {
		fprintf(stderr, "could not open the microphone's pipe %s\n", fifo_path);
		fail();
	}
	pthread_create(&mic_thread, NULL, mic_main, NULL);
}

static void stop_mic(void)
{
	if (mic_fd < 0)
		return;
	mic_quit = true;
	pthread_join(mic_thread, NULL);
	close(mic_fd);
	mic_fd = -1;
}

/* ---- what the capture records ---- */

static pthread_mutex_t audio_mutex = PTHREAD_MUTEX_INITIALIZER;
static uint64_t frames_desktop, frames_mic, frames_quiet, frames_other;
static uint64_t mic_total;

static void on_audio(void *param, obs_source_t *source, const struct audio_data *audio, bool muted)
{
	(void)param;
	(void)source;
	(void)muted;
	const float *d = (const float *)audio->data[0];

	pthread_mutex_lock(&audio_mutex);
	for (uint32_t i = 0; i < audio->frames; i++) {
		float v = d[i];
		if (v < -0.1f) {
			frames_mic++;
			mic_total++;
		} else if (v > DESKTOP_LEVEL - 0.08f && v < DESKTOP_LEVEL + 0.08f) {
			frames_desktop++;
		} else if (v > -0.02f && v < 0.02f) {
			frames_quiet++;
		} else {
			frames_other++;
		}
	}
	pthread_mutex_unlock(&audio_mutex);
}

enum heard { HEARD_NOTHING, HEARD_DESKTOP, HEARD_QUIET, HEARD_MIC, HEARD_MIXED };

static const char *heard_name(enum heard h)
{
	switch (h) {
	case HEARD_NOTHING:
		return "no audio";
	case HEARD_DESKTOP:
		return "the desktop";
	case HEARD_QUIET:
		return "silence";
	case HEARD_MIC:
		return "the microphone";
	case HEARD_MIXED:
		return "mixed audio";
	}
	return "?";
}

/* What the capture recorded over the next 300 ms. */
static enum heard listen(void)
{
	pthread_mutex_lock(&audio_mutex);
	frames_desktop = frames_mic = frames_quiet = frames_other = 0;
	pthread_mutex_unlock(&audio_mutex);

	os_sleep_ms(300);

	pthread_mutex_lock(&audio_mutex);
	uint64_t desktop = frames_desktop, mic = frames_mic, quiet = frames_quiet, other = frames_other;
	pthread_mutex_unlock(&audio_mutex);

	uint64_t total = desktop + mic + quiet + other;
	if (total < 4800)
		return HEARD_NOTHING;
	if (mic * 10 >= total)
		return HEARD_MIC;
	if (desktop * 10 >= total * 9)
		return HEARD_DESKTOP;
	if (quiet * 10 >= total * 9)
		return HEARD_QUIET;
	return HEARD_MIXED;
}

/* Waits for the capture to record `want` from `source`. */
static void expect(const char *source, enum heard want, const char *when)
{
	char recorded[256] = "";
	enum heard heard = HEARD_NOTHING;
	uint64_t end = os_gettime_ns() + (uint64_t)FOLLOW_TIMEOUT_MS * 1000000;

	do {
		heard = listen();
		stream_source(CAPTURE, recorded, sizeof(recorded));
		/* A monitor of an idle output may deliver nothing at all. */
		bool heard_ok = heard == want || (want == HEARD_QUIET && heard == HEARD_NOTHING);
		if (heard_ok && strcmp(recorded, source) == 0)
			return;
	} while (os_gettime_ns() < end);

	fprintf(stderr, "%s: the capture records from '%s' and hears %s; expected '%s' and %s\n", when,
		*recorded ? recorded : "(no stream)", heard_name(heard), source, heard_name(want));
	fail();
}

/* The microphone must never have reached the capture. */
static void expect_no_mic(const char *when)
{
	pthread_mutex_lock(&audio_mutex);
	uint64_t mic = mic_total;
	pthread_mutex_unlock(&audio_mutex);

	if (mic > 0) {
		fprintf(stderr, "%s: the output capture recorded %llu frames of the microphone\n", when,
			(unsigned long long)mic);
		fail();
	}
}

/* ---- fixture ---- */

static int module_sink_a = -1, module_sink_b = -1, module_sink_c = -1, module_sink_d = -1, module_mic = -1;
static obs_source_t *capture;

static void start_capture(const char *id, const char *device)
{
	obs_data_t *settings = obs_data_create();
	obs_data_set_string(settings, "device_id", device);
	capture = obs_source_create_private(id, CAPTURE, settings);
	obs_data_release(settings);
	assert_non_null(capture);

	pthread_mutex_lock(&audio_mutex);
	mic_total = 0;
	pthread_mutex_unlock(&audio_mutex);
	obs_source_add_audio_capture_callback(capture, on_audio, NULL);
}

static int case_setup(void **state)
{
	(void)state;
	if (!module_up)
		return 0;

	module_sink_a = load_sink(SINK_A);
	module_sink_b = load_sink(SINK_B);
	module_sink_c = load_sink(SINK_C);
	module_mic = load_module("module-pipe-source source_name=%s file=%s format=float32le rate=48000 channels=1 "
				 "source_properties=device.description=%s",
				 MIC, fifo_path, MIC);
	pactl("set-default-sink %s", SINK_A);
	pactl("set-default-source %s", MIC);
	start_mic();
	start_player();
	return 0;
}

static int case_teardown(void **state)
{
	(void)state;
	if (capture) {
		obs_source_remove_audio_capture_callback(capture, on_audio, NULL);
		obs_source_release(capture);
		capture = NULL;
	}
	ui_drain();
	stop_player();
	stop_mic();
	unload_module(&module_sink_d);
	unload_module(&module_sink_c);
	unload_module(&module_sink_b);
	unload_module(&module_sink_a);
	unload_module(&module_mic);
	return 0;
}

#define REQUIRE_OBS()          \
	do {                   \
		if (!module_up) \
			skip(); \
	} while (0)

/* The user picks another output (or a dock or HDMI output is selected). */
static void default_output_switch(void **state)
{
	(void)state;
	REQUIRE_OBS();

	start_capture("pulse_output_capture", "default");
	expect(SINK_A ".monitor", HEARD_DESKTOP, "started on the default output A");

	pactl("set-default-sink %s", SINK_B);
	expect(SINK_B ".monitor", HEARD_DESKTOP, "after the default output changed to B");

	pactl("set-default-sink %s", SINK_C);
	expect(SINK_C ".monitor", HEARD_DESKTOP, "after the default output changed to C");

	expect_no_mic("default output switched");
}

/* The default output disappears (headset off, USB DAC unplugged), then
 * comes back. */
static void default_output_unplugged(void **state)
{
	(void)state;
	REQUIRE_OBS();

	start_capture("pulse_output_capture", "default");
	expect(SINK_A ".monitor", HEARD_DESKTOP, "started on the default output A");

	unload_module(&module_sink_a);
	char fallback[256] = "";
	for (int i = 0; i < 50 && (!*fallback || strcmp(fallback, SINK_A) == 0); i++) {
		os_sleep_ms(20);
		default_sink(fallback, sizeof(fallback));
	}
	if (strcmp(fallback, SINK_B) != 0 && strcmp(fallback, SINK_C) != 0) {
		fprintf(stderr, "the server chose '%s' as the default output after A was unplugged\n", fallback);
		fail();
	}
	char monitor[300];
	snprintf(monitor, sizeof(monitor), "%s.monitor", fallback);
	expect(monitor, HEARD_DESKTOP, "after the default output A was unplugged");
	/* Long enough for the server to move the stream to the default
	 * source and for that to be recorded. */
	os_sleep_ms(1500);
	expect(monitor, HEARD_DESKTOP, "a while after the default output A was unplugged");

	module_sink_a = load_sink(SINK_A);
	pactl("set-default-sink %s", SINK_A);
	expect(SINK_A ".monitor", HEARD_DESKTOP, "after A was plugged back in and made the default");

	expect_no_mic("default output unplugged");
}

/* A new output appears and becomes the default (a USB headset is plugged
 * in), then goes away again. */
static void new_default_output(void **state)
{
	(void)state;
	REQUIRE_OBS();

	start_capture("pulse_output_capture", "default");
	expect(SINK_A ".monitor", HEARD_DESKTOP, "started on the default output A");

	module_sink_d = load_sink(SINK_D);
	pactl("set-default-sink %s", SINK_D);
	expect(SINK_D ".monitor", HEARD_DESKTOP, "after the new output D became the default");

	unload_module(&module_sink_d);
	char fallback[256] = "";
	for (int i = 0; i < 50 && (!*fallback || strcmp(fallback, SINK_D) == 0); i++) {
		os_sleep_ms(20);
		default_sink(fallback, sizeof(fallback));
	}
	char monitor[300];
	snprintf(monitor, sizeof(monitor), "%s.monitor", fallback);
	expect(monitor, HEARD_DESKTOP, "after the new output D was unplugged again");

	expect_no_mic("new default output");
}

/* An output the user chose by name stays chosen when the default changes. */
static void named_output_stays(void **state)
{
	(void)state;
	REQUIRE_OBS();

	start_capture("pulse_output_capture", SINK_A ".monitor");
	expect(SINK_A ".monitor", HEARD_DESKTOP, "started on the chosen output A");

	/* The desktop moves to B; the capture stays on A, now silent. */
	pactl("set-default-sink %s", SINK_B);
	os_sleep_ms(1000);
	expect(SINK_A ".monitor", HEARD_QUIET, "after the default output changed to B");

	pactl("set-default-sink %s", SINK_A);
	expect(SINK_A ".monitor", HEARD_DESKTOP, "after the default output changed back to A");

	expect_no_mic("named output");
}

/* Control: an Audio Input Capture set to "Default" follows the default
 * source, here from the microphone to an output's monitor and back. */
static void default_input_follows(void **state)
{
	(void)state;
	REQUIRE_OBS();

	start_capture("pulse_input_capture", "default");
	expect(MIC, HEARD_MIC, "started on the default input");

	pactl("set-default-sink %s", SINK_B);
	pactl("set-default-source %s.monitor", SINK_B);
	expect(SINK_B ".monitor", HEARD_DESKTOP, "after the default input changed to B's monitor");

	pactl("set-default-source %s", MIC);
	expect(MIC, HEARD_MIC, "after the default input changed back to the microphone");
}

/* Saving the properties of a "Default" capture unchanged keeps its stream:
 * it compared "default" with the device it had resolved, and restarted. */
static void reapplying_default_keeps_stream(void **state)
{
	(void)state;
	REQUIRE_OBS();

	start_capture("pulse_output_capture", "default");
	expect(SINK_A ".monitor", HEARD_DESKTOP, "started on the default output A");

	char before[64] = "", after[64] = "";
	run("LC_ALL=C pactl list short source-outputs 2>/dev/null | cut -f1 | sort | tr '\\n' ' '", before,
	    sizeof(before));
	obs_data_t *settings = obs_source_get_settings(capture);
	obs_source_update(capture, settings);
	obs_data_release(settings);
	ui_drain();
	os_sleep_ms(300);
	run("LC_ALL=C pactl list short source-outputs 2>/dev/null | cut -f1 | sort | tr '\\n' ' '", after,
	    sizeof(after));
	if (strcmp(before, after) != 0) {
		fprintf(stderr, "saving unchanged settings restarted the stream: source outputs '%s' -> '%s'\n", before,
			after);
		fail();
	}
	expect(SINK_A ".monitor", HEARD_DESKTOP, "after saving unchanged settings");
}

/* ---- group ---- */

static void quiet_log(int lvl, const char *msg, va_list args, void *p)
{
	(void)p;
	if (lvl <= LOG_WARNING || getenv("OBS_TEST_VERBOSE")) {
		vfprintf(stderr, msg, args);
		fputc('\n', stderr);
	}
}

static int group_setup(void **state)
{
	(void)state;
	signal(SIGPIPE, SIG_IGN);

	const char *runtime = getenv("XDG_RUNTIME_DIR");
	snprintf(fifo_path, sizeof(fifo_path), "%s/obs_test_mic.fifo", runtime ? runtime : "/tmp");

	base_set_log_handler(quiet_log, NULL);
	/* obs_startup() needs a display on Linux; the cases skip without one. */
	obs_up = obs_startup("en-US", NULL, NULL);
	if (!obs_up) {
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
		return 0;
	}
	pthread_create(&ui_thread, NULL, ui_main, NULL);
	obs_set_ui_task_handler(ui_task_handler);

	struct obs_audio_info oai = {.samples_per_sec = 48000, .speakers = SPEAKERS_STEREO};
	if (!obs_reset_audio(&oai)) {
		fprintf(stderr, "obs_reset_audio failed\n");
		return -1;
	}

	obs_module_t *module = NULL;
	if (obs_open_module(&module, PULSE_MODULE_PATH, PULSE_DATA_PATH) != MODULE_SUCCESS ||
	    !obs_init_module(module)) {
		fprintf(stderr, "could not load linux-pulseaudio from %s\n", PULSE_MODULE_PATH);
		return -1;
	}
	module_up = true;
	return 0;
}

static int group_teardown(void **state)
{
	(void)state;
	if (obs_up) {
		ui_drain();
		obs_shutdown();
		pthread_mutex_lock(&ui_mutex);
		ui_quit = true;
		pthread_cond_broadcast(&ui_cond);
		pthread_mutex_unlock(&ui_mutex);
		pthread_join(ui_thread, NULL);
	}
	return 0;
}

int main(void)
{
	if (!getenv("PULSE_SERVER")) {
		fprintf(stderr, "PULSE_SERVER is not set: run through test_pulse_pulseaudio.sh or "
				"test_pulse_monitor_device.sh\n");
		return 77;
	}
	if (system("command -v pacat >/dev/null 2>&1") != 0) {
		fprintf(stderr, "pacat not found: skipping\n");
		return 77;
	}

	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(default_output_switch, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(default_output_unplugged, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(new_default_output, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(named_output_stays, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(default_input_follows, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(reapplying_default_keeps_stream, case_setup, case_teardown),
	};

	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
