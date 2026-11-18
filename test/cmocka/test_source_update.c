/* Actual-library characterization of obs_source_update before extraction.
 * Settings merge into the source's own settings object before any dispatch.
 * A video source only counts a deferred update; the next video tick runs the
 * callback once with the merged settings. Any other source with data and an
 * update callback runs it synchronously, then signals the global
 * "source_update" (public sources only) and the local "update". */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <string.h>
#include <obs-internal.h>
#include <util/platform.h>
#include <util/threading.h>

#define VIDEO_ID "test_source_update_video"
#define AUDIO_ID "test_source_update_audio"
#define PLAIN_ID "test_source_update_plain"
#define EMPTY_ID "test_source_update_empty"

struct trace {
	pthread_mutex_t mutex;
	obs_source_t *source;
	obs_data_t *settings;
	char events[32];
	size_t count;
	size_t updates;
	long long seen_a;
	long long seen_b;
	long long seen_c;
	int reenter;
	os_event_t *updated;
	size_t ticks;
	os_event_t *ticked;
};

static struct trace trace;

static void record(char event)
{
	assert_true(trace.count + 1 < sizeof(trace.events));
	trace.events[trace.count++] = event;
	trace.events[trace.count] = '\0';
}

static void reset_trace(obs_source_t *source)
{
	pthread_mutex_lock(&trace.mutex);
	trace.source = source;
	trace.settings = NULL;
	trace.events[0] = '\0';
	trace.count = 0;
	trace.updates = 0;
	trace.seen_a = trace.seen_b = trace.seen_c = -1;
	trace.reenter = 0;
	pthread_mutex_unlock(&trace.mutex);
}

static const char *source_name(void *type_data)
{
	return type_data;
}

static void *source_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	return source;
}

static void *source_create_empty(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	(void)source;
	return NULL;
}

static void source_destroy(void *data)
{
	(void)data;
}

static uint32_t source_size(void *data)
{
	(void)data;
	return 32;
}

static void source_update(void *data, obs_data_t *settings)
{
	pthread_mutex_lock(&trace.mutex);
	assert_ptr_equal(data, trace.source);
	trace.settings = settings;
	trace.updates++;
	/* The merged values are already visible to the callback. */
	trace.seen_a = obs_data_get_int(settings, "a");
	trace.seen_b = obs_data_get_int(settings, "b");
	trace.seen_c = obs_data_get_int(settings, "c");
	record('U');
	bool reenter = trace.reenter > 0;
	if (reenter)
		trace.reenter--;
	pthread_mutex_unlock(&trace.mutex);

	if (reenter) {
		obs_data_t *inner = obs_data_create();
		obs_data_set_int(inner, "c", 30);
		obs_source_update(data, inner);
		obs_data_release(inner);
		pthread_mutex_lock(&trace.mutex);
		/* The nested call merged, called back and signalled before returning. */
		assert_int_equal(obs_data_get_int(settings, "c"), 30);
		record(')');
		pthread_mutex_unlock(&trace.mutex);
	}
}

static void on_signal(void *param, calldata_t *data)
{
	char event = *(const char *)param;
	pthread_mutex_lock(&trace.mutex);
	if (calldata_ptr(data, "source") == trace.source) {
		record(event);
		if (event == 'L' && trace.updated)
			os_event_signal(trace.updated);
	}
	pthread_mutex_unlock(&trace.mutex);
}

static const char global_event = 'G';
static const char local_event = 'L';

static void connect_signals(obs_source_t *source)
{
	signal_handler_connect(obs_get_signal_handler(), "source_update", on_signal, (void *)&global_event);
	signal_handler_connect(obs_source_get_signal_handler(source), "update", on_signal, (void *)&local_event);
}

static void disconnect_signals(obs_source_t *source)
{
	signal_handler_disconnect(obs_source_get_signal_handler(source), "update", on_signal, (void *)&local_event);
	signal_handler_disconnect(obs_get_signal_handler(), "source_update", on_signal, (void *)&global_event);
}

static void register_type(const char *id, uint32_t flags, bool has_update, bool has_data)
{
	struct obs_source_info info = {
		.id = id,
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = flags,
		.get_name = source_name,
		.create = has_data ? source_create : source_create_empty,
		.destroy = source_destroy,
		.update = has_update ? source_update : NULL,
		.type_data = (void *)id,
	};
	/* Registration requires a size for a video source. */
	if (flags & OBS_SOURCE_VIDEO) {
		info.get_width = source_size;
		info.get_height = source_size;
	}
	obs_register_source(&info);
	/* A rejected registration fails here, not as a missing source later. */
	assert_non_null(obs_source_get_display_name(id));
}

static int setup(void **state)
{
	(void)state;
	/* A startup failure is a failing characterization, never a passing skip. */
	assert_int_equal(pthread_mutex_init(&trace.mutex, NULL), 0);
	assert_true(obs_startup("en-US", NULL, NULL));
	register_type(VIDEO_ID, OBS_SOURCE_VIDEO, true, true);
	register_type(AUDIO_ID, OBS_SOURCE_AUDIO, true, true);
	register_type(PLAIN_ID, 0, false, true);
	register_type(EMPTY_ID, 0, true, false);
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	obs_wait_for_destroy_queue();
	obs_shutdown();
	pthread_mutex_destroy(&trace.mutex);
	return 0;
}

static obs_source_t *create(const char *id, bool private)
{
	obs_data_t *initial = obs_data_create();
	obs_data_set_int(initial, "a", 1);
	obs_data_set_int(initial, "b", 1);
	obs_source_t *source = private ? obs_source_create_private(id, id, initial)
				       : obs_source_create(id, id, initial, NULL);
	obs_data_release(initial);
	assert_non_null(source);
	return source;
}

static obs_data_t *patch(void)
{
	obs_data_t *settings = obs_data_create();
	obs_data_set_int(settings, "b", 2);
	obs_data_set_int(settings, "c", 3);
	return settings;
}

static void check_values(obs_source_t *source, long long a, long long b, long long c)
{
	assert_int_equal(obs_data_get_int(source->context.settings, "a"), a);
	assert_int_equal(obs_data_get_int(source->context.settings, "b"), b);
	assert_int_equal(obs_data_get_int(source->context.settings, "c"), c);
}

static void null_source(void **state)
{
	(void)state;
	obs_data_t *settings = patch();
	obs_source_update(NULL, settings);
	obs_source_update(NULL, NULL);
	/* The caller's object is not touched. */
	assert_int_equal(obs_data_get_int(settings, "b"), 2);
	assert_false(obs_data_has_user_value(settings, "a"));
	obs_data_release(settings);
}

static void sync_update(bool private, const char *events)
{
	obs_source_t *source = create(AUDIO_ID, private);
	obs_data_t *own = source->context.settings;
	reset_trace(source);
	connect_signals(source);

	obs_data_t *settings = patch();
	obs_source_update(source, settings);
	assert_int_equal(trace.updates, 1);
	/* The callback gets the source's own object, not the caller's. */
	assert_ptr_equal(trace.settings, own);
	assert_ptr_not_equal(trace.settings, settings);
	assert_int_equal(trace.seen_a, 1);
	assert_int_equal(trace.seen_b, 2);
	assert_int_equal(trace.seen_c, 3);
	/* Callback first, then the global and local signals. */
	assert_string_equal(trace.events, events);
	assert_ptr_equal(source->context.settings, own);
	check_values(source, 1, 2, 3);
	assert_int_equal(os_atomic_load_long(&source->defer_update_count), 0);
	/* The caller's object is applied, not changed. */
	assert_false(obs_data_has_user_value(settings, "a"));
	obs_data_release(settings);

	/* NULL settings still dispatch, with the values unchanged. */
	reset_trace(source);
	obs_source_update(source, NULL);
	assert_int_equal(trace.updates, 1);
	assert_ptr_equal(trace.settings, own);
	assert_string_equal(trace.events, events);
	assert_ptr_equal(source->context.settings, own);
	check_values(source, 1, 2, 3);

	disconnect_signals(source);
	obs_source_release(source);
}

static void public_sync_order(void **state)
{
	(void)state;
	sync_update(false, "UGL");
}

static void private_sync_order(void **state)
{
	(void)state;
	/* A private source skips the global signal. */
	sync_update(true, "UL");
}

static void guarded_without_dispatch(const char *id)
{
	obs_source_t *source = create(id, false);
	obs_data_t *own = source->context.settings;
	reset_trace(source);
	connect_signals(source);

	obs_data_t *settings = patch();
	obs_source_update(source, settings);
	obs_source_update(source, NULL);
	obs_data_release(settings);
	/* Merged anyway, but no callback and no signal. */
	assert_int_equal(trace.updates, 0);
	assert_string_equal(trace.events, "");
	assert_ptr_equal(source->context.settings, own);
	check_values(source, 1, 2, 3);
	assert_int_equal(os_atomic_load_long(&source->defer_update_count), 0);

	disconnect_signals(source);
	obs_source_release(source);
}

static void no_callback_no_dispatch(void **state)
{
	(void)state;
	guarded_without_dispatch(PLAIN_ID);
}

static void no_data_no_dispatch(void **state)
{
	(void)state;
	guarded_without_dispatch(EMPTY_ID);
}

static void reentrant_update(void **state)
{
	(void)state;
	obs_source_t *source = create(AUDIO_ID, false);
	reset_trace(source);
	connect_signals(source);
	pthread_mutex_lock(&trace.mutex);
	trace.reenter = 1;
	pthread_mutex_unlock(&trace.mutex);

	obs_data_t *settings = patch();
	obs_source_update(source, settings);
	obs_data_release(settings);
	/* The nested update runs to completion inside the outer callback,
	 * then the outer call signals. */
	assert_int_equal(trace.updates, 2);
	assert_string_equal(trace.events, "UUGL)GL");
	assert_int_equal(trace.seen_c, 30);
	check_values(source, 1, 2, 30);
	assert_int_equal(os_atomic_load_long(&source->defer_update_count), 0);

	disconnect_signals(source);
	obs_source_release(source);
}

static obs_source_t *deferred;

static void video_defers(void **state)
{
	(void)state;
	/* No video is running yet: nothing ticks the source here. */
	obs_source_t *source = create(VIDEO_ID, false);
	obs_data_t *own = source->context.settings;
	reset_trace(source);
	connect_signals(source);
	assert_int_equal(os_atomic_load_long(&source->defer_update_count), 0);

	obs_data_t *settings = patch();
	obs_source_update(source, settings);
	obs_data_release(settings);
	/* Merged now, dispatched later. */
	assert_int_equal(trace.updates, 0);
	assert_string_equal(trace.events, "");
	assert_ptr_equal(source->context.settings, own);
	check_values(source, 1, 2, 3);
	assert_int_equal(os_atomic_load_long(&source->defer_update_count), 1);

	obs_source_update(source, NULL);
	assert_int_equal(trace.updates, 0);
	assert_string_equal(trace.events, "");
	check_values(source, 1, 2, 3);
	assert_int_equal(os_atomic_load_long(&source->defer_update_count), 2);

	deferred = source;
}

static void count_tick(void *param, float seconds)
{
	(void)param;
	(void)seconds;
	pthread_mutex_lock(&trace.mutex);
	trace.ticks++;
	if (trace.ticked)
		os_event_signal(trace.ticked);
	pthread_mutex_unlock(&trace.mutex);
}

static void wait_ticks(size_t target)
{
	for (;;) {
		pthread_mutex_lock(&trace.mutex);
		size_t ticks = trace.ticks;
		pthread_mutex_unlock(&trace.mutex);
		if (ticks >= target)
			return;
		/* The bound only turns a hang into a failure. */
		if (os_event_timedwait(trace.ticked, 30000) != 0) {
			fprintf(stderr, "video ticked %zu of %zu times\n", ticks, target);
			fail();
		}
	}
}

static void video_tick_runs_deferred(void **state)
{
	(void)state;
	obs_source_t *source = deferred;
	assert_non_null(source);
	obs_data_t *own = source->context.settings;
	assert_int_equal(os_event_init(&trace.updated, OS_EVENT_TYPE_MANUAL), 0);
	assert_int_equal(os_event_init(&trace.ticked, OS_EVENT_TYPE_AUTO), 0);
	obs_add_tick_callback(count_tick, NULL);

	struct obs_video_info video = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = 30,
		.fps_den = 1,
		.base_width = 32,
		.base_height = 32,
		.output_width = 32,
		.output_height = 32,
		.output_format = VIDEO_FORMAT_NV12,
		.colorspace = VIDEO_CS_709,
		.range = VIDEO_RANGE_PARTIAL,
		.scale_type = OBS_SCALE_BICUBIC,
	};
	assert_int_equal(obs_reset_video(&video), OBS_VIDEO_SUCCESS);
	/* The local signal follows the callback; the bound only turns a hang
	 * into a failure. */
	assert_int_equal(os_event_timedwait(trace.updated, 30000), 0);

	/* Tick callbacks run before the sources tick, so two more ticks prove a
	 * whole later frame ticked the source again. */
	pthread_mutex_lock(&trace.mutex);
	size_t ticks = trace.ticks;
	pthread_mutex_unlock(&trace.mutex);
	wait_ticks(ticks + 2);

	pthread_mutex_lock(&trace.mutex);
	/* Two queued updates ran once, with the source's merged settings. */
	assert_int_equal(trace.updates, 1);
	assert_ptr_equal(trace.settings, own);
	assert_int_equal(trace.seen_a, 1);
	assert_int_equal(trace.seen_b, 2);
	assert_int_equal(trace.seen_c, 3);
	assert_string_equal(trace.events, "UGL");
	pthread_mutex_unlock(&trace.mutex);
	assert_int_equal(os_atomic_load_long(&source->defer_update_count), 0);

	obs_remove_tick_callback(count_tick, NULL);
	disconnect_signals(source);
	pthread_mutex_lock(&trace.mutex);
	trace.source = NULL;
	pthread_mutex_unlock(&trace.mutex);
	obs_source_release(source);
	deferred = NULL;
	os_event_destroy(trace.updated);
	os_event_destroy(trace.ticked);
	trace.updated = NULL;
	trace.ticked = NULL;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(null_source),
		cmocka_unit_test(public_sync_order),
		cmocka_unit_test(private_sync_order),
		cmocka_unit_test(no_callback_no_dispatch),
		cmocka_unit_test(no_data_no_dispatch),
		cmocka_unit_test(reentrant_update),
		cmocka_unit_test(video_defers),
		/* Last: it starts video, which ticks every live source. */
		cmocka_unit_test(video_tick_runs_deferred),
	};
	return cmocka_run_group_tests(tests, setup, teardown);
}
