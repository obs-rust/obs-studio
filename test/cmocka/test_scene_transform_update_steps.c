#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs-scene-transform-update.h"

/*
 * Characterization of the transform update decisions of scene_video_render()
 * and update_transforms_and_prune_sources() in libobs/obs-scene.c, moved to
 * libobs/obs-scene-transform-update.c before their Rust port:
 * - obs_scene_transform_update_scene_size(): scene_size_changed(), whether
 *   the scene has a new size since its last render, and the stored size;
 * - obs_scene_transform_update_item(): whether an item's transform is
 *   computed again on this walk.
 *
 * The functions are hidden in libobs, so this test compiles the C source
 * into itself and calls only them. test_scene_transform_update_walk covers
 * the caller through the real libobs: the walk, the removed and deferred
 * items, groups, and when the source size is read.
 */

#define MAX32 UINT32_MAX

/* The scene size step: the stored size and whether it changed. */
struct size_step {
	uint32_t last_width;
	uint32_t last_height;
	uint32_t width;
	uint32_t height;
	bool changed;
};

static void check_size_step(const struct size_step *s)
{
	uint32_t last_width = s->last_width;
	uint32_t last_height = s->last_height;

	assert_int_equal(obs_scene_transform_update_scene_size(&last_width, &last_height, s->width, s->height),
			 s->changed);
	if (s->changed) {
		assert_int_equal(last_width, s->width);
		assert_int_equal(last_height, s->height);
	} else {
		assert_int_equal(last_width, s->last_width);
		assert_int_equal(last_height, s->last_height);
	}
}

static void scene_size_same(void **state)
{
	(void)state;
	static const struct size_step steps[] = {
		{1920, 1080, 1920, 1080, false},     {0, 0, 0, 0, false},         {1, 1, 1, 1, false},
		{MAX32, MAX32, MAX32, MAX32, false}, {MAX32, 0, MAX32, 0, false},
	};

	for (size_t i = 0; i < sizeof(steps) / sizeof(steps[0]); i++)
		check_size_step(&steps[i]);
}

static void scene_size_new(void **state)
{
	(void)state;
	static const struct size_step steps[] = {
		/* the first render: a new scene stores 0 x 0 */
		{0, 0, 1920, 1080, true},
		/* the width alone, the height alone, both */
		{1920, 1080, 1280, 1080, true},
		{1920, 1080, 1920, 720, true},
		{1920, 1080, 1280, 720, true},
		/* width and height swapped */
		{1920, 1080, 1080, 1920, true},
		/* back to 0 x 0 (an empty scene, a canvas without video) */
		{1920, 1080, 0, 0, true},
		/* the limits */
		{0, 0, MAX32, MAX32, true},
		{MAX32, MAX32, MAX32 - 1, MAX32, true},
		{MAX32, MAX32, MAX32, MAX32 - 1, true},
	};

	for (size_t i = 0; i < sizeof(steps) / sizeof(steps[0]); i++)
		check_size_step(&steps[i]);
}

/* A run of renders, as scene_video_render() calls it: the size is stored only
 * on a change, so a size that goes back to an earlier one is a change again,
 * and the same size twice is a change only once. */
static void scene_size_sequence(void **state)
{
	(void)state;
	static const struct {
		uint32_t width;
		uint32_t height;
		bool changed;
	} renders[] = {
		{1920, 1080, true}, {1920, 1080, false}, {1280, 720, true},  {1280, 720, false},
		{1920, 1080, true}, {1920, 1080, false}, {1920, 720, true},  {1920, 1080, true},
		{0, 0, true},       {0, 0, false},       {1920, 1080, true},
	};
	uint32_t last_width = 0;
	uint32_t last_height = 0;

	for (size_t i = 0; i < sizeof(renders) / sizeof(renders[0]); i++) {
		assert_int_equal(obs_scene_transform_update_scene_size(&last_width, &last_height, renders[i].width,
								       renders[i].height),
				 renders[i].changed);
		assert_int_equal(last_width, renders[i].width);
		assert_int_equal(last_height, renders[i].height);
	}
}

/* No input asks for an update: the flag is clear, the source has the size of
 * the item's last transform and the scene did not change. */
static void item_no_update(void **state)
{
	(void)state;
	assert_false(obs_scene_transform_update_item(false, 1920, 1080, 1920, 1080, false));
	assert_false(obs_scene_transform_update_item(false, 0, 0, 0, 0, false));
	assert_false(obs_scene_transform_update_item(false, MAX32, MAX32, MAX32, MAX32, false));
}

/* Each input alone asks for an update. */
static void item_each_input(void **state)
{
	(void)state;
	/* update_transform */
	assert_true(obs_scene_transform_update_item(true, 1920, 1080, 1920, 1080, false));
	/* a new source width, a new source height, both */
	assert_true(obs_scene_transform_update_item(false, 1920, 1080, 1280, 1080, false));
	assert_true(obs_scene_transform_update_item(false, 1920, 1080, 1920, 720, false));
	assert_true(obs_scene_transform_update_item(false, 1920, 1080, 1280, 720, false));
	/* the scene size */
	assert_true(obs_scene_transform_update_item(false, 1920, 1080, 1920, 1080, true));
}

/* Every pattern of the three inputs, with the size the same or new. */
static void item_all_patterns(void **state)
{
	(void)state;
	for (int pattern = 0; pattern < 8; pattern++) {
		bool update_transform = (pattern & 1) != 0;
		bool size_new = (pattern & 2) != 0;
		bool scene_size_changed = (pattern & 4) != 0;
		uint32_t width = size_new ? 641 : 640;

		assert_int_equal(obs_scene_transform_update_item(update_transform, 640, 480, width, 480,
								 scene_size_changed),
				 update_transform || size_new || scene_size_changed);
	}
}

/* With update_transform set the caller does not read the source size and
 * passes 0 x 0; the item updates whatever size it gets. */
static void item_flag_ignores_size(void **state)
{
	(void)state;
	assert_true(obs_scene_transform_update_item(true, 1920, 1080, 0, 0, false));
	assert_true(obs_scene_transform_update_item(true, 0, 0, 0, 0, false));
	assert_true(obs_scene_transform_update_item(true, MAX32, MAX32, 0, 0, true));
}

/* A source that is new, or has no size yet: the item's last size is 0 x 0
 * until its first transform, so a source of 0 x 0 does not ask for one, and a
 * source that gets a size does. */
static void item_zero_sizes(void **state)
{
	(void)state;
	assert_false(obs_scene_transform_update_item(false, 0, 0, 0, 0, false));
	assert_true(obs_scene_transform_update_item(false, 0, 0, 1, 0, false));
	assert_true(obs_scene_transform_update_item(false, 0, 0, 0, 1, false));
	assert_true(obs_scene_transform_update_item(false, 1, 1, 0, 0, false));
}

/* The comparison is of each dimension, not of the area. */
static void item_size_compare(void **state)
{
	(void)state;
	assert_true(obs_scene_transform_update_item(false, 1920, 1080, 1080, 1920, false));
	assert_true(obs_scene_transform_update_item(false, 2, 8, 4, 4, false));
	assert_true(obs_scene_transform_update_item(false, MAX32, 0, 0, MAX32, false));
	assert_true(obs_scene_transform_update_item(false, MAX32, MAX32, MAX32 - 1, MAX32, false));
	assert_true(obs_scene_transform_update_item(false, MAX32, MAX32, MAX32, MAX32 - 1, false));
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(scene_size_same),        cmocka_unit_test(scene_size_new),
		cmocka_unit_test(scene_size_sequence),    cmocka_unit_test(item_no_update),
		cmocka_unit_test(item_each_input),        cmocka_unit_test(item_all_patterns),
		cmocka_unit_test(item_flag_ignores_size), cmocka_unit_test(item_zero_sizes),
		cmocka_unit_test(item_size_compare),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
