/* Characterization of obs_sceneitem_defer_update_begin() and
 * obs_sceneitem_defer_update_end() in libobs/obs-scene.c, through the real
 * libobs, before they move to their own file and then to Rust (#4646).
 *
 * begin() counts one deferral up; end() counts it down and, when the count
 * reaches exactly zero, asks for the item's transform the way a setter does:
 * - an item of a scene has its transform computed at once (one size read of
 *   its source, one item_transform signal of the scene, during end());
 * - an item of a group, or an item with no parent, only has its
 *   update_transform flag set: end() reads nothing and signals nothing, and
 *   the group's next walk computes it.
 * Each NULL call returns after one LOG_DEBUG "Null 'item' parameter" line.
 *
 * test_scene_transform_update_walk covers what the walk does with a deferred
 * item; this test covers the pair itself and runs on libobs and
 * libobs-c-reference alike. obs_startup() needs a display on Linux; without
 * one the cases skip. */

#include <stdarg.h>
#include <stddef.h>
#include <stdint.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <string.h>

#include <obs.h>
#include <util/base.h>

#define SIZE_SOURCE_ID "scene_item_defer_update_source"
#define LOG_MAX 16

static void quiet_log(int lvl, const char *msg, va_list args, void *p)
{
	(void)lvl;
	(void)msg;
	(void)args;
	(void)p;
}

/* The log lines of one call, captured. */
static unsigned log_count;
static int log_level;
static char log_message[256];

static void capture_log(int lvl, const char *msg, va_list args, void *p)
{
	(void)p;
	if (log_count++ == 0) {
		log_level = lvl;
		vsnprintf(log_message, sizeof(log_message), msg, args);
	}
}

/* A source with a fixed size, which counts its size reads. */
struct size_source {
	long width_calls;
	long height_calls;
};

static const char *size_source_name(void *type_data)
{
	(void)type_data;
	return "size source";
}

static void *size_source_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	(void)source;
	return bzalloc(sizeof(struct size_source));
}

static void size_source_destroy(void *data)
{
	bfree(data);
}

static uint32_t size_source_width(void *data)
{
	((struct size_source *)data)->width_calls++;
	return 100;
}

static uint32_t size_source_height(void *data)
{
	((struct size_source *)data)->height_calls++;
	return 50;
}

/* The item_transform signals, in order. */
struct signal_log {
	obs_sceneitem_t *items[LOG_MAX];
	size_t count;
};

static void on_item_transform(void *param, calldata_t *cd)
{
	struct signal_log *log = param;
	obs_sceneitem_t *item = calldata_ptr(cd, "item");

	if (log->count < LOG_MAX)
		log->items[log->count] = item;
	log->count++;
}

static void watch(obs_scene_t *scene, struct signal_log *log)
{
	signal_handler_t *sh = obs_source_get_signal_handler(obs_scene_get_source(scene));
	signal_handler_connect(sh, "item_transform", on_item_transform, log);
}

static void unwatch(obs_scene_t *scene, struct signal_log *log)
{
	signal_handler_t *sh = obs_source_get_signal_handler(obs_scene_get_source(scene));
	signal_handler_disconnect(sh, "item_transform", on_item_transform, log);
}

static int setup(void **state)
{
	(void)state;
	base_set_log_handler(quiet_log, NULL);
	if (!obs_startup("en-US", NULL, NULL)) {
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
		return 0;
	}

	struct obs_source_info info = {
		.id = SIZE_SOURCE_ID,
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_VIDEO,
		.get_name = size_source_name,
		.create = size_source_create,
		.destroy = size_source_destroy,
		.get_width = size_source_width,
		.get_height = size_source_height,
	};
	obs_register_source(&info);
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	if (obs_initialized())
		obs_shutdown();
	return 0;
}

static void require_obs(void)
{
	if (!obs_initialized())
		skip();
}

/* An item of a new size source in scene, settled by one walk. The scene
 * holds the only reference to the source. */
static obs_sceneitem_t *add_item(obs_scene_t *scene, struct size_source **data)
{
	obs_source_t *source = obs_source_create_private(SIZE_SOURCE_ID, "item", NULL);
	obs_sceneitem_t *item = obs_scene_add(scene, source);

	*data = obs_obj_get_data(source);
	obs_source_release(source);
	return item;
}

/* The counters an end() call can move, cleared before the call. */
struct observed {
	struct signal_log log;
	struct size_source *data;
};

static void reset(struct observed *o)
{
	memset(&o->log, 0, sizeof(o->log));
	o->data->width_calls = 0;
	o->data->height_calls = 0;
}

/* No transform asked for: nothing signalled, the size not read. */
static void assert_quiet(const struct observed *o)
{
	assert_int_equal(o->log.count, 0);
	assert_int_equal(o->data->width_calls, 0);
	assert_int_equal(o->data->height_calls, 0);
}

/* The transform computed once, at once: one size read, one signal. */
static void assert_computed(const struct observed *o, obs_sceneitem_t *item)
{
	assert_int_equal(o->log.count, 1);
	assert_ptr_equal(o->log.items[0], item);
	assert_int_equal(o->data->width_calls, 1);
	assert_int_equal(o->data->height_calls, 1);
}

/* A scene with one settled item, watched. */
struct plain {
	obs_scene_t *scene;
	obs_sceneitem_t *item;
	struct observed o;
};

static void build_plain(struct plain *p)
{
	memset(p, 0, sizeof(*p));
	p->scene = obs_scene_create_private("plain");
	p->item = add_item(p->scene, &p->o.data);
	obs_scene_prune_sources(p->scene);
	watch(p->scene, &p->o.log);
	reset(&p->o);
}

static void free_plain(struct plain *p)
{
	unwatch(p->scene, &p->o.log);
	obs_scene_release(p->scene);
}

/* Calls fn(NULL) with the log captured: one LOG_DEBUG line. */
static void expect_null_log(void (*fn)(obs_sceneitem_t *), const char *message)
{
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(capture_log, NULL);
	log_count = 0;
	log_level = -1;
	log_message[0] = '\0';
	fn(NULL);
	unsigned count = log_count;
	int level = log_level;
	base_set_log_handler(old_handler, old_param);

	assert_int_equal(count, 1);
	assert_int_equal(level, LOG_DEBUG);
	assert_string_equal(log_message, message);
}

static void null_item_logs(void **state)
{
	(void)state;
	require_obs();
	expect_null_log(obs_sceneitem_defer_update_begin, "obs_sceneitem_defer_update_begin: Null 'item' parameter");
	expect_null_log(obs_sceneitem_defer_update_end, "obs_sceneitem_defer_update_end: Null 'item' parameter");
}

/* A NULL call changes no item: the next pair on a real item still
 * computes on its end. */
static void null_item_touches_nothing(void **state)
{
	(void)state;
	require_obs();
	struct plain p;
	build_plain(&p);

	base_set_log_handler(quiet_log, NULL);
	obs_sceneitem_defer_update_begin(NULL);
	obs_sceneitem_defer_update_end(NULL);
	obs_sceneitem_defer_update_end(NULL);
	assert_quiet(&p.o);

	obs_sceneitem_defer_update_begin(p.item);
	assert_quiet(&p.o);
	obs_sceneitem_defer_update_end(p.item);
	assert_computed(&p.o, p.item);

	free_plain(&p);
}

/* begin() never asks for a transform; the matching end() of an item of a
 * scene computes it at once, every time. */
static void plain_pair_computes_on_end(void **state)
{
	(void)state;
	require_obs();
	struct plain p;
	build_plain(&p);

	for (int i = 0; i < 3; i++) {
		reset(&p.o);
		obs_sceneitem_defer_update_begin(p.item);
		assert_quiet(&p.o);
		obs_sceneitem_defer_update_end(p.item);
		assert_computed(&p.o, p.item);
	}

	free_plain(&p);
}

/* Nested pairs: only the end() that brings the count back to zero computes,
 * and only once. */
static void nested_pairs_compute_on_outer_end(void **state)
{
	(void)state;
	require_obs();
	struct plain p;
	build_plain(&p);

	obs_sceneitem_defer_update_begin(p.item);
	obs_sceneitem_defer_update_begin(p.item);
	obs_sceneitem_defer_update_begin(p.item);
	assert_quiet(&p.o);

	obs_sceneitem_defer_update_end(p.item);
	assert_quiet(&p.o);
	obs_sceneitem_defer_update_end(p.item);
	assert_quiet(&p.o);

	/* a position setter while still deferred computes nothing */
	struct vec2 pos;
	vec2_set(&pos, 3.0f, 0.0f);
	obs_sceneitem_set_pos(p.item, &pos);
	assert_quiet(&p.o);

	obs_sceneitem_defer_update_end(p.item);
	assert_computed(&p.o, p.item);

	free_plain(&p);
}

/* An end() without a begin() takes the count below zero: it asks for
 * nothing (the count is not zero), and a negative count does not defer, so
 * a setter computes at once. The next begin() brings the count back to zero
 * without asking either, and a pair after that behaves as usual. The count
 * is not clamped. Characterized, not endorsed. */
static void unmatched_end_goes_negative(void **state)
{
	(void)state;
	require_obs();
	struct plain p;
	build_plain(&p);

	obs_sceneitem_defer_update_end(p.item);
	assert_quiet(&p.o);
	obs_sceneitem_defer_update_end(p.item);
	assert_quiet(&p.o);

	struct vec2 pos;
	vec2_set(&pos, 5.0f, 0.0f);
	obs_sceneitem_set_pos(p.item, &pos);
	assert_computed(&p.o, p.item);

	/* -2 -> -1 -> 0: neither begin() asks */
	reset(&p.o);
	obs_sceneitem_defer_update_begin(p.item);
	obs_sceneitem_defer_update_begin(p.item);
	assert_quiet(&p.o);

	obs_sceneitem_defer_update_begin(p.item);
	obs_sceneitem_defer_update_end(p.item);
	assert_computed(&p.o, p.item);

	free_plain(&p);
}

/* An item of a group: end() only sets the update_transform flag. Nothing is
 * read or signalled until the outer scene's walk, which computes the child
 * (and, as the group resizes, more). */
static void group_child_end_sets_flag(void **state)
{
	(void)state;
	require_obs();
	obs_scene_t *scene = obs_scene_create_private("grouped");
	obs_sceneitem_t *group = obs_scene_add_group(scene, "group");
	obs_scene_t *group_scene = obs_sceneitem_group_get_scene(group);
	struct observed o;
	obs_sceneitem_t *child = add_item(group_scene, &o.data);
	obs_scene_prune_sources(scene);

	memset(&o.log, 0, sizeof(o.log));
	watch(scene, &o.log);
	watch(group_scene, &o.log);

	/* settled: a walk computes nothing */
	reset(&o);
	obs_scene_prune_sources(scene);
	assert_int_equal(o.log.count, 0);

	reset(&o);
	obs_sceneitem_defer_update_begin(child);
	obs_sceneitem_defer_update_end(child);
	assert_quiet(&o);

	obs_scene_prune_sources(scene);
	assert_true(o.log.count >= 1);
	assert_ptr_equal(o.log.items[0], child);

	/* the flag was cleared by that walk */
	reset(&o);
	obs_scene_prune_sources(scene);
	assert_int_equal(o.log.count, 0);

	unwatch(group_scene, &o.log);
	unwatch(scene, &o.log);
	obs_scene_release(scene);
}

/* The group item itself is an item of a scene: its end() computes at once,
 * like any other. */
static void group_item_end_computes(void **state)
{
	(void)state;
	require_obs();
	struct signal_log log;
	obs_scene_t *scene = obs_scene_create_private("group owner");
	obs_sceneitem_t *group = obs_scene_add_group(scene, "group");
	obs_scene_prune_sources(scene);

	memset(&log, 0, sizeof(log));
	watch(scene, &log);
	obs_sceneitem_defer_update_begin(group);
	assert_int_equal(log.count, 0);
	obs_sceneitem_defer_update_end(group);
	assert_int_equal(log.count, 1);
	assert_ptr_equal(log.items[0], group);

	unwatch(scene, &log);
	obs_scene_release(scene);
}

/* An item taken out of its scene (no parent) but still referenced: end()
 * sets the flag only, so the source size is not read and the scene it left
 * hears nothing. */
static void parentless_end_reads_nothing(void **state)
{
	(void)state;
	require_obs();
	struct plain p;
	build_plain(&p);

	obs_sceneitem_addref(p.item);
	obs_sceneitem_remove(p.item);
	reset(&p.o);

	obs_sceneitem_defer_update_begin(p.item);
	obs_sceneitem_defer_update_end(p.item);
	assert_quiet(&p.o);

	/* nested on a parentless item: still nothing */
	obs_sceneitem_defer_update_begin(p.item);
	obs_sceneitem_defer_update_begin(p.item);
	obs_sceneitem_defer_update_end(p.item);
	obs_sceneitem_defer_update_end(p.item);
	assert_quiet(&p.o);

	obs_sceneitem_release(p.item);
	free_plain(&p);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(null_item_logs),
		cmocka_unit_test(null_item_touches_nothing),
		cmocka_unit_test(plain_pair_computes_on_end),
		cmocka_unit_test(nested_pairs_compute_on_outer_end),
		cmocka_unit_test(unmatched_end_goes_negative),
		cmocka_unit_test(group_child_end_sets_flag),
		cmocka_unit_test(group_item_end_computes),
		cmocka_unit_test(parentless_end_reads_nothing),
	};

	return cmocka_run_group_tests(tests, setup, teardown);
}
