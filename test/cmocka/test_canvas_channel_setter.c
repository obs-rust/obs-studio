/*
 * GPU-free characterization of obs_canvas_set_channel's signal routing,
 * payload ordering and activation lifetime. The broader caller suite keeps
 * the callback-replacement ownership quirk pinned unchanged.
 */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include "cmocka-compat.h"

#include <obs.h>
#include <stdio.h>
#include <string.h>

#define SOURCE_ID "test_canvas_channel_setter_source"
#define EVENT_CAPACITY 16

struct event {
	char kind;
	obs_canvas_t *canvas;
	long long channel;
	obs_source_t *prev_source;
	obs_source_t *source;
};

struct event_log {
	struct event events[EVENT_CAPACITY];
	size_t count;
	bool overflow;
};

static struct event_log events;
static obs_canvas_t *main_canvas;
static obs_canvas_t *public_canvas;
static obs_source_t *first_source;
static obs_source_t *second_source;
static bool obs_started;

static const char *source_name(void *data)
{
	(void)data;
	return "canvas channel setter test source";
}

static void *source_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	return source;
}

static void source_destroy(void *data)
{
	(void)data;
}

static uint32_t source_size(void *data)
{
	(void)data;
	return 1;
}

static void record_event(char kind, obs_source_t *source)
{
	if (events.count >= EVENT_CAPACITY) {
		events.overflow = true;
		return;
	}
	events.events[events.count++] = (struct event){.kind = kind, .source = source};
}

static void source_activate(void *data)
{
	record_event('A', data);
}

static void source_deactivate(void *data)
{
	record_event('D', data);
}

static void record_signal(char kind, calldata_t *cd)
{
	if (events.count >= EVENT_CAPACITY) {
		events.overflow = true;
		return;
	}
	struct event *event = &events.events[events.count++];
	event->kind = kind;
	event->canvas = calldata_ptr(cd, "canvas");
	event->channel = calldata_int(cd, "channel");
	event->prev_source = calldata_ptr(cd, "prev_source");
	event->source = calldata_ptr(cd, "source");
}

static void local_channel_change(void *data, calldata_t *cd)
{
	(void)data;
	record_signal('L', cd);
}

static void global_channel_change(void *data, calldata_t *cd)
{
	(void)data;
	record_signal('G', cd);
}

static int setup(void **state)
{
	(void)state;
	obs_started = obs_startup("en-US", NULL, NULL);
	if (!obs_started) {
		fprintf(stderr, "obs_startup failed for GPU-free canvas channel test\n");
		return -1;
	}

	struct obs_source_info source_info = {
		.id = SOURCE_ID,
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_VIDEO,
		.get_name = source_name,
		.create = source_create,
		.destroy = source_destroy,
		.get_width = source_size,
		.get_height = source_size,
		.activate = source_activate,
		.deactivate = source_deactivate,
	};
	obs_register_source(&source_info);

	main_canvas = obs_get_main_canvas();
	public_canvas = obs_canvas_create("channel setter public", NULL, 0);
	first_source = obs_source_create_private(SOURCE_ID, "channel setter first", NULL);
	second_source = obs_source_create_private(SOURCE_ID, "channel setter second", NULL);
	assert_non_null(main_canvas);
	assert_non_null(public_canvas);
	assert_non_null(first_source);
	assert_non_null(second_source);

	signal_handler_connect(obs_canvas_get_signal_handler(main_canvas), "channel_change", local_channel_change,
			       NULL);
	signal_handler_connect(obs_get_signal_handler(), "channel_change", global_channel_change, NULL);
	signal_handler_connect(obs_canvas_get_signal_handler(public_canvas), "channel_change", local_channel_change,
			       NULL);
	memset(&events, 0, sizeof(events));
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	if (obs_started) {
		if (main_canvas) {
			obs_canvas_set_channel(main_canvas, 2, NULL);
			signal_handler_disconnect(obs_canvas_get_signal_handler(main_canvas), "channel_change",
						  local_channel_change, NULL);
			obs_canvas_release(main_canvas);
			main_canvas = NULL;
		}
		if (public_canvas) {
			obs_canvas_set_channel(public_canvas, 1, NULL);
			signal_handler_disconnect(obs_canvas_get_signal_handler(public_canvas), "channel_change",
						  local_channel_change, NULL);
			obs_canvas_release(public_canvas);
			public_canvas = NULL;
		}
		signal_handler_disconnect(obs_get_signal_handler(), "channel_change", global_channel_change, NULL);
		if (first_source) {
			obs_source_release(first_source);
			first_source = NULL;
		}
		if (second_source) {
			obs_source_release(second_source);
			second_source = NULL;
		}
		obs_wait_for_destroy_queue();
		obs_shutdown();
		obs_started = false;
	}
	return 0;
}

static void assert_event(size_t index, char kind, obs_canvas_t *canvas, long long channel, obs_source_t *prev_source,
			 obs_source_t *source)
{
	assert_true(index < events.count);
	assert_int_equal(events.events[index].kind, kind);
	if (kind == 'L' || kind == 'G') {
		assert_ptr_equal(events.events[index].canvas, canvas);
		assert_int_equal(events.events[index].channel, channel);
		assert_ptr_equal(events.events[index].prev_source, prev_source);
		assert_ptr_equal(events.events[index].source, source);
	} else {
		assert_ptr_equal(events.events[index].source, source);
	}
}

static void channel_signals_and_activation_follow_the_channel_change(void **state)
{
	(void)state;

	/* Main-view changes emit local then global signals, and only activate the
	 * new source after both handlers have seen the complete payload. */
	obs_canvas_set_channel(main_canvas, 2, first_source);
	assert_false(events.overflow);
	assert_int_equal(events.count, 3);
	assert_event(0, 'L', main_canvas, 2, NULL, first_source);
	assert_event(1, 'G', main_canvas, 2, NULL, first_source);
	assert_event(2, 'A', NULL, 0, NULL, first_source);
	assert_true(obs_source_active(first_source));
	obs_source_t *stored = obs_canvas_get_channel(main_canvas, 2);
	assert_ptr_equal(stored, first_source);
	obs_source_release(stored);

	/* Reinstalling the same source is a reference-only no-op: it emits no
	 * channel signal and does not activate the source again. */
	memset(&events, 0, sizeof(events));
	obs_canvas_set_channel(main_canvas, 2, first_source);
	assert_int_equal(events.count, 0);
	assert_true(obs_source_active(first_source));

	/* Replacement activates the incoming source before deactivating the old
	 * one, after the local and global callbacks have observed the change. */
	obs_canvas_set_channel(main_canvas, 2, second_source);
	assert_false(events.overflow);
	assert_int_equal(events.count, 4);
	assert_event(0, 'L', main_canvas, 2, first_source, second_source);
	assert_event(1, 'G', main_canvas, 2, first_source, second_source);
	assert_event(2, 'A', NULL, 0, NULL, second_source);
	assert_event(3, 'D', NULL, 0, NULL, first_source);
	assert_true(obs_source_active(second_source));
	assert_false(obs_source_active(first_source));

	memset(&events, 0, sizeof(events));
	obs_canvas_set_channel(main_canvas, 2, NULL);
	assert_false(events.overflow);
	assert_int_equal(events.count, 3);
	assert_event(0, 'L', main_canvas, 2, second_source, NULL);
	assert_event(1, 'G', main_canvas, 2, second_source, NULL);
	assert_event(2, 'D', NULL, 0, NULL, second_source);
	assert_false(obs_source_active(second_source));

	/* A non-main canvas still emits its local signal but is not routed through
	 * the global MAIN channel-change signal. */
	memset(&events, 0, sizeof(events));
	obs_canvas_set_channel(public_canvas, 1, first_source);
	assert_false(events.overflow);
	assert_int_equal(events.count, 1);
	assert_event(0, 'L', public_canvas, 1, NULL, first_source);
	assert_false(obs_source_active(first_source));

	memset(&events, 0, sizeof(events));
	obs_canvas_set_channel(public_canvas, 1, NULL);
	assert_false(events.overflow);
	assert_int_equal(events.count, 1);
	assert_event(0, 'L', public_canvas, 1, first_source, NULL);
	assert_false(obs_source_active(first_source));
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(channel_signals_and_activation_follow_the_channel_change),
	};
	return cmocka_run_group_tests(tests, setup, teardown);
}
