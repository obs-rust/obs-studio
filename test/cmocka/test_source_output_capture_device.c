/* Characterization of obs_source_audio_output_capture_device_changed() and
 * its activate handler before their Rust swap (#5824), calling the export
 * directly against hybrid libobs and the C reference.
 *
 * test_set_audio_monitoring_device covers the export through
 * obs_set_audio_monitoring_device(); this test pins the export itself:
 *
 * - a source without OBS_SOURCE_DO_NOT_SELF_MONITOR is ignored, even when
 *   its device matches: no signal, no log line;
 * - an Audio Output Capture source whose device is the monitoring device
 *   (PulseAudio: "<sink>.monitor", or "default" resolved to the server's
 *   default sink) sends one deduplication_changed with itself and logs one
 *   LOG_INFO line naming it, and from then on its "activate" signal runs the
 *   check again with the "device_id" of its settings (a NULL "source" in
 *   that calldata does nothing);
 * - a source that no longer matches and is the deduplicating one sends
 *   deduplication_changed with NULL, logs "Deduplication logic stopped." and
 *   its "activate" signal no longer runs the check;
 * - a non-matching source that is not deduplicating, and a NULL device id,
 *   do nothing.
 *
 * The NULL monitoring-device case can't be reached from outside libobs (the
 * core always holds one once started), and on Linux the host default-device
 * hook always declines; both go to Tier 1 and Tier 3.
 *
 * Only external libpulse calls are interposed (as
 * test_set_audio_monitoring_device does), so PulseAudio monitoring is
 * available without a sound server. Audio only, no display needed. */
#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <setjmp.h>
#include <string.h>
#include <cmocka.h>

#include <pulse/pulseaudio.h>
#include <obs.h>
#include <util/base.h>
#include <util/bmem.h>

#define AOC_SOURCE "output_capture_device_aoc_source"
#define PLAIN_SOURCE "output_capture_device_plain_source"

/* ------------------------------------------------------------------------ */
/* Interposed libpulse                                                       */

static int fake_object;

pa_threaded_mainloop *pa_threaded_mainloop_new(void)
{
	return (pa_threaded_mainloop *)&fake_object;
}
int pa_threaded_mainloop_start(pa_threaded_mainloop *m)
{
	UNUSED_PARAMETER(m);
	return 0;
}
void pa_threaded_mainloop_stop(pa_threaded_mainloop *m)
{
	UNUSED_PARAMETER(m);
}
void pa_threaded_mainloop_free(pa_threaded_mainloop *m)
{
	UNUSED_PARAMETER(m);
}
void pa_threaded_mainloop_lock(pa_threaded_mainloop *m)
{
	UNUSED_PARAMETER(m);
}
void pa_threaded_mainloop_unlock(pa_threaded_mainloop *m)
{
	UNUSED_PARAMETER(m);
}
void pa_threaded_mainloop_wait(pa_threaded_mainloop *m)
{
	UNUSED_PARAMETER(m);
}
void pa_threaded_mainloop_signal(pa_threaded_mainloop *m, int wait_for_accept)
{
	UNUSED_PARAMETER(m);
	UNUSED_PARAMETER(wait_for_accept);
}
pa_mainloop_api *pa_threaded_mainloop_get_api(pa_threaded_mainloop *m)
{
	UNUSED_PARAMETER(m);
	return NULL;
}

pa_context *pa_context_new_with_proplist(pa_mainloop_api *api, const char *name, const pa_proplist *p)
{
	UNUSED_PARAMETER(api);
	UNUSED_PARAMETER(name);
	UNUSED_PARAMETER(p);
	return (pa_context *)&fake_object;
}
void pa_context_set_state_callback(pa_context *c, pa_context_notify_cb_t cb, void *userdata)
{
	UNUSED_PARAMETER(c);
	UNUSED_PARAMETER(cb);
	UNUSED_PARAMETER(userdata);
}
int pa_context_connect(pa_context *c, const char *server, pa_context_flags_t flags, const pa_spawn_api *api)
{
	UNUSED_PARAMETER(c);
	UNUSED_PARAMETER(server);
	UNUSED_PARAMETER(flags);
	UNUSED_PARAMETER(api);
	return 0;
}
void pa_context_disconnect(pa_context *c)
{
	UNUSED_PARAMETER(c);
}
void pa_context_unref(pa_context *c)
{
	UNUSED_PARAMETER(c);
}
pa_context_state_t pa_context_get_state(const pa_context *c)
{
	UNUSED_PARAMETER(c);
	return PA_CONTEXT_READY;
}

pa_operation *pa_context_get_server_info(pa_context *c, pa_server_info_cb_t cb, void *userdata)
{
	pa_server_info info;
	memset(&info, 0, sizeof(info));
	info.default_sink_name = "sink_default";
	info.server_name = "test";
	info.server_version = "1";
	cb(c, &info, userdata);
	return (pa_operation *)&fake_object;
}
pa_operation_state_t pa_operation_get_state(const pa_operation *o)
{
	UNUSED_PARAMETER(o);
	return PA_OPERATION_DONE;
}
void pa_operation_unref(pa_operation *o)
{
	UNUSED_PARAMETER(o);
}

pa_operation *pa_context_get_sink_info_by_name(pa_context *c, const char *name, pa_sink_info_cb_t cb, void *userdata)
{
	UNUSED_PARAMETER(name);
	pa_sink_info info = {0};
	info.index = 1;
	info.sample_spec = (pa_sample_spec){PA_SAMPLE_FLOAT32LE, 48000, 2};
	cb(c, &info, 0, userdata);
	cb(c, NULL, 1, userdata);
	return (pa_operation *)&fake_object;
}
void pa_context_set_subscribe_callback(pa_context *c, pa_context_subscribe_cb_t cb, void *userdata)
{
	UNUSED_PARAMETER(c);
	UNUSED_PARAMETER(cb);
	UNUSED_PARAMETER(userdata);
}
pa_operation *pa_context_subscribe(pa_context *c, pa_subscription_mask_t m, pa_context_success_cb_t cb, void *userdata)
{
	UNUSED_PARAMETER(c);
	UNUSED_PARAMETER(m);
	UNUSED_PARAMETER(cb);
	UNUSED_PARAMETER(userdata);
	return NULL;
}

/* ------------------------------------------------------------------------ */
/* Sources, signals and log lines                                            */

static const char *source_name(void *unused)
{
	UNUSED_PARAMETER(unused);
	return "output capture device source";
}

static void *source_create(obs_data_t *settings, obs_source_t *source)
{
	UNUSED_PARAMETER(settings);
	return source;
}

static void source_destroy(void *unused)
{
	UNUSED_PARAMETER(unused);
}

/* Private, so obs_set_audio_monitoring_device() never tells them. */
static obs_source_t *create_with_device(const char *id, const char *name, const char *device_id)
{
	obs_data_t *settings = obs_data_create();
	obs_data_set_string(settings, "device_id", device_id);
	obs_source_t *source = obs_source_create_private(id, name, settings);
	obs_data_release(settings);
	assert_non_null(source);
	return source;
}

static void set_device_setting(obs_source_t *source, const char *device_id)
{
	obs_data_t *settings = obs_source_get_settings(source);
	obs_data_set_string(settings, "device_id", device_id);
	obs_data_release(settings);
}

#define MAX_SIGNALS 8
static obs_source_t *signalled[MAX_SIGNALS];
static int signals_seen;

static void on_deduplication_changed(void *unused, calldata_t *cd)
{
	UNUSED_PARAMETER(unused);
	assert_true(signals_seen < MAX_SIGNALS);
	signalled[signals_seen++] = calldata_ptr(cd, "source");
}

#define MAX_LINES 8
static char lines[MAX_LINES][256];
static int line_levels[MAX_LINES];
static int lines_seen;
static log_handler_t old_handler;
static void *old_param;

static void capture_log(int level, const char *format, va_list args, void *param)
{
	UNUSED_PARAMETER(param);
	char line[256];
	vsnprintf(line, sizeof(line), format, args);
	/* Only this unit's lines: other threads (audio) may log too. */
	if (!strstr(line, "Audio Output Capture") && !strstr(line, "Deduplication"))
		return;
	if (lines_seen < MAX_LINES) {
		snprintf(lines[lines_seen], sizeof(lines[0]), "%s", line);
		line_levels[lines_seen] = level;
	}
	lines_seen++;
}

static void observe(bool on)
{
	signal_handler_t *handler = obs_get_signal_handler();
	if (on) {
		signal_handler_connect(handler, "deduplication_changed", on_deduplication_changed, NULL);
		base_get_log_handler(&old_handler, &old_param);
		base_set_log_handler(capture_log, NULL);
	} else {
		base_set_log_handler(old_handler, old_param);
		signal_handler_disconnect(handler, "deduplication_changed", on_deduplication_changed, NULL);
	}
	signals_seen = 0;
	lines_seen = 0;
}

static void reset_seen(void)
{
	signals_seen = 0;
	lines_seen = 0;
}

/* The core applies a deduplication change as an audio task; run every task
 * queued so far before the next step depends on the deduplicating source. */
static void audio_task_noop(void *unused)
{
	UNUSED_PARAMETER(unused);
}

static void flush_audio_tasks(void)
{
	obs_queue_task(OBS_TASK_AUDIO, audio_task_noop, NULL, true);
}

/* The source's "activate" signal, as obs_source_dosignal() sends it (or with
 * no "source" at all). */
static void send_activate(obs_source_t *source, bool with_source)
{
	struct calldata cd;
	calldata_init(&cd);
	if (with_source)
		calldata_set_ptr(&cd, "source", source);
	signal_handler_signal(obs_source_get_signal_handler(source), "activate", &cd);
	calldata_free(&cd);
}

static void assert_started(obs_source_t *source)
{
	char expected[256];
	snprintf(expected, sizeof(expected),
		 "Device for 'Audio Output Capture' source %s is also used for audio monitoring."
		 "\nDeduplication logic is being applied to all monitored sources.",
		 obs_source_get_name(source));
	assert_int_equal(signals_seen, 1);
	assert_ptr_equal(signalled[0], source);
	assert_int_equal(lines_seen, 1);
	assert_int_equal(line_levels[0], LOG_INFO);
	assert_string_equal(lines[0], expected);
}

static void assert_stopped(void)
{
	assert_int_equal(signals_seen, 1);
	assert_null(signalled[0]);
	assert_int_equal(lines_seen, 1);
	assert_int_equal(line_levels[0], LOG_INFO);
	assert_string_equal(lines[0], "Deduplication logic stopped.");
}

static void assert_nothing(void)
{
	assert_int_equal(signals_seen, 0);
	assert_int_equal(lines_seen, 0);
}

/* Stops whatever deduplication a case left running, so the next one starts
 * with no deduplicating source. */
static void stop_deduplication(obs_source_t *source)
{
	reset_seen();
	obs_source_audio_output_capture_device_changed(source, "no_such_sink.monitor");
	assert_stopped();
	flush_audio_tasks();
	reset_seen();
}

/* ------------------------------------------------------------------------ */
/* Cases                                                                     */

static void source_without_flag_is_ignored(void **state)
{
	UNUSED_PARAMETER(state);
	obs_source_t *plain = create_with_device(PLAIN_SOURCE, "plain", "sink_a.monitor");
	observe(true);

	obs_source_audio_output_capture_device_changed(plain, "sink_a.monitor");
	assert_nothing();
	obs_source_audio_output_capture_device_changed(plain, "other.monitor");
	assert_nothing();
	/* No activate handler was connected. */
	send_activate(plain, true);
	assert_nothing();

	observe(false);
	obs_source_release(plain);
}

static void matching_device_starts_deduplication(void **state)
{
	UNUSED_PARAMETER(state);
	obs_source_t *aoc = create_with_device(AOC_SOURCE, "aoc match", "sink_a.monitor");
	observe(true);

	obs_source_audio_output_capture_device_changed(aoc, "sink_a.monitor");
	assert_started(aoc);
	flush_audio_tasks();

	/* The activate handler runs the check again with the settings' id. */
	reset_seen();
	send_activate(aoc, true);
	assert_started(aoc);
	flush_audio_tasks();

	/* A calldata with no source does nothing. */
	reset_seen();
	send_activate(aoc, false);
	assert_nothing();

	/* Its settings no longer match: the activate check stops it. */
	reset_seen();
	set_device_setting(aoc, "sink_b.monitor");
	send_activate(aoc, true);
	assert_stopped();
	flush_audio_tasks();

	/* Stopped also disconnected the handler. */
	reset_seen();
	set_device_setting(aoc, "sink_a.monitor");
	send_activate(aoc, true);
	assert_nothing();

	observe(false);
	obs_source_release(aoc);
}

static void default_device_resolves_to_default_sink(void **state)
{
	UNUSED_PARAMETER(state);
	obs_source_t *aoc = create_with_device(AOC_SOURCE, "aoc default", "default");
	observe(true);

	/* "default" is the server's default sink, sink_default, not sink_a. */
	obs_source_audio_output_capture_device_changed(aoc, "default");
	assert_nothing();
	obs_source_audio_output_capture_device_changed(aoc, "sink_default.monitor");
	assert_nothing();

	/* With "default" monitored, both spellings of it match. */
	assert_true(obs_set_audio_monitoring_device("Default", "default"));
	obs_source_audio_output_capture_device_changed(aoc, "default");
	assert_started(aoc);
	flush_audio_tasks();
	reset_seen();
	obs_source_audio_output_capture_device_changed(aoc, "sink_default.monitor");
	assert_started(aoc);
	flush_audio_tasks();
	stop_deduplication(aoc);
	assert_true(obs_set_audio_monitoring_device("Sink A", "sink_a"));

	observe(false);
	obs_source_release(aoc);
}

static void non_matching_stops_only_the_deduplicating_source(void **state)
{
	UNUSED_PARAMETER(state);
	obs_source_t *a = create_with_device(AOC_SOURCE, "aoc a", "sink_a.monitor");
	obs_source_t *b = create_with_device(AOC_SOURCE, "aoc b", "sink_b.monitor");
	observe(true);

	/* Nobody deduplicates: a non-match and a NULL id do nothing. */
	obs_source_audio_output_capture_device_changed(b, "sink_b.monitor");
	assert_nothing();
	obs_source_audio_output_capture_device_changed(b, NULL);
	assert_nothing();

	obs_source_audio_output_capture_device_changed(a, "sink_a.monitor");
	assert_started(a);
	flush_audio_tasks();

	/* b is not the deduplicating source. */
	reset_seen();
	obs_source_audio_output_capture_device_changed(b, "sink_b.monitor");
	assert_nothing();
	obs_source_audio_output_capture_device_changed(b, NULL);
	assert_nothing();

	/* a is: a NULL id stops it. */
	obs_source_audio_output_capture_device_changed(a, NULL);
	assert_stopped();
	flush_audio_tasks();

	/* Stopped already: nothing more. */
	reset_seen();
	obs_source_audio_output_capture_device_changed(a, NULL);
	assert_nothing();

	/* A second match takes over without a stop. */
	obs_source_audio_output_capture_device_changed(a, "sink_a.monitor");
	assert_started(a);
	flush_audio_tasks();
	reset_seen();
	obs_source_audio_output_capture_device_changed(b, "sink_a.monitor");
	assert_started(b);
	flush_audio_tasks();
	/* a is no longer the deduplicating one. */
	reset_seen();
	obs_source_audio_output_capture_device_changed(a, "sink_b.monitor");
	assert_nothing();
	stop_deduplication(b);

	observe(false);
	obs_source_release(b);
	obs_source_release(a);
}

int main(void)
{
	long before = bnum_allocs();
	if (!obs_startup("en-US", NULL, NULL))
		return 77;
	struct obs_audio_info audio = {.samples_per_sec = 48000, .speakers = SPEAKERS_STEREO};
	if (!obs_reset_audio(&audio))
		return 1;

	struct obs_source_info aoc_info = {
		.id = AOC_SOURCE,
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_AUDIO | OBS_SOURCE_DO_NOT_SELF_MONITOR,
		.get_name = source_name,
		.create = source_create,
		.destroy = source_destroy,
	};
	obs_register_source(&aoc_info);
	struct obs_source_info plain_info = aoc_info;
	plain_info.id = PLAIN_SOURCE;
	plain_info.output_flags = OBS_SOURCE_AUDIO;
	obs_register_source(&plain_info);

	if (!obs_set_audio_monitoring_device("Sink A", "sink_a"))
		return 1;

	const struct CMUnitTest tests[] = {
		cmocka_unit_test(source_without_flag_is_ignored),
		cmocka_unit_test(matching_device_starts_deduplication),
		cmocka_unit_test(default_device_resolves_to_default_sink),
		cmocka_unit_test(non_matching_stops_only_the_deduplicating_source),
	};
	int result = cmocka_run_group_tests(tests, NULL, NULL);
	if (result)
		return result;
	obs_shutdown();
	if (bnum_allocs() != before) {
		fprintf(stderr, "Unbalanced libobs allocations: %ld (expected %ld)\n", bnum_allocs(), before);
		return 1;
	}
	return 0;
}
