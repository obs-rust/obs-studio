/* C-only characterization before extracting obs_set_audio_monitoring_device
 * and obs_reset_audio_monitoring (Refs #5489).
 *
 * test_core_setup_rules_steps (test_monitoring_device) already pins the
 * NULL and empty refusals and the return value per availability, and
 * test_pulse_monitor_reset pins how one PulseAudio monitor survives a failed
 * reset. This test pins the rest of the public behavior:
 *
 * - a new device is stored by copy: obs_get_audio_monitoring_device reads
 *   back the name and id given, not the caller's storage, and every live
 *   monitor is reset once, onto the new device;
 * - the same id again returns true, keeps the old name and resets nothing,
 *   and tells no Audio Output Capture source;
 * - every public input with OBS_SOURCE_DO_NOT_SELF_MONITOR (an Audio Output
 *   Capture source) is told about a new device with its own "device_id"
 *   setting: one capturing the new device starts deduplication, the one that
 *   was deduplicating and no longer matches stops it; inputs without the
 *   flag, and private ones, are not told;
 * - obs_reset_audio_monitoring with no monitors touches nothing, and with
 *   monitors resets each live monitor once on the current device.
 *
 * Only external libpulse calls are interposed (as test_pulse_monitor_reset
 * does), so PulseAudio monitoring is available without a sound server; the
 * monitors, the source enumeration and the deduplication signal all run in
 * libobs. Audio only, no display needed. */
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
#include <util/bmem.h>

#define MONITOR_SOURCE "monitoring_device_monitor_source"
#define AOC_SOURCE "monitoring_device_aoc_source"
#define PLAIN_SOURCE "monitoring_device_plain_source"

/* ------------------------------------------------------------------------ */
/* Interposed libpulse                                                       */

struct test_context {
	pa_context_state_t state;
	unsigned disconnects;
	unsigned unrefs;
};

struct test_stream {
	struct test_context *context;
	char device[64];
	unsigned connects;
	unsigned disconnects;
	unsigned unrefs;
};

static struct test_context contexts[16];
static unsigned contexts_created;
static struct test_stream streams[32];
static unsigned streams_created;
static unsigned mainloops_created;
static unsigned mainloops_freed;
static int fake_object;

pa_threaded_mainloop *pa_threaded_mainloop_new(void)
{
	mainloops_created++;
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
	mainloops_freed++;
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
	assert_true(contexts_created < sizeof(contexts) / sizeof(contexts[0]));
	struct test_context *context = &contexts[contexts_created++];
	context->state = PA_CONTEXT_READY;
	return (pa_context *)context;
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
	((struct test_context *)c)->disconnects++;
}
void pa_context_unref(pa_context *c)
{
	((struct test_context *)c)->unrefs++;
}
pa_context_state_t pa_context_get_state(const pa_context *c)
{
	const struct test_context *context = (const struct test_context *)c;
	assert_int_equal(context->unrefs, 0);
	return context->state;
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
pa_stream *pa_stream_new_with_proplist(pa_context *c, const char *name, const pa_sample_spec *ss,
				       const pa_channel_map *map, pa_proplist *p)
{
	UNUSED_PARAMETER(name);
	UNUSED_PARAMETER(ss);
	UNUSED_PARAMETER(map);
	UNUSED_PARAMETER(p);
	assert_true(streams_created < sizeof(streams) / sizeof(streams[0]));
	struct test_stream *stream = &streams[streams_created++];
	stream->context = (struct test_context *)c;
	return (pa_stream *)stream;
}
pa_context *pa_stream_get_context(const pa_stream *s)
{
	const struct test_stream *stream = (const struct test_stream *)s;
	assert_int_equal(stream->unrefs, 0);
	return (pa_context *)stream->context;
}
int pa_stream_connect_playback(pa_stream *s, const char *dev, const pa_buffer_attr *attr, pa_stream_flags_t flags,
			       const pa_cvolume *volume, pa_stream *sync_stream)
{
	UNUSED_PARAMETER(attr);
	UNUSED_PARAMETER(flags);
	UNUSED_PARAMETER(volume);
	UNUSED_PARAMETER(sync_stream);
	struct test_stream *stream = (struct test_stream *)s;
	snprintf(stream->device, sizeof(stream->device), "%s", dev ? dev : "(null)");
	stream->connects++;
	return 0;
}
void pa_stream_set_write_callback(pa_stream *s, pa_stream_request_cb_t cb, void *userdata)
{
	UNUSED_PARAMETER(s);
	UNUSED_PARAMETER(cb);
	UNUSED_PARAMETER(userdata);
}
int pa_stream_disconnect(pa_stream *s)
{
	((struct test_stream *)s)->disconnects++;
	return 0;
}
void pa_stream_unref(pa_stream *s)
{
	((struct test_stream *)s)->unrefs++;
}
int pa_stream_is_corked(const pa_stream *s)
{
	UNUSED_PARAMETER(s);
	return 1;
}

/* ------------------------------------------------------------------------ */
/* Sources                                                                   */

static const char *source_name(void *unused)
{
	UNUSED_PARAMETER(unused);
	return "monitoring device source";
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

/* A private input with monitoring on: one live monitor, one stream. */
static obs_source_t *create_monitor(void)
{
	unsigned before = streams_created;
	obs_source_t *source = obs_source_create_private(MONITOR_SOURCE, "monitored", NULL);
	assert_non_null(source);
	obs_source_inc_active(source);
	obs_source_set_monitoring_enabled(source, true);
	assert_int_equal(streams_created, before + 1);
	return source;
}

static void destroy_monitor(obs_source_t *source)
{
	obs_source_set_monitoring_enabled(source, false);
	obs_source_dec_active(source);
	obs_source_release(source);
}

static obs_source_t *create_with_device(const char *id, const char *name, const char *device_id, bool private)
{
	obs_data_t *settings = obs_data_create();
	obs_data_set_string(settings, "device_id", device_id);
	obs_source_t *source = private ? obs_source_create_private(id, name, settings)
				       : obs_source_create(id, name, settings, NULL);
	obs_data_release(settings);
	assert_non_null(source);
	return source;
}

/* The deduplication_changed signals the core sends while a test listens. */
#define MAX_SIGNALS 8
static obs_source_t *signalled[MAX_SIGNALS];
static int signals_seen;

static void on_deduplication_changed(void *unused, calldata_t *cd)
{
	UNUSED_PARAMETER(unused);
	assert_true(signals_seen < MAX_SIGNALS);
	signalled[signals_seen++] = calldata_ptr(cd, "source");
}

static void listen_for_deduplication(bool on)
{
	signal_handler_t *handler = obs_get_signal_handler();
	if (on)
		signal_handler_connect(handler, "deduplication_changed", on_deduplication_changed, NULL);
	else
		signal_handler_disconnect(handler, "deduplication_changed", on_deduplication_changed, NULL);
	signals_seen = 0;
}

/* The core applies a deduplication change as an audio task; run every task
 * queued so far before the next step reads the deduplicating source. */
static void audio_task_noop(void *unused)
{
	UNUSED_PARAMETER(unused);
}

static void flush_audio_tasks(void)
{
	obs_queue_task(OBS_TASK_AUDIO, audio_task_noop, NULL, true);
}

static void assert_device(const char *name, const char *id)
{
	const char *got_name = NULL;
	const char *got_id = NULL;
	obs_get_audio_monitoring_device(&got_name, &got_id);
	assert_string_equal(got_name, name);
	assert_string_equal(got_id, id);
}

/* Each monitor's stream from `first` on was replaced by exactly one new
 * stream, connected to `device`, which is still live. */
static void assert_each_reset_once(unsigned first, unsigned count, const char *device)
{
	assert_int_equal(streams_created, first + 2 * count);
	for (unsigned i = 0; i < count; i++) {
		assert_int_equal(streams[first + i].disconnects, 1);
		assert_int_equal(streams[first + i].unrefs, 1);
		struct test_stream *fresh = &streams[first + count + i];
		assert_int_equal(fresh->connects, 1);
		assert_int_equal(fresh->disconnects, 0);
		assert_int_equal(fresh->unrefs, 0);
		assert_string_equal(fresh->device, device);
	}
}

/* ------------------------------------------------------------------------ */
/* Tests                                                                     */

static void reset_without_monitors_is_noop(void **state)
{
	UNUSED_PARAMETER(state);
	assert_true(obs_set_audio_monitoring_device("Sink A", "sink_a"));
	unsigned contexts = contexts_created;
	unsigned created = streams_created;
	unsigned loops = mainloops_created;
	unsigned freed = mainloops_freed;

	obs_reset_audio_monitoring();
	obs_reset_audio_monitoring();

	assert_int_equal(contexts_created, contexts);
	assert_int_equal(streams_created, created);
	assert_int_equal(mainloops_created, loops);
	assert_int_equal(mainloops_freed, freed);
	assert_device("Sink A", "sink_a");
}

static void reset_resets_each_live_monitor_once(void **state)
{
	UNUSED_PARAMETER(state);
	assert_true(obs_set_audio_monitoring_device("Sink A", "sink_a"));
	unsigned first = streams_created;
	obs_source_t *one = create_monitor();
	obs_source_t *two = create_monitor();
	assert_string_equal(streams[first].device, "sink_a");
	assert_string_equal(streams[first + 1].device, "sink_a");

	obs_reset_audio_monitoring();
	assert_each_reset_once(first, 2, "sink_a");

	obs_reset_audio_monitoring();
	assert_each_reset_once(first + 2, 2, "sink_a");
	assert_device("Sink A", "sink_a");

	destroy_monitor(one);
	destroy_monitor(two);
	assert_int_equal(mainloops_created, mainloops_freed);
}

static void new_device_is_copied_and_resets_monitors(void **state)
{
	UNUSED_PARAMETER(state);
	assert_true(obs_set_audio_monitoring_device("Sink A", "sink_a"));
	unsigned first = streams_created;
	obs_source_t *one = create_monitor();
	obs_source_t *two = create_monitor();

	char name[16] = "Sink B";
	char id[16] = "sink_b";
	assert_true(obs_set_audio_monitoring_device(name, id));
	assert_each_reset_once(first, 2, "sink_b");

	/* The core keeps its own copies, not the caller's storage. */
	const char *got_name = NULL;
	const char *got_id = NULL;
	obs_get_audio_monitoring_device(&got_name, &got_id);
	assert_ptr_not_equal(got_name, name);
	assert_ptr_not_equal(got_id, id);
	snprintf(name, sizeof(name), "Overwritten");
	snprintf(id, sizeof(id), "overwritten");
	assert_device("Sink B", "sink_b");

	/* A later reset uses the stored id. */
	obs_reset_audio_monitoring();
	assert_each_reset_once(first + 2, 2, "sink_b");

	destroy_monitor(one);
	destroy_monitor(two);
	assert_int_equal(mainloops_created, mainloops_freed);
}

static void same_device_keeps_name_and_resets_nothing(void **state)
{
	UNUSED_PARAMETER(state);
	assert_true(obs_set_audio_monitoring_device("Sink A", "sink_a"));
	obs_source_t *monitor = create_monitor();
	obs_source_t *aoc = create_with_device(AOC_SOURCE, "aoc same", "sink_a.monitor", false);
	listen_for_deduplication(true);
	unsigned created = streams_created;

	assert_true(obs_set_audio_monitoring_device("Renamed", "sink_a"));
	assert_device("Sink A", "sink_a");
	assert_int_equal(streams_created, created);
	assert_int_equal(streams[created - 1].disconnects, 0);
	assert_int_equal(streams[created - 1].unrefs, 0);
	assert_int_equal(signals_seen, 0);

	listen_for_deduplication(false);
	obs_source_release(aoc);
	destroy_monitor(monitor);
	assert_int_equal(mainloops_created, mainloops_freed);
}

static void aoc_sources_told_with_their_device_id(void **state)
{
	UNUSED_PARAMETER(state);
	assert_true(obs_set_audio_monitoring_device("Sink A", "sink_a"));
	/* Created in this order: the core enumerates them in it. */
	obs_source_t *aoc_b = create_with_device(AOC_SOURCE, "aoc b", "sink_b.monitor", false);
	obs_source_t *aoc_c = create_with_device(AOC_SOURCE, "aoc c", "sink_c.monitor", false);
	obs_source_t *plain = create_with_device(PLAIN_SOURCE, "plain b", "sink_b.monitor", false);
	obs_source_t *hidden = create_with_device(AOC_SOURCE, "private aoc b", "sink_b.monitor", true);
	listen_for_deduplication(true);

	/* Only the public capture source of the new device starts it. */
	assert_true(obs_set_audio_monitoring_device("Sink B", "sink_b"));
	flush_audio_tasks();
	assert_int_equal(signals_seen, 1);
	assert_ptr_equal(signalled[0], aoc_b);

	/* aoc b no longer matches and was deduplicating: it stops it; then
	 * aoc c, which matches, starts it. */
	signals_seen = 0;
	assert_true(obs_set_audio_monitoring_device("Sink C", "sink_c"));
	flush_audio_tasks();
	assert_int_equal(signals_seen, 2);
	assert_ptr_equal(signalled[0], NULL);
	assert_ptr_equal(signalled[1], aoc_c);

	/* No capture source matches: only the deduplicating one is stopped. */
	signals_seen = 0;
	assert_true(obs_set_audio_monitoring_device("Sink D", "sink_d"));
	flush_audio_tasks();
	assert_int_equal(signals_seen, 1);
	assert_ptr_equal(signalled[0], NULL);

	/* Neither is deduplicating any more: a new id tells nobody. */
	signals_seen = 0;
	assert_true(obs_set_audio_monitoring_device("Sink A", "sink_a"));
	flush_audio_tasks();
	assert_int_equal(signals_seen, 0);

	listen_for_deduplication(false);
	obs_source_release(hidden);
	obs_source_release(plain);
	obs_source_release(aoc_c);
	obs_source_release(aoc_b);
}

int main(void)
{
	long before = bnum_allocs();
	if (!obs_startup("en-US", NULL, NULL))
		return 77;
	struct obs_audio_info audio = {.samples_per_sec = 48000, .speakers = SPEAKERS_STEREO};
	if (!obs_reset_audio(&audio))
		return 1;

	struct obs_source_info monitor_info = {
		.id = MONITOR_SOURCE,
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_AUDIO,
		.get_name = source_name,
		.create = source_create,
		.destroy = source_destroy,
	};
	obs_register_source(&monitor_info);
	struct obs_source_info aoc_info = monitor_info;
	aoc_info.id = AOC_SOURCE;
	aoc_info.output_flags = OBS_SOURCE_AUDIO | OBS_SOURCE_DO_NOT_SELF_MONITOR;
	obs_register_source(&aoc_info);
	struct obs_source_info plain_info = monitor_info;
	plain_info.id = PLAIN_SOURCE;
	obs_register_source(&plain_info);

	const struct CMUnitTest tests[] = {
		cmocka_unit_test(reset_without_monitors_is_noop),
		cmocka_unit_test(reset_resets_each_live_monitor_once),
		cmocka_unit_test(new_device_is_copied_and_resets_monitors),
		cmocka_unit_test(same_device_keeps_name_and_resets_nothing),
		cmocka_unit_test(aoc_sources_told_with_their_device_id),
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
