/* Characterization of obs_save_sources_filtered() and obs_save_sources()
 * (#5564), the scene-collection save every frontend uses, driven through the
 * public API on a live core (obs_startup, obs_reset_video with the OpenGL
 * module).
 *
 * What it pins:
 *
 *   - an empty core: both return a new, empty array (not NULL) that the
 *     caller owns: two calls give two arrays, and what the caller pushes into
 *     one is not in the next;
 *   - obs_save_sources() saves every public input and scene, in creation
 *     order, each entry equal to obs_save_source() of that source (name,
 *     uuid, id, and the whole entry);
 *   - it leaves out filters (an attached filter shows up only in its parent's
 *     "filters" array; obs_source_create() always makes a filter private, so
 *     the save's own filter-type check is not reachable on its own through
 *     the public API), private sources (obs_source_create_private,
 *     obs_scene_create_private), hidden sources (obs_source_set_hidden),
 *     sources after obs_source_remove (while the caller still holds them)
 *     and sources whose last reference was released;
 *   - obs_save_sources_filtered(cb, param) calls cb once per candidate, in
 *     creation order, with param unchanged, never for an excluded source,
 *     and keeps exactly the sources cb returns true for (false for all gives
 *     an empty array);
 *   - a source saved this way keeps its refcount: the save leaves no
 *     reference behind (the caller's release destroys the source) and drops
 *     none (the source lives on while the caller holds it).
 *
 * The window between a source's last release and its removal from the source
 * list is not reachable through the public API, so "released" is pinned once
 * the source is gone. The video thread takes a short reference on every
 * source each tick, so a released source is waited for (bounded) through a
 * weak reference rather than counted.
 *
 * Core video needs a graphics module and libobs needs an X display on Linux,
 * so the cases skip without one (run under xvfb-run). */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <obs.h>
#include <util/platform.h>

/* obs_source_set_hidden() is deprecated, but it still sets the flag the save
 * checks, and it remains part of the compatibility contract. */
#if defined(_MSC_VER)
#pragma warning(disable : 4996)
#else
#pragma GCC diagnostic ignored "-Wdeprecated-declarations"
#endif

#define INPUT_ID "test_save_sources_input"
#define FILTER_ID "test_save_sources_filter"
#define TIMEOUT_MS 10000
#define MAX_SEEN 16

static bool obs_up;

/* ------------------------------------------------------------------------- */
/* source types                                                              */

static const char *type_name(void *type_data)
{
	(void)type_data;
	return "save sources test";
}

static void *type_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	(void)source;
	return bzalloc(1);
}

static void type_destroy(void *data)
{
	bfree(data);
}

static uint32_t type_size(void *data)
{
	(void)data;
	return 16;
}

static void register_types(void)
{
	/* A video input, so the video filter below is compatible with it. */
	struct obs_source_info input = {
		.id = INPUT_ID,
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_VIDEO,
		.get_width = type_size,
		.get_height = type_size,
		.get_name = type_name,
		.create = type_create,
		.destroy = type_destroy,
	};
	obs_register_source(&input);

	struct obs_source_info filter = {
		.id = FILTER_ID,
		.type = OBS_SOURCE_TYPE_FILTER,
		.output_flags = OBS_SOURCE_VIDEO,
		.get_name = type_name,
		.create = type_create,
		.destroy = type_destroy,
	};
	obs_register_source(&filter);
}

/* ------------------------------------------------------------------------- */
/* helpers                                                                   */

static obs_source_t *input(const char *name)
{
	obs_source_t *source = obs_source_create(INPUT_ID, name, NULL, NULL);
	assert_non_null(source);
	return source;
}

static obs_source_t *scene(const char *name)
{
	obs_scene_t *scene = obs_scene_create(name);
	assert_non_null(scene);
	return obs_scene_get_source(scene);
}

/* Drops the caller's reference to a source and waits (bounded) until it is
 * destroyed: the video thread may still hold a tick reference for a moment. */
static void release_and_wait(obs_source_t *source)
{
	if (!source)
		return;
	obs_weak_source_t *weak = obs_source_get_weak_source(source);
	if (obs_source_get_type(source) == OBS_SOURCE_TYPE_SCENE)
		obs_source_remove(source);
	obs_source_release(source);

	uint64_t deadline = os_gettime_ns() + (uint64_t)TIMEOUT_MS * 1000000ULL;
	for (;;) {
		obs_source_t *alive = obs_weak_source_get_source(weak);
		if (!alive)
			break;
		obs_source_release(alive);
		if (os_gettime_ns() > deadline) {
			obs_weak_source_release(weak);
			fail_msg("a released source was not destroyed within %d ms", TIMEOUT_MS);
		}
		os_sleep_ms(5);
	}
	obs_weak_source_release(weak);
	obs_wait_for_destroy_queue();
}

static void assert_entry_is(obs_data_array_t *array, size_t index, obs_source_t *source)
{
	obs_data_t *entry = obs_data_array_item(array, index);
	assert_non_null(entry);
	obs_data_t *expected = obs_save_source(source);
	assert_non_null(expected);

	assert_string_equal(obs_data_get_string(entry, "name"), obs_source_get_name(source));
	assert_string_equal(obs_data_get_string(entry, "uuid"), obs_source_get_uuid(source));
	assert_string_equal(obs_data_get_string(entry, "id"), obs_source_get_unversioned_id(source));
	assert_string_equal(obs_data_get_string(entry, "versioned_id"), obs_source_get_id(source));
	assert_string_equal(obs_data_get_json(entry), obs_data_get_json(expected));

	obs_data_release(expected);
	obs_data_release(entry);
}

static void assert_saves(obs_data_array_t *array, obs_source_t *const *expected, size_t count)
{
	assert_non_null(array);
	assert_int_equal(obs_data_array_count(array), count);
	for (size_t i = 0; i < count; i++)
		assert_entry_is(array, i, expected[i]);
}

static void assert_core_saves_nothing(void)
{
	obs_data_array_t *array = obs_save_sources();
	assert_non_null(array);
	assert_int_equal(obs_data_array_count(array), 0);
	obs_data_array_release(array);
}

struct visit {
	void *param;
	bool param_ok;
	size_t count;
	obs_source_t *seen[MAX_SEEN];
	/* keep the source at index i when keep_mask bit i is set */
	unsigned keep_mask;
};

static bool record_filter(void *param, obs_source_t *source)
{
	struct visit *v = param;
	if (v->param != param)
		v->param_ok = false;
	size_t index = v->count;
	if (v->count < MAX_SEEN)
		v->seen[v->count] = source;
	v->count++;
	return index < 32 && (v->keep_mask & (1u << index)) != 0;
}

/* ------------------------------------------------------------------------- */
/* cases                                                                     */

#define SKIP_WITHOUT_CORE()                                     \
	do {                                                    \
		if (!obs_up) {                                  \
			fprintf(stderr, "skipped: no live core\n"); \
			skip();                                 \
		}                                               \
	} while (0)

static void empty_core_returns_new_empty_arrays(void **state)
{
	(void)state;
	SKIP_WITHOUT_CORE();

	obs_data_array_t *first = obs_save_sources();
	assert_non_null(first);
	assert_int_equal(obs_data_array_count(first), 0);

	struct visit v = {.param_ok = true};
	v.param = &v;
	obs_data_array_t *filtered = obs_save_sources_filtered(record_filter, &v);
	assert_non_null(filtered);
	assert_int_equal(obs_data_array_count(filtered), 0);
	assert_int_equal(v.count, 0);
	assert_ptr_not_equal(filtered, first);

	/* The caller owns the array: what it pushes stays in its own array. */
	obs_data_t *mine = obs_data_create();
	obs_data_set_string(mine, "name", "caller entry");
	obs_data_array_push_back(first, mine);
	obs_data_release(mine);
	assert_int_equal(obs_data_array_count(first), 1);

	obs_data_array_t *second = obs_save_sources();
	assert_non_null(second);
	assert_ptr_not_equal(second, first);
	assert_int_equal(obs_data_array_count(second), 0);

	obs_data_array_release(first);
	obs_data_array_release(filtered);
	obs_data_array_release(second);
}

static void saves_public_inputs_and_scenes_in_creation_order(void **state)
{
	(void)state;
	SKIP_WITHOUT_CORE();

	obs_source_t *sources[] = {
		input("save input a"),
		scene("save scene a"),
		input("save input b"),
		scene("save scene b"),
	};
	const size_t n = sizeof(sources) / sizeof(sources[0]);

	obs_data_array_t *array = obs_save_sources();
	assert_saves(array, sources, n);
	obs_data_array_release(array);

	for (size_t i = 0; i < n; i++)
		release_and_wait(sources[i]);
	assert_core_saves_nothing();
}

static void leaves_out_filters_private_hidden_removed_and_released(void **state)
{
	(void)state;
	SKIP_WITHOUT_CORE();

	obs_source_t *first = input("keep first");
	obs_source_t *filter = obs_source_create(FILTER_ID, "attached filter", NULL, NULL);
	assert_non_null(filter);
	assert_int_equal(obs_source_get_type(filter), OBS_SOURCE_TYPE_FILTER);
	obs_source_filter_add(first, filter);
	assert_int_equal(obs_source_filter_count(first), 1);
	obs_source_t *loose_filter = obs_source_create(FILTER_ID, "loose filter", NULL, NULL);
	assert_non_null(loose_filter);
	obs_source_t *private_input = obs_source_create_private(INPUT_ID, "private input", NULL);
	assert_non_null(private_input);
	obs_scene_t *private_scene = obs_scene_create_private("private scene");
	assert_non_null(private_scene);
	obs_source_t *hidden = input("hidden input");
	obs_source_set_hidden(hidden, true);
	obs_source_t *removed = input("removed input");
	obs_source_remove(removed);
	assert_true(obs_source_removed(removed));
	obs_source_t *released = input("released input");
	obs_source_t *middle = scene("keep middle");
	obs_source_t *last = input("keep last");

	release_and_wait(released);

	obs_source_t *expected[] = {first, middle, last};
	obs_data_array_t *array = obs_save_sources();
	assert_saves(array, expected, 3);

	/* The attached filter is saved inside its parent's entry only. */
	obs_data_t *entry = obs_data_array_item(array, 0);
	obs_data_array_t *filters = obs_data_get_array(entry, "filters");
	assert_non_null(filters);
	assert_int_equal(obs_data_array_count(filters), 1);
	obs_data_t *saved_filter = obs_data_array_item(filters, 0);
	assert_string_equal(obs_data_get_string(saved_filter, "name"), "attached filter");
	assert_string_equal(obs_data_get_string(saved_filter, "uuid"), obs_source_get_uuid(filter));
	obs_data_release(saved_filter);
	obs_data_array_release(filters);
	obs_data_release(entry);
	obs_data_array_release(array);

	/* A hidden source that is shown again is saved again, in its place. */
	obs_source_set_hidden(hidden, false);
	obs_source_t *with_hidden[] = {first, hidden, middle, last};
	array = obs_save_sources();
	assert_saves(array, with_hidden, 4);
	obs_data_array_release(array);

	obs_source_filter_remove(first, filter);
	release_and_wait(filter);
	release_and_wait(loose_filter);
	release_and_wait(private_input);
	obs_source_t *private_scene_source = obs_source_get_ref(obs_scene_get_source(private_scene));
	obs_scene_release(private_scene);
	release_and_wait(private_scene_source);
	release_and_wait(hidden);
	release_and_wait(removed);
	release_and_wait(middle);
	release_and_wait(last);
	release_and_wait(first);
	assert_core_saves_nothing();
}

static void filtered_calls_back_per_candidate_and_keeps_true(void **state)
{
	(void)state;
	SKIP_WITHOUT_CORE();

	obs_source_t *a = input("filtered a");
	obs_source_t *private_input = obs_source_create_private(INPUT_ID, "filtered private", NULL);
	assert_non_null(private_input);
	obs_source_t *b = scene("filtered b");
	obs_source_t *filter = obs_source_create(FILTER_ID, "filtered filter", NULL, NULL);
	assert_non_null(filter);
	obs_source_filter_add(a, filter);
	assert_int_equal(obs_source_filter_count(a), 1);
	obs_source_t *removed = input("filtered removed");
	obs_source_remove(removed);
	obs_source_t *hidden = input("filtered hidden");
	obs_source_set_hidden(hidden, true);
	obs_source_t *c = input("filtered c");
	obs_source_t *d = scene("filtered d");

	obs_source_t *candidates[] = {a, b, c, d};
	const size_t n = 4;

	/* keep the 1st and 3rd, then none, then all */
	const unsigned masks[] = {0x5u, 0x0u, 0xfu, 0xau};
	for (size_t m = 0; m < sizeof(masks) / sizeof(masks[0]); m++) {
		struct visit v = {.param_ok = true, .keep_mask = masks[m]};
		v.param = &v;
		obs_data_array_t *array = obs_save_sources_filtered(record_filter, &v);
		assert_non_null(array);

		assert_true(v.param_ok);
		assert_int_equal(v.count, n);
		for (size_t i = 0; i < n; i++)
			assert_ptr_equal(v.seen[i], candidates[i]);

		obs_source_t *kept[4];
		size_t kept_count = 0;
		for (size_t i = 0; i < n; i++)
			if (masks[m] & (1u << i))
				kept[kept_count++] = candidates[i];
		assert_saves(array, kept, kept_count);
		obs_data_array_release(array);
	}

	obs_source_filter_remove(a, filter);
	release_and_wait(filter);
	release_and_wait(private_input);
	release_and_wait(removed);
	release_and_wait(hidden);
	release_and_wait(a);
	release_and_wait(b);
	release_and_wait(c);
	release_and_wait(d);
	assert_core_saves_nothing();
}

static void save_keeps_refcount(void **state)
{
	(void)state;
	SKIP_WITHOUT_CORE();

	obs_source_t *source = input("refcount input");
	obs_source_t *scene_source = scene("refcount scene");
	obs_weak_source_t *weak = obs_source_get_weak_source(source);
	obs_weak_source_t *weak_scene = obs_source_get_weak_source(scene_source);

	for (int i = 0; i < 3; i++) {
		obs_data_array_t *array = obs_save_sources();
		assert_int_equal(obs_data_array_count(array), 2);
		obs_data_array_release(array);

		struct visit v = {.param_ok = true, .keep_mask = 0x3u};
		v.param = &v;
		array = obs_save_sources_filtered(record_filter, &v);
		assert_int_equal(obs_data_array_count(array), 2);
		obs_data_array_release(array);
	}

	/* No reference dropped: both still live while the caller holds them. */
	obs_wait_for_destroy_queue();
	obs_source_t *alive = obs_weak_source_get_source(weak);
	assert_ptr_equal(alive, source);
	obs_source_release(alive);
	alive = obs_weak_source_get_source(weak_scene);
	assert_ptr_equal(alive, scene_source);
	obs_source_release(alive);
	assert_string_equal(obs_source_get_name(source), "refcount input");

	/* No reference left behind: the caller's release destroys them. */
	release_and_wait(source);
	release_and_wait(scene_source);
	assert_null(obs_weak_source_get_source(weak));
	assert_null(obs_weak_source_get_source(weak_scene));
	obs_weak_source_release(weak);
	obs_weak_source_release(weak_scene);
	assert_core_saves_nothing();
}

/* ------------------------------------------------------------------------- */

static int group_setup(void **state)
{
	(void)state;
	const char *display = getenv("DISPLAY");
	if (!display || !*display) {
		fprintf(stderr, "skipped: no display\n");
		return 0;
	}
	if (!obs_startup("en-US", NULL, NULL)) {
		fprintf(stderr, "skipped: obs_startup failed\n");
		return 0;
	}

	struct obs_video_info ovi = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = 30,
		.fps_den = 1,
		.base_width = 16,
		.base_height = 16,
		.output_width = 16,
		.output_height = 16,
		.output_format = VIDEO_FORMAT_BGRA,
		.colorspace = VIDEO_CS_SRGB,
		.range = VIDEO_RANGE_FULL,
		.scale_type = OBS_SCALE_POINT,
	};
	int ret = obs_reset_video(&ovi);
	if (ret != OBS_VIDEO_SUCCESS) {
		fprintf(stderr, "skipped: obs_reset_video failed: %d\n", ret);
		obs_shutdown();
		return 0;
	}

	register_types();
	obs_up = true;
	return 0;
}

static int group_teardown(void **state)
{
	(void)state;
	if (obs_up) {
		obs_wait_for_destroy_queue();
		obs_shutdown();
	}
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		/* first: nothing has been created yet */
		cmocka_unit_test(empty_core_returns_new_empty_arrays),
		cmocka_unit_test(saves_public_inputs_and_scenes_in_creation_order),
		cmocka_unit_test(leaves_out_filters_private_hidden_removed_and_released),
		cmocka_unit_test(filtered_calls_back_per_candidate_and_keeps_true),
		cmocka_unit_test(save_keeps_refcount),
	};

	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
