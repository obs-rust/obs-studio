#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <setjmp.h>
#include <unistd.h>
#include <cmocka.h>

#include <pulse/pulseaudio.h>

#include <obs.h>
#include <util/base.h>

/* End to end for obsproject/obs-studio#10342 (reported by gentoo-root): the
 * audio monitoring device chosen in Settings -> Audio -> Advanced must keep
 * the monitoring stream when the system's default output changes.
 *
 * Runs libobs audio monitoring against a real sound server: a private
 * PipeWire + WirePlumber + pipewire-pulse stack started by
 * test_pulse_monitor_device.sh (no hardware, its own runtime dir and D-Bus),
 * with two null sinks this test loads itself. The monitoring stream is the
 * only sink input on that server; pactl reports which sink it plays on.
 * Without that stack the wrapper exits 77 and ctest reports a skip. */

#define SOURCE_ID "pulse_monitor_device_test_source"

static bool obs_up;
static int module_a = -1;
static int module_b = -1;

static void quiet_log(int lvl, const char *msg, va_list args, void *p)
{
	(void)p;
	if (lvl <= LOG_WARNING) {
		vfprintf(stderr, msg, args);
		fputc('\n', stderr);
	}
}

static const char *test_source_name(void *type_data)
{
	(void)type_data;
	return "monitor device test source";
}

static void *test_source_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	return source;
}

static void test_source_destroy(void *data)
{
	(void)data;
}

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
	return pclose(p) == 0;
}

static int load_sink(const char *name)
{
	char cmd[256], out[64];
	snprintf(cmd, sizeof(cmd), "pactl load-module module-null-sink sink_name=%s 2>/dev/null", name);
	if (!run(cmd, out, sizeof(out)) || !*out)
		return -1;
	return atoi(out);
}

static void unload_module(int *module)
{
	char cmd[128];
	if (*module < 0)
		return;
	snprintf(cmd, sizeof(cmd), "pactl unload-module %d 2>/dev/null", *module);
	run(cmd, NULL, 0);
	*module = -1;
}

/* A client that stays connected, as libobs's monitoring context does.
 * pipewire-pulse acknowledges set-default-sink before every connected
 * client sees the new default: a fresh client (pactl) reads it at once,
 * one connected earlier only once the server has passed the change on to
 * it. Monitoring opened in that window resolves "default" to the old sink,
 * so set_default_sink() waits until this client reports the new one. */
static pa_mainloop *observer_loop;
static pa_context *observer;

static bool observer_wait(pa_operation *op)
{
	if (!op)
		return false;
	while (pa_operation_get_state(op) == PA_OPERATION_RUNNING)
		if (pa_mainloop_iterate(observer_loop, 1, NULL) < 0)
			break;
	bool done = pa_operation_get_state(op) == PA_OPERATION_DONE;
	pa_operation_unref(op);
	return done;
}

static bool observer_connect(void)
{
	observer_loop = pa_mainloop_new();
	observer = pa_context_new(pa_mainloop_get_api(observer_loop), "obs-test-default-sink-observer");
	if (pa_context_connect(observer, NULL, PA_CONTEXT_NOFLAGS, NULL) < 0)
		return false;
	for (;;) {
		pa_context_state_t st = pa_context_get_state(observer);
		if (st == PA_CONTEXT_READY)
			return true;
		if (!PA_CONTEXT_IS_GOOD(st) || pa_mainloop_iterate(observer_loop, 1, NULL) < 0)
			return false;
	}
}

static void observer_disconnect(void)
{
	if (observer) {
		pa_context_disconnect(observer);
		pa_context_unref(observer);
		observer = NULL;
	}
	if (observer_loop) {
		pa_mainloop_free(observer_loop);
		observer_loop = NULL;
	}
}

static void observer_server_info(pa_context *c, const pa_server_info *info, void *userdata)
{
	(void)c;
	snprintf(userdata, 128, "%s", info && info->default_sink_name ? info->default_sink_name : "");
}

/* Waits up to 3 s for the observer to report `name` as the default sink. */
static bool observer_sees_default(const char *name)
{
	char seen[128] = "";
	for (int i = 0; i < 300; i++) {
		if (observer_wait(pa_context_get_server_info(observer, observer_server_info, seen)) &&
		    strcmp(seen, name) == 0)
			return true;
		usleep(10000);
	}
	fprintf(stderr, "a connected client still sees default sink '%s', expected '%s'\n", seen, name);
	return false;
}

static void set_default_sink(const char *name)
{
	char cmd[128];
	snprintf(cmd, sizeof(cmd), "pactl set-default-sink %s 2>/dev/null", name);
	if (!run(cmd, NULL, 0)) {
		fprintf(stderr, "pactl set-default-sink %s failed\n", name);
		fail();
	}
	if (!observer_sees_default(name))
		fail();
}

/* Name of the sink the (only) sink input plays on, or "" if there is none. */
static void stream_sink(char *name, size_t size)
{
	char line[256];
	name[0] = 0;
	if (!run("pactl list short sink-inputs 2>/dev/null", line, sizeof(line)) || !*line)
		return;

	unsigned input, sink;
	if (sscanf(line, "%u %u", &input, &sink) != 2)
		return;

	FILE *p = popen("pactl list short sinks 2>/dev/null", "r");
	if (!p)
		return;
	while (fgets(line, sizeof(line), p)) {
		unsigned idx;
		char sink_name[128];
		if (sscanf(line, "%u %127s", &idx, sink_name) == 2 && idx == sink) {
			snprintf(name, size, "%s", sink_name);
			break;
		}
	}
	pclose(p);
}

/* Waits up to 3 s for the monitoring stream to play on `expected`. */
static void expect_stream_on(const char *expected, const char *when)
{
	char name[128] = "";
	for (int i = 0; i < 60; i++) {
		stream_sink(name, sizeof(name));
		if (strcmp(name, expected) == 0)
			return;
		usleep(50000);
	}
	fprintf(stderr, "%s: monitoring plays on '%s', expected '%s'\n", when, *name ? name : "(no stream)", expected);
	fail();
}

static obs_source_t *start_monitoring(const char *device_name, const char *device_id)
{
	if (!obs_set_audio_monitoring_device(device_name, device_id)) {
		fprintf(stderr, "obs_set_audio_monitoring_device(%s) failed\n", device_id);
		fail();
	}
	obs_source_t *source = obs_source_create_private(SOURCE_ID, "monitored", NULL);
	assert_non_null(source);
	obs_source_set_monitoring_enabled(source, true);
	return source;
}

static void chosen_device_stays_when_default_changes(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	set_default_sink("obs_test_sink_a");
	obs_source_t *source = start_monitoring("Sink A", "obs_test_sink_a");
	expect_stream_on("obs_test_sink_a", "monitoring started on the chosen sink");

	set_default_sink("obs_test_sink_b");
	usleep(500000);
	expect_stream_on("obs_test_sink_a", "after the default output changed");

	obs_source_release(source);
}

static void default_device_follows_default_output(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	set_default_sink("obs_test_sink_a");
	obs_source_t *source = start_monitoring("Default", "default");
	expect_stream_on("obs_test_sink_a", "monitoring started on the default sink");

	set_default_sink("obs_test_sink_b");
	expect_stream_on("obs_test_sink_b", "after the default output changed");

	obs_source_release(source);
}

static void unplugged_device_falls_back_and_returns(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	set_default_sink("obs_test_sink_b");
	obs_source_t *source = start_monitoring("Sink A", "obs_test_sink_a");
	expect_stream_on("obs_test_sink_a", "monitoring started on the chosen sink");

	unload_module(&module_a);
	expect_stream_on("obs_test_sink_b", "after the chosen sink was unplugged");

	module_a = load_sink("obs_test_sink_a");
	assert_true(module_a >= 0);
	expect_stream_on("obs_test_sink_a", "after the chosen sink came back");

	obs_source_release(source);
}

static int setup(void **state)
{
	(void)state;
	module_a = load_sink("obs_test_sink_a");
	module_b = load_sink("obs_test_sink_b");
	if (module_a < 0 || module_b < 0) {
		fprintf(stderr, "could not load null sinks; is PULSE_SERVER a private test server?\n");
		return -1;
	}
	if (!observer_connect()) {
		fprintf(stderr, "could not connect to PULSE_SERVER\n");
		return -1;
	}

	base_set_log_handler(quiet_log, NULL);
	/* obs_startup() needs a display on Linux; the cases skip without one. */
	obs_up = obs_startup("en-US", NULL, NULL);
	if (!obs_up) {
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
		return 0;
	}

	struct obs_audio_info oai = {.samples_per_sec = 48000, .speakers = SPEAKERS_STEREO};
	if (!obs_reset_audio(&oai)) {
		fprintf(stderr, "obs_reset_audio failed\n");
		return -1;
	}

	struct obs_source_info info = {
		.id = SOURCE_ID,
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_AUDIO,
		.get_name = test_source_name,
		.create = test_source_create,
		.destroy = test_source_destroy,
	};
	obs_register_source(&info);
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	if (obs_up)
		obs_shutdown();
	observer_disconnect();
	unload_module(&module_a);
	unload_module(&module_b);
	return 0;
}

int main(void)
{
	if (!getenv("PULSE_SERVER")) {
		fprintf(stderr, "PULSE_SERVER is not set: run through test_pulse_monitor_device.sh\n");
		return 77;
	}

	const struct CMUnitTest tests[] = {
		cmocka_unit_test(chosen_device_stays_when_default_changes),
		cmocka_unit_test(default_device_follows_default_output),
		cmocka_unit_test(unplugged_device_falls_back_and_returns),
	};

	return cmocka_run_group_tests(tests, setup, teardown);
}
