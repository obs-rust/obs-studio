/* Characterization of obs_source_duplicate() (#5805), against the hybrid
 * libobs and the C reference through the real core.
 *
 * - A NULL source logs the validity check's line once and returns NULL.
 * - A scene duplicated public is shared: the same source with one new
 *   reference. Duplicated private it is copied with obs_scene_duplicate()
 *   (OBS_SCENE_DUP_PRIVATE_COPY): a new private scene named new_name whose
 *   items' sources are copies. The copy's source comes back with only the
 *   scene's own reference.
 * - A group is always copied: public with OBS_SCENE_DUP_COPY (a public group
 *   copy keeps its items' sources), private with
 *   OBS_SCENE_DUP_PRIVATE_COPY (they are copied).
 * - A source of a type marked OBS_SOURCE_DO_NOT_DUPLICATE is shared: the
 *   same source with one new reference.
 * - Any other source is created anew with its id and new_name, public with
 *   obs_source_create() or private with obs_source_create_private(), from a
 *   copy of its settings (a separate object). The audio mixers, sync offset,
 *   user volume, user mute, volume, mute and flags are copied, the private
 *   settings applied, and its filters duplicated onto the copy (private,
 *   with their settings and enabled state). A filter is duplicated the
 *   same way. The copy comes back with one reference.
 *
 * The field values are written and read through obs-internal.h so no audio
 * action or signal changes them. obs_startup() needs a display on Linux
 * (run under xvfb-run); the cases skip when it cannot start. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <string.h>

#include <obs.h>
#include "obs-internal.h"
#include <util/base.h>
#include <util/dstr.h>

#define BOX_ID "source_duplicate_box"
#define SHARED_ID "source_duplicate_shared"
#define FILTER_ID "source_duplicate_filter"

static bool obs_up;

/* The LOG_DEBUG lines naming obs_source_duplicate, while capturing. */
static bool capturing;
static struct dstr captured;
static int captured_lines;

static void log_handler(int lvl, const char *msg, va_list args, void *p)
{
	(void)p;
	if (capturing) {
		char line[256];
		va_list copy;
		va_copy(copy, args);
		vsnprintf(line, sizeof(line), msg, copy);
		va_end(copy);
		if (lvl == LOG_DEBUG && strstr(line, "obs_source_duplicate")) {
			if (captured.len)
				dstr_cat(&captured, "|");
			dstr_cat(&captured, line);
			captured_lines++;
		}
	}
	if (lvl > LOG_WARNING)
		return;
	vfprintf(stderr, msg, args);
	fprintf(stderr, "\n");
}

static const char *box_name(void *type_data)
{
	(void)type_data;
	return "box";
}

static void *data_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	return source;
}

static void data_destroy(void *data)
{
	(void)data;
}

static uint32_t box_size(void *data)
{
	(void)data;
	return 64;
}

static void register_types(void)
{
	struct obs_source_info box = {
		.id = BOX_ID,
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_VIDEO,
		.get_name = box_name,
		.create = data_create,
		.destroy = data_destroy,
		.get_width = box_size,
		.get_height = box_size,
	};
	struct obs_source_info shared = box;
	shared.id = SHARED_ID;
	shared.output_flags = OBS_SOURCE_VIDEO | OBS_SOURCE_DO_NOT_DUPLICATE;
	struct obs_source_info filter = {
		.id = FILTER_ID,
		.type = OBS_SOURCE_TYPE_FILTER,
		.output_flags = OBS_SOURCE_VIDEO,
		.get_name = box_name,
		.create = data_create,
		.destroy = data_destroy,
	};
	obs_register_source(&box);
	obs_register_source(&shared);
	obs_register_source(&filter);
}

static bool skip_without_core(void)
{
	if (!obs_up) {
		skip();
		return true;
	}
	return false;
}

/* The caller's reference is the only one left: one release destroys the
 * source. */
static void release_last(obs_source_t *source)
{
	obs_weak_source_t *weak = obs_source_get_weak_source(source);
	assert_false(obs_weak_source_expired(weak));
	obs_source_release(source);
	assert_true(obs_weak_source_expired(weak));
	obs_weak_source_release(weak);
}

/* One release leaves the source alive (another reference is held). */
static void release_not_last(obs_source_t *source)
{
	obs_weak_source_t *weak = obs_source_get_weak_source(source);
	obs_source_release(source);
	assert_false(obs_weak_source_expired(weak));
	obs_weak_source_release(weak);
}

static void drop(obs_source_t *source)
{
	obs_source_remove(source);
	release_last(source);
}

/* Whether a public source of this name is listed. */
static bool listed(const char *name)
{
	obs_source_t *found = obs_get_source_by_name(name);
	obs_source_release(found);
	return found != NULL;
}

static obs_source_t *box(const char *name, int value)
{
	obs_data_t *settings = obs_data_create();
	obs_data_set_int(settings, "value", value);
	obs_source_t *source = obs_source_create(BOX_ID, name, settings, NULL);
	obs_data_release(settings);
	assert_non_null(source);
	return source;
}

static obs_source_t *add_filter(obs_source_t *source, const char *name, bool enabled, int value)
{
	obs_data_t *settings = obs_data_create();
	obs_data_set_int(settings, "value", value);
	obs_source_t *filter = obs_source_create_private(FILTER_ID, name, settings);
	obs_data_release(settings);
	assert_non_null(filter);
	obs_source_set_enabled(filter, enabled);
	obs_source_filter_add(source, filter);
	obs_source_release(filter);
	return filter;
}

static long long setting(obs_source_t *source, const char *name)
{
	obs_data_t *settings = obs_source_get_settings(source);
	long long value = obs_data_get_int(settings, name);
	obs_data_release(settings);
	return value;
}

struct listing {
	struct dstr text;
	obs_source_t *originals[2];
};

static void describe(obs_source_t *parent, obs_source_t *child, void *param)
{
	struct listing *l = param;
	(void)parent;
	if (l->text.len)
		dstr_cat(&l->text, ",");
	dstr_catf(&l->text, "%s:%s:%s:%lld:%s", obs_source_get_name(child), obs_source_enabled(child) ? "on" : "off",
		  obs_obj_is_private(child) ? "private" : "public", setting(child, "value"),
		  child == l->originals[0] || child == l->originals[1] ? "original" : "copy");
}

/* The source's filters, in order, as "name:enabled:private:value:which". */
static void expect_filters(obs_source_t *source, obs_source_t *a, obs_source_t *b, const char *expected)
{
	struct listing l = {.originals = {a, b}};
	obs_source_enum_filters(source, describe, &l);
	assert_string_equal(l.text.array ? l.text.array : "", expected);
	dstr_free(&l.text);
}

static bool first_item(obs_scene_t *scene, obs_sceneitem_t *item, void *param)
{
	(void)scene;
	obs_source_t **out = param;
	*out = obs_sceneitem_get_source(item);
	return false;
}

/* The source of the scene's first item (borrowed). */
static obs_source_t *first_item_source(obs_source_t *scene_source)
{
	obs_scene_t *scene = obs_scene_from_source(scene_source);
	if (!scene)
		scene = obs_group_from_source(scene_source);
	assert_non_null(scene);
	obs_source_t *found = NULL;
	obs_scene_enum_items(scene, first_item, &found);
	return found;
}

static void null_source_logs_once(void **state)
{
	(void)state;
	if (skip_without_core())
		return;

	dstr_free(&captured);
	captured_lines = 0;
	capturing = true;
	assert_null(obs_source_duplicate(NULL, "copy", false));
	assert_null(obs_source_duplicate(NULL, NULL, true));
	capturing = false;
	assert_int_equal(captured_lines, 2);
	assert_string_equal(
		captured.array,
		"obs_source_duplicate: Null 'source' parameter|obs_source_duplicate: Null 'source' parameter");
	dstr_free(&captured);
}

static void public_scene_is_shared(void **state)
{
	(void)state;
	if (skip_without_core())
		return;

	obs_scene_t *scene = obs_scene_create("shared scene");
	obs_source_t *src = obs_scene_get_source(scene);
	obs_source_t *dup = obs_source_duplicate(src, "unused name", false);
	assert_ptr_equal(dup, src);
	assert_false(listed("unused name"));
	release_not_last(dup);
	drop(src);
}

static void private_scene_is_copied(void **state)
{
	(void)state;
	if (skip_without_core())
		return;

	obs_scene_t *scene = obs_scene_create("copied scene");
	obs_source_t *src = obs_scene_get_source(scene);
	obs_source_t *child = box("scene child", 1);
	obs_scene_add(scene, child);
	/* The item keeps the child alive. */
	obs_source_release(child);

	obs_source_t *dup = obs_source_duplicate(src, "scene copy", true);
	assert_non_null(dup);
	assert_ptr_not_equal(dup, src);
	assert_non_null(obs_scene_from_source(dup));
	assert_string_equal(obs_source_get_name(dup), "scene copy");
	assert_true(obs_obj_is_private(dup));
	assert_false(listed("scene copy"));
	/* A private copy copies the items' sources. */
	obs_source_t *copied_child = first_item_source(dup);
	assert_non_null(copied_child);
	assert_ptr_not_equal(copied_child, child);
	assert_int_equal(setting(copied_child, "value"), 1);

	release_last(dup);
	drop(src);
}

static void group_is_copied(bool create_private)
{
	obs_scene_t *scene = obs_scene_create("group holder");
	obs_sceneitem_t *group_item = obs_scene_add_group(scene, "group");
	assert_non_null(group_item);
	obs_source_t *group = obs_sceneitem_get_source(group_item);
	obs_source_t *child = box("group child", 2);
	obs_scene_add(obs_sceneitem_group_get_scene(group_item), child);
	/* The item keeps the child alive. */
	obs_source_release(child);
	assert_null(obs_scene_from_source(group));

	obs_source_t *dup = obs_source_duplicate(group, "group copy", create_private);
	assert_non_null(dup);
	assert_ptr_not_equal(dup, group);
	assert_non_null(obs_group_from_source(dup));
	assert_string_equal(obs_source_get_name(dup), "group copy");
	assert_int_equal(obs_obj_is_private(dup), create_private);
	obs_source_t *copied_child = first_item_source(dup);
	assert_non_null(copied_child);
	if (create_private) {
		assert_ptr_not_equal(copied_child, child);
		assert_int_equal(setting(copied_child, "value"), 2);
	} else {
		/* A public group copy keeps its items' sources. */
		assert_ptr_equal(copied_child, child);
	}

	if (!create_private)
		obs_source_remove(dup);
	release_last(dup);
	drop(obs_scene_get_source(scene));
}

static void public_group_is_copied(void **state)
{
	(void)state;
	if (skip_without_core())
		return;
	group_is_copied(false);
}

static void private_group_is_copied(void **state)
{
	(void)state;
	if (skip_without_core())
		return;
	group_is_copied(true);
}

static void do_not_duplicate_is_shared(void **state)
{
	(void)state;
	if (skip_without_core())
		return;

	obs_source_t *src = obs_source_create(SHARED_ID, "not duplicated", NULL, NULL);
	assert_non_null(src);
	add_filter(src, "kept", true, 1);
	for (int private = 0; private < 2; private++) {
		obs_source_t *dup = obs_source_duplicate(src, "unused name", private);
		assert_ptr_equal(dup, src);
		release_not_last(dup);
	}
	assert_false(listed("unused name"));
	drop(src);
}

/* The state obs_source_duplicate() copies, set straight on the fields. */
static void set_state(obs_source_t *src)
{
	src->audio_mixers = 0x2d;
	src->sync_offset = -123456789;
	src->user_volume = 0.25f;
	os_atomic_set_bool(&src->user_muted, true);
	src->volume = 0.75f;
	src->muted = true;
	src->flags = 0x5;
}

static void expect_state(obs_source_t *dup)
{
	assert_int_equal(dup->audio_mixers, 0x2d);
	assert_int_equal(dup->sync_offset, -123456789);
	assert_true(dup->user_volume == 0.25f);
	assert_true(os_atomic_load_bool(&dup->user_muted));
	assert_true(dup->volume == 0.75f);
	assert_true(dup->muted);
	assert_int_equal(dup->flags, 0x5);
}

static void source_is_created(bool create_private)
{
	obs_source_t *src = box("original", 7);
	obs_data_t *priv = obs_source_get_private_settings(src);
	obs_data_set_string(priv, "kept", "private value");
	obs_data_release(priv);
	obs_source_t *a = add_filter(src, "a", true, 1);
	obs_source_t *b = add_filter(src, "b", false, 2);
	set_state(src);

	obs_source_t *dup = obs_source_duplicate(src, "duplicate", create_private);
	assert_non_null(dup);
	assert_ptr_not_equal(dup, src);
	assert_string_equal(obs_source_get_id(dup), BOX_ID);
	assert_string_equal(obs_source_get_name(dup), "duplicate");
	assert_int_equal(obs_obj_is_private(dup), create_private);
	assert_int_equal(listed("duplicate"), !create_private);

	/* The settings are a copy, not the same object. */
	obs_data_t *src_settings = obs_source_get_settings(src);
	obs_data_t *dup_settings = obs_source_get_settings(dup);
	assert_ptr_not_equal(src_settings, dup_settings);
	assert_int_equal(obs_data_get_int(dup_settings, "value"), 7);
	obs_data_set_int(src_settings, "value", 8);
	assert_int_equal(obs_data_get_int(dup_settings, "value"), 7);
	obs_data_release(dup_settings);
	obs_data_release(src_settings);

	priv = obs_source_get_private_settings(dup);
	assert_string_equal(obs_data_get_string(priv, "kept"), "private value");
	obs_data_release(priv);

	expect_state(dup);
	/* Filters are copied (private, settings and enabled state), the
	 * original filters stay where they were. */
	expect_filters(dup, a, b, "a:on:private:1:copy,b:off:private:2:copy");
	expect_filters(src, a, b, "a:on:private:1:original,b:off:private:2:original");

	if (!create_private)
		obs_source_remove(dup);
	release_last(dup);
	drop(src);
}

static void public_source_is_created(void **state)
{
	(void)state;
	if (skip_without_core())
		return;
	source_is_created(false);
}

static void private_source_is_created(void **state)
{
	(void)state;
	if (skip_without_core())
		return;
	source_is_created(true);
}

static void filter_is_created(void **state)
{
	(void)state;
	if (skip_without_core())
		return;

	obs_source_t *src = box("filtered", 3);
	obs_source_t *filter = add_filter(src, "f", false, 4);
	for (int private = 0; private < 2; private++) {
		obs_source_t *dup = obs_source_duplicate(filter, "filter copy", private);
		assert_non_null(dup);
		assert_ptr_not_equal(dup, filter);
		assert_int_equal(obs_source_get_type(dup), OBS_SOURCE_TYPE_FILTER);
		assert_string_equal(obs_source_get_name(dup), "filter copy");
		/* libobs creates every filter private. */
		assert_true(obs_obj_is_private(dup));
		assert_int_equal(setting(dup, "value"), 4);
		assert_null(obs_filter_get_parent(dup));
		release_last(dup);
	}
	drop(src);
}

static void no_task(void *param)
{
	(void)param;
}

static int teardown(void **state)
{
	(void)state;
	capturing = false;
	dstr_free(&captured);
	if (obs_up)
		obs_queue_task(OBS_TASK_DESTROY, no_task, NULL, true);
	return 0;
}

static int group_setup(void **state)
{
	(void)state;
	base_set_log_handler(log_handler, NULL);
	obs_up = obs_startup("en-US", NULL, NULL);
	if (obs_up)
		register_types();
	else
		fprintf(stderr, "obs_startup failed (no display?), skipping\n");
	return 0;
}

static int group_teardown(void **state)
{
	(void)state;
	if (obs_up)
		obs_shutdown();
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_teardown(null_source_logs_once, teardown),
		cmocka_unit_test_teardown(public_scene_is_shared, teardown),
		cmocka_unit_test_teardown(private_scene_is_copied, teardown),
		cmocka_unit_test_teardown(public_group_is_copied, teardown),
		cmocka_unit_test_teardown(private_group_is_copied, teardown),
		cmocka_unit_test_teardown(do_not_duplicate_is_shared, teardown),
		cmocka_unit_test_teardown(public_source_is_created, teardown),
		cmocka_unit_test_teardown(private_source_is_created, teardown),
		cmocka_unit_test_teardown(filter_is_created, teardown),
	};

	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
