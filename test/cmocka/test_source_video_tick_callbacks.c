/* Characterize the per-source callback steps of obs_source_video_tick()
 * (#5842): obs_source_deferred_update(), activate_source(),
 * deactivate_source(), show_source() and hide_source(). All of them, and
 * obs_source_video_tick() itself, are hidden, so they are reached here the
 * way users reach them: the core's video thread ticks every source once a
 * frame. The test sets a public source's show and activate counts (its
 * filters' stay 0, so their own ticks never see an edge) or defers an
 * update with obs_source_update(), then waits for what the tick records:
 * - A show, hide, activate or deactivate edge of the source runs the step on
 *   the source, then on its filters from the last entry to the first. Each
 *   step calls the type's callback with the source's data first, then sends
 *   the global source_show, source_hide, source_activate or
 *   source_deactivate (public sources only; the filters here are private)
 *   and then the source's own show, hide, activate or deactivate, each with
 *   the source as "source".
 * - The signals go out even when the type has no callback or the source has
 *   no data (its create returned NULL); only the callback is skipped.
 * - The show edge runs before the activate edge.
 * - A deferred update (obs_source_update() on a video source counts one)
 *   calls the type's update with the source's data and its own settings,
 *   swaps the count to 0 and sends source_update, then update. Without an
 *   update callback, or without data, nothing happens at all: no signal,
 *   and the count stays.
 * - An update deferred again from inside the callback survives: the count
 *   is compared against the value loaded before the callback, so the swap
 *   fails, the count is still 2 when the signals go out, and the next tick
 *   runs the update again and clears it.
 * The core needs a display and OpenGL on Linux (run under xvfb-run); the
 * cases skip without one. Each wait has a generous bound that only turns a
 * hang into a failure. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"
#include "util/platform.h"
#include "util/threading.h"

#define FULL_ID "test_vtc_full"
#define BARE_ID "test_vtc_bare"
#define NO_DATA_ID "test_vtc_no_data"
#define FILTER_ID "test_vtc_filter"

#define MAX_EVENTS 64
#define WAIT_MS 10000

/* One thing that happened on the video thread: the callback ("cb:show"),
 * the global signal ("g:source_show") or the source's signal ("l:show");
 * the source (for a callback, its data, which is the source itself); and,
 * for the update events, the deferred count at that moment. */
struct event {
	char what[32];
	const void *source;
	long count;
};

static pthread_mutex_t event_mutex;
static struct event events[MAX_EVENTS];
static size_t event_count;
static volatile long ticks;
static bool obs_up;

/* The update callback's settings, and whether it defers again once. */
static const void *update_settings;
static volatile bool redefer;

static void record(const char *prefix, const char *name, const void *source, long count)
{
	pthread_mutex_lock(&event_mutex);
	if (event_count < MAX_EVENTS) {
		snprintf(events[event_count].what, sizeof(events[event_count].what), "%s:%s", prefix, name);
		events[event_count].source = source;
		events[event_count].count = count;
	}
	event_count++;
	pthread_mutex_unlock(&event_mutex);
}

static size_t recorded(void)
{
	pthread_mutex_lock(&event_mutex);
	size_t count = event_count;
	pthread_mutex_unlock(&event_mutex);
	return count;
}

static void reset_events(void)
{
	pthread_mutex_lock(&event_mutex);
	memset(events, 0, sizeof(events));
	event_count = 0;
	pthread_mutex_unlock(&event_mutex);
}

static void on_tick(void *param, float seconds)
{
	(void)param;
	(void)seconds;
	os_atomic_inc_long(&ticks);
}

/* Waits until n more ticks have begun: the tick callbacks run before the
 * sources tick, so every source has then been ticked at least once since
 * the call. */
static void wait_ticks(long n)
{
	long start = os_atomic_load_long(&ticks);
	for (int ms = 0; os_atomic_load_long(&ticks) < start + n + 1; ms++) {
		assert_true(ms < WAIT_MS);
		os_sleep_ms(1);
	}
}

/* Waits for n events, then two more ticks to show nothing else follows. */
static void wait_events(size_t n)
{
	for (int ms = 0; recorded() < n; ms++) {
		if (ms >= WAIT_MS)
			fail_msg("waited %d ms for %zu events, have %zu", WAIT_MS, n, recorded());
		os_sleep_ms(1);
	}
	wait_ticks(2);
	assert_int_equal(recorded(), n);
}

static const char *type_name(void *unused)
{
	(void)unused;
	return "video tick callbacks";
}

/* The source's data is the source itself, so a callback names its source. */
static void *source_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	return source;
}

static void *no_data_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	(void)source;
	return NULL;
}

static void source_destroy(void *data)
{
	(void)data;
}

/* A video source type must have these to register. */
static void source_render(void *data, gs_effect_t *effect)
{
	(void)data;
	(void)effect;
}

static uint32_t source_size(void *data)
{
	(void)data;
	return 4;
}

static void on_update(void *data, obs_data_t *settings)
{
	obs_source_t *source = data;
	record("cb", "update", data, os_atomic_load_long(&source->defer_update_count));
	update_settings = settings;
	if (redefer) {
		redefer = false;
		obs_source_update(source, NULL);
	}
}

static void on_activate(void *data)
{
	record("cb", "activate", data, 0);
}

static void on_deactivate(void *data)
{
	record("cb", "deactivate", data, 0);
}

static void on_show(void *data)
{
	record("cb", "show", data, 0);
}

static void on_hide(void *data)
{
	record("cb", "hide", data, 0);
}

static long count_of(const char *name, obs_source_t *source)
{
	return strstr(name, "update") ? os_atomic_load_long(&source->defer_update_count) : 0;
}

static void on_global(void *param, calldata_t *data)
{
	obs_source_t *source = calldata_ptr(data, "source");
	record("g", param, source, count_of(param, source));
}

static void on_local(void *param, calldata_t *data)
{
	obs_source_t *source = calldata_ptr(data, "source");
	record("l", param, source, count_of(param, source));
}

static const char *const global_signals[] = {"source_update", "source_activate", "source_deactivate", "source_show",
					     "source_hide"};
static const char *const local_signals[] = {"update", "activate", "deactivate", "show", "hide"};

static int group_setup(void **state)
{
	(void)state;
	assert_int_equal(pthread_mutex_init(&event_mutex, NULL), 0);
	obs_up = obs_startup("en-US", NULL, NULL);
	if (!obs_up) {
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
		return 0;
	}
	struct obs_video_info ovi = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = 60,
		.fps_den = 1,
		.base_width = 64,
		.base_height = 64,
		.output_width = 64,
		.output_height = 64,
		.output_format = VIDEO_FORMAT_NV12,
		.colorspace = VIDEO_CS_709,
		.range = VIDEO_RANGE_PARTIAL,
		.scale_type = OBS_SCALE_BICUBIC,
	};
	int ret = obs_reset_video(&ovi);
	if (ret != OBS_VIDEO_SUCCESS) {
		fprintf(stderr, "skipped: obs_reset_video failed: %d\n", ret);
		obs_shutdown();
		obs_up = false;
		return 0;
	}
	struct obs_source_info full = {
		.id = FULL_ID,
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_VIDEO,
		.get_name = type_name,
		.create = source_create,
		.destroy = source_destroy,
		.video_render = source_render,
		.get_width = source_size,
		.get_height = source_size,
		.update = on_update,
		.activate = on_activate,
		.deactivate = on_deactivate,
		.show = on_show,
		.hide = on_hide,
	};
	struct obs_source_info bare = {
		.id = BARE_ID,
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_VIDEO,
		.get_name = type_name,
		.create = source_create,
		.destroy = source_destroy,
		.video_render = source_render,
		.get_width = source_size,
		.get_height = source_size,
	};
	struct obs_source_info no_data = full;
	no_data.id = NO_DATA_ID;
	no_data.create = no_data_create;
	struct obs_source_info filter = full;
	filter.id = FILTER_ID;
	filter.type = OBS_SOURCE_TYPE_FILTER;
	obs_register_source(&full);
	obs_register_source(&bare);
	obs_register_source(&no_data);
	obs_register_source(&filter);
	signal_handler_t *core = obs_get_signal_handler();
	for (size_t i = 0; i < sizeof(global_signals) / sizeof(global_signals[0]); i++)
		signal_handler_connect(core, global_signals[i], on_global, (void *)global_signals[i]);
	obs_add_tick_callback(on_tick, NULL);
	return 0;
}

static int group_teardown(void **state)
{
	(void)state;
	if (obs_up) {
		obs_remove_tick_callback(on_tick, NULL);
		signal_handler_t *core = obs_get_signal_handler();
		for (size_t i = 0; i < sizeof(global_signals) / sizeof(global_signals[0]); i++)
			signal_handler_disconnect(core, global_signals[i], on_global, (void *)global_signals[i]);
		obs_shutdown();
	}
	pthread_mutex_destroy(&event_mutex);
	return 0;
}

static bool skip_without_core(void)
{
	if (!obs_up) {
		skip();
		return true;
	}
	return false;
}

static void connect_local(obs_source_t *source)
{
	signal_handler_t *handler = obs_source_get_signal_handler(source);
	for (size_t i = 0; i < sizeof(local_signals) / sizeof(local_signals[0]); i++)
		signal_handler_connect(handler, local_signals[i], on_local, (void *)local_signals[i]);
}

static obs_source_t *make(const char *id, const char *name, bool is_private)
{
	obs_source_t *source = is_private ? obs_source_create_private(id, name, NULL)
					  : obs_source_create(id, name, NULL, NULL);
	assert_non_null(source);
	assert_ptr_equal(source->context.data, strcmp(id, NO_DATA_ID) == 0 ? NULL : source);
	assert_int_equal(os_atomic_load_long(&source->defer_update_count), 0);
	connect_local(source);
	return source;
}

/* A public source with two private filters; the step walks filters[1]
 * first. */
struct fixture {
	obs_source_t *source;
	obs_source_t *filters[2];
};

static void make_fixture(struct fixture *f, const char *id)
{
	f->source = make(id, "source", false);
	/* obs_source_filter_add() puts each new filter first. */
	f->filters[1] = make(FILTER_ID, "second filter", true);
	obs_source_filter_add(f->source, f->filters[1]);
	f->filters[0] = make(FILTER_ID, "first filter", true);
	obs_source_filter_add(f->source, f->filters[0]);
	assert_int_equal(f->source->filters.num, 2);
	assert_ptr_equal(f->source->filters.array[0], f->filters[0]);
	assert_ptr_equal(f->source->filters.array[1], f->filters[1]);
	redefer = false;
	update_settings = NULL;
	wait_ticks(2);
	reset_events();
}

static void free_fixture(struct fixture *f)
{
	/* Settle the flags first, so nothing is left showing or active. */
	os_atomic_set_long(&f->source->show_refs, 0);
	os_atomic_set_long(&f->source->activate_refs, 0);
	os_atomic_set_long(&f->source->defer_update_count, 0);
	wait_ticks(2);
	for (size_t i = 0; i < 2; i++) {
		obs_source_filter_remove(f->source, f->filters[i]);
		obs_source_release(f->filters[i]);
	}
	obs_source_remove(f->source);
	obs_source_release(f->source);
	obs_wait_for_destroy_queue();
	reset_events();
}

static void expect_event(size_t i, const char *what, const void *source)
{
	assert_true(i < recorded());
	assert_string_equal(events[i].what, what);
	assert_ptr_equal(events[i].source, source);
}

/* What one step records for `source`, from index i: the callback (unless
 * skipped), the global signal (public sources only), the local one.
 * Returns the next index. */
static size_t expect_step(size_t i, const char *step, const void *source, bool callback, bool global)
{
	char what[32];
	if (callback) {
		snprintf(what, sizeof(what), "cb:%s", step);
		expect_event(i++, what, source);
	}
	if (global) {
		snprintf(what, sizeof(what), "g:source_%s", step);
		expect_event(i++, what, source);
	}
	snprintf(what, sizeof(what), "l:%s", step);
	expect_event(i++, what, source);
	return i;
}

/* The source's step, then filters[1]'s, then filters[0]'s. */
static size_t expect_edge(size_t i, const struct fixture *f, const char *step, bool source_callback)
{
	i = expect_step(i, step, f->source, source_callback, true);
	i = expect_step(i, step, f->filters[1], true, false);
	return expect_step(i, step, f->filters[0], true, false);
}

/* Events of one edge: 3 for the public source with its callback (2
 * without), 2 for each private filter. */
static size_t edge_events(bool source_callback)
{
	return (source_callback ? 3 : 2) + 2 + 2;
}

/* The update's three events, the count each saw. */
static size_t expect_update(size_t i, const void *source, long at_callback, long at_signals)
{
	expect_event(i, "cb:update", source);
	assert_int_equal(events[i].count, at_callback);
	expect_event(i + 1, "g:source_update", source);
	assert_int_equal(events[i + 1].count, at_signals);
	expect_event(i + 2, "l:update", source);
	assert_int_equal(events[i + 2].count, at_signals);
	return i + 3;
}

static void show_then_hide(void **state)
{
	(void)state;
	if (skip_without_core())
		return;
	struct fixture f;
	make_fixture(&f, FULL_ID);

	os_atomic_set_long(&f.source->show_refs, 1);
	wait_events(edge_events(true));
	expect_edge(0, &f, "show", true);
	assert_true(f.source->showing);
	assert_false(f.source->active);

	reset_events();
	os_atomic_set_long(&f.source->show_refs, 0);
	wait_events(edge_events(true));
	expect_edge(0, &f, "hide", true);
	assert_false(f.source->showing);

	free_fixture(&f);
}

static void activate_then_deactivate(void **state)
{
	(void)state;
	if (skip_without_core())
		return;
	struct fixture f;
	make_fixture(&f, FULL_ID);

	os_atomic_set_long(&f.source->activate_refs, 2);
	wait_events(edge_events(true));
	expect_edge(0, &f, "activate", true);
	assert_true(f.source->active);
	assert_false(f.source->showing);

	reset_events();
	os_atomic_set_long(&f.source->activate_refs, 0);
	wait_events(edge_events(true));
	expect_edge(0, &f, "deactivate", true);
	assert_false(f.source->active);

	free_fixture(&f);
}

/* Set together, the show edge runs first (in the same tick, or the activate
 * edge in the next one when the tick read the show count in between). */
static void show_edge_runs_before_activate_edge(void **state)
{
	(void)state;
	if (skip_without_core())
		return;
	struct fixture f;
	make_fixture(&f, FULL_ID);

	os_atomic_set_long(&f.source->show_refs, 1);
	os_atomic_set_long(&f.source->activate_refs, 1);
	wait_events(2 * edge_events(true));
	expect_edge(expect_edge(0, &f, "show", true), &f, "activate", true);

	reset_events();
	os_atomic_set_long(&f.source->show_refs, 0);
	os_atomic_set_long(&f.source->activate_refs, 0);
	wait_events(2 * edge_events(true));
	expect_edge(expect_edge(0, &f, "hide", true), &f, "deactivate", true);

	free_fixture(&f);
}

/* A source whose type has no callbacks, or with no data, still sends every
 * signal. */
static void signals_without_callback_or_data(void **state)
{
	(void)state;
	if (skip_without_core())
		return;
	const char *const ids[] = {BARE_ID, NO_DATA_ID};
	for (size_t k = 0; k < 2; k++) {
		struct fixture f;
		make_fixture(&f, ids[k]);

		os_atomic_set_long(&f.source->show_refs, 1);
		os_atomic_set_long(&f.source->activate_refs, 1);
		wait_events(2 * edge_events(false));
		expect_edge(expect_edge(0, &f, "show", false), &f, "activate", false);

		reset_events();
		os_atomic_set_long(&f.source->show_refs, 0);
		os_atomic_set_long(&f.source->activate_refs, 0);
		wait_events(2 * edge_events(false));
		expect_edge(expect_edge(0, &f, "hide", false), &f, "deactivate", false);

		free_fixture(&f);
	}
}

static void deferred_update_runs_and_clears(void **state)
{
	(void)state;
	if (skip_without_core())
		return;
	struct fixture f;
	make_fixture(&f, FULL_ID);

	obs_source_update(f.source, NULL);
	wait_events(3);
	expect_update(0, f.source, 1, 0);
	assert_ptr_equal(update_settings, f.source->context.settings);
	assert_int_equal(os_atomic_load_long(&f.source->defer_update_count), 0);

	free_fixture(&f);
}

/* Without an update callback, or without data, nothing happens, not even
 * the signal, and the count stays. */
static void deferred_update_skipped(void **state)
{
	(void)state;
	if (skip_without_core())
		return;
	const char *const ids[] = {BARE_ID, NO_DATA_ID};
	for (size_t k = 0; k < 2; k++) {
		struct fixture f;
		make_fixture(&f, ids[k]);

		obs_source_update(f.source, NULL);
		wait_ticks(3);
		assert_int_equal(recorded(), 0);
		assert_int_equal(os_atomic_load_long(&f.source->defer_update_count), 1);

		free_fixture(&f);
	}
}

/* An update deferred from inside the callback is not lost. */
static void update_deferred_again_survives(void **state)
{
	(void)state;
	if (skip_without_core())
		return;
	struct fixture f;
	make_fixture(&f, FULL_ID);

	redefer = true;
	obs_source_update(f.source, NULL);
	wait_events(6);
	assert_false(redefer);
	/* Swapped against 1 while the count was 2: it stays for the signals. */
	size_t i = expect_update(0, f.source, 1, 2);
	/* The next tick runs it again and clears it. */
	expect_update(i, f.source, 2, 0);
	assert_int_equal(os_atomic_load_long(&f.source->defer_update_count), 0);

	free_fixture(&f);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(show_then_hide),
		cmocka_unit_test(activate_then_deactivate),
		cmocka_unit_test(show_edge_runs_before_activate_edge),
		cmocka_unit_test(signals_without_callback_or_data),
		cmocka_unit_test(deferred_update_runs_and_clears),
		cmocka_unit_test(deferred_update_skipped),
		cmocka_unit_test(update_deferred_again_survives),
	};
	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
