/* Characterization of scene, canvas and transition getters and setters that
 * need no graphics device (testing-policy.md, "Characterization tests for
 * units without C tests").
 *
 * Pins the CURRENT behavior of libobs/obs-scene.c.h, obs-canvas.c.h and
 * obs-source-transition.c.h before they are ported: scene references, item
 * lookup through groups, item order and order position, selection, item
 * save and restore, canvas references and weak references, canvas channels,
 * moving and removing canvas scenes, and the transition alignment, size,
 * fixed duration, manual torque and force stop entry points.
 *
 * obs_startup() runs without obs_reset_video(), so no canvas has a video mix
 * and nothing renders. obs_startup() needs a display on Linux (run under
 * xvfb-run); without one the cases skip. On Windows the Cargo-built cores
 * intermittently crash during deferred source destruction even with CTest
 * RUN_SERIAL (#2543); the entire executable is skipped there until #2547
 * fixes the root cause. This is characterized, not endorsed. Public API only,
 * so it runs against libobs, the hybrid libobs and libobs-c-reference. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include "cmocka-compat.h"

#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include <obs.h>
#include <util/bmem.h>

#define LEAF_ID "test_scene_canvas_char_leaf"
#define TRANSITION_ID "test_scene_canvas_char_transition"

static bool obs_up;
static int transition_stop_calls;

/* ------------------------------------------------------------------------ */
/* Fixture source types                                                      */

static const char *fixture_name(void *data)
{
	(void)data;
	return "scene canvas characterization fixture";
}

static void *leaf_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	(void)source;
	return bzalloc(1);
}

static void leaf_destroy(void *data)
{
	bfree(data);
}

static uint32_t leaf_size(void *data)
{
	(void)data;
	return 16;
}

static void *transition_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	return source;
}

static void transition_destroy(void *data)
{
	(void)data;
}

static void transition_video_render(void *data, gs_effect_t *effect)
{
	(void)data;
	(void)effect;
}

static bool transition_audio_render(void *data, uint64_t *ts_out, struct obs_source_audio_mix *audio_output,
				    uint32_t mixers, size_t channels, size_t sample_rate)
{
	(void)data;
	(void)ts_out;
	(void)audio_output;
	(void)mixers;
	(void)channels;
	(void)sample_rate;
	return false;
}

static void transition_stop_cb(void *data)
{
	(void)data;
	transition_stop_calls++;
}

static void register_sources(void)
{
	struct obs_source_info leaf = {
		.id = LEAF_ID,
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_VIDEO,
		.get_name = fixture_name,
		.create = leaf_create,
		.destroy = leaf_destroy,
		.get_width = leaf_size,
		.get_height = leaf_size,
	};
	obs_register_source(&leaf);

	struct obs_source_info transition = {
		.id = TRANSITION_ID,
		.type = OBS_SOURCE_TYPE_TRANSITION,
		.output_flags = OBS_SOURCE_VIDEO,
		.get_name = fixture_name,
		.create = transition_create,
		.destroy = transition_destroy,
		.video_render = transition_video_render,
		.audio_render = transition_audio_render,
		.transition_stop = transition_stop_cb,
	};
	obs_register_source(&transition);
}

#define REQUIRE_OBS()                                                     \
	do {                                                              \
		if (!obs_up) {                                            \
			fprintf(stderr, "skipped: obs_startup failed\n"); \
			skip();                                           \
		}                                                         \
	} while (false)

static void counter_cb(void *data, calldata_t *cd)
{
	(void)cd;
	(*(int *)data)++;
}

/* ------------------------------------------------------------------------ */
/* Scenes                                                                    */

static void scene_get_ref(void **state)
{
	(void)state;
	REQUIRE_OBS();

	assert_null(obs_scene_get_ref(NULL));

	obs_scene_t *scene = obs_scene_create_private("char ref scene");
	assert_non_null(scene);
	obs_scene_t *ref = obs_scene_get_ref(scene);
	assert_ptr_equal(ref, scene);
	obs_scene_release(ref);

	/* The original reference is still alive. */
	assert_ptr_equal(obs_scene_from_source(obs_scene_get_source(scene)), scene);
	obs_scene_release(scene);
	obs_wait_for_destroy_queue();
}

static void scene_find_source_recursive(void **state)
{
	(void)state;
	REQUIRE_OBS();

	assert_null(obs_scene_find_source_recursive(NULL, "x"));

	obs_scene_t *scene = obs_scene_create_private("char find scene");
	obs_source_t *top = obs_source_create_private(LEAF_ID, "char find top", NULL);
	obs_source_t *inner = obs_source_create_private(LEAF_ID, "char find inner", NULL);
	assert_non_null(scene);
	assert_non_null(top);
	assert_non_null(inner);

	obs_sceneitem_t *top_item = obs_scene_add(scene, top);
	obs_sceneitem_t *group = obs_scene_add_group(scene, "char find group");
	assert_non_null(top_item);
	assert_non_null(group);
	assert_true(obs_sceneitem_is_group(group));
	obs_scene_t *group_scene = obs_sceneitem_group_get_scene(group);
	assert_non_null(group_scene);
	obs_sceneitem_t *inner_item = obs_scene_add(group_scene, inner);
	assert_non_null(inner_item);

	assert_ptr_equal(obs_scene_find_source(scene, "char find top"), top_item);
	assert_ptr_equal(obs_scene_find_source_recursive(scene, "char find top"), top_item);

	/* Only the recursive lookup looks inside groups. */
	assert_null(obs_scene_find_source(scene, "char find inner"));
	assert_ptr_equal(obs_scene_find_source_recursive(scene, "char find inner"), inner_item);

	/* The group item itself is found by its name. */
	assert_ptr_equal(obs_scene_find_source_recursive(scene, "char find group"), group);

	assert_null(obs_scene_find_source_recursive(scene, "char find missing"));

	obs_scene_release(scene);
	obs_source_release(top);
	obs_source_release(inner);
	obs_wait_for_destroy_queue();
}

static void scene_item_selected(void **state)
{
	(void)state;
	REQUIRE_OBS();

	assert_false(obs_sceneitem_selected(NULL));

	obs_scene_t *scene = obs_scene_create_private("char select scene");
	obs_source_t *leaf = obs_source_create_private(LEAF_ID, "char select leaf", NULL);
	obs_sceneitem_t *item = obs_scene_add(scene, leaf);
	assert_non_null(item);

	int selects = 0, deselects = 0;
	signal_handler_t *sh = obs_source_get_signal_handler(obs_scene_get_source(scene));
	signal_handler_connect(sh, "item_select", counter_cb, &selects);
	signal_handler_connect(sh, "item_deselect", counter_cb, &deselects);

	assert_false(obs_sceneitem_selected(item));

	obs_sceneitem_select(item, true);
	assert_true(obs_sceneitem_selected(item));
	assert_int_equal(selects, 1);

	/* Selecting a selected item signals nothing. */
	obs_sceneitem_select(item, true);
	assert_int_equal(selects, 1);

	obs_sceneitem_select(item, false);
	assert_false(obs_sceneitem_selected(item));
	assert_int_equal(deselects, 1);

	obs_sceneitem_select(item, false);
	assert_int_equal(deselects, 1);

	signal_handler_disconnect(sh, "item_select", counter_cb, &selects);
	signal_handler_disconnect(sh, "item_deselect", counter_cb, &deselects);
	obs_scene_release(scene);
	obs_source_release(leaf);
	obs_wait_for_destroy_queue();
}

struct order_fixture {
	obs_scene_t *scene;
	obs_source_t *src[3];
	obs_sceneitem_t *item[3]; /* a, b, c */
	int reorders;
};

static void order_setup(struct order_fixture *fx)
{
	static const char *names[3] = {"char order a", "char order b", "char order c"};
	memset(fx, 0, sizeof(*fx));
	fx->scene = obs_scene_create_private("char order scene");
	assert_non_null(fx->scene);
	for (int i = 0; i < 3; i++) {
		fx->src[i] = obs_source_create_private(LEAF_ID, names[i], NULL);
		assert_non_null(fx->src[i]);
		fx->item[i] = obs_scene_add(fx->scene, fx->src[i]);
		assert_non_null(fx->item[i]);
	}
	signal_handler_connect(obs_source_get_signal_handler(obs_scene_get_source(fx->scene)), "reorder", counter_cb,
			       &fx->reorders);
}

static void order_teardown(struct order_fixture *fx)
{
	signal_handler_disconnect(obs_source_get_signal_handler(obs_scene_get_source(fx->scene)), "reorder", counter_cb,
				  &fx->reorders);
	obs_scene_release(fx->scene);
	for (int i = 0; i < 3; i++)
		obs_source_release(fx->src[i]);
	obs_wait_for_destroy_queue();
}

/* Checks the bottom-to-top order by item index (0 = a, 1 = b, 2 = c). */
static void expect_order(struct order_fixture *fx, int bottom, int middle, int top)
{
	int want[3] = {bottom, middle, top};
	for (int pos = 0; pos < 3; pos++) {
		int got = obs_sceneitem_get_order_position(fx->item[want[pos]]);
		if (got != pos) {
			fprintf(stderr, "item %c: order position %d, expected %d\n", 'a' + want[pos], got, pos);
			fail();
		}
	}
}

static void scene_item_set_order(void **state)
{
	(void)state;
	REQUIRE_OBS();

	struct order_fixture fx;
	order_setup(&fx);
	obs_sceneitem_t *a = fx.item[0];
	obs_sceneitem_t *c = fx.item[2];

	/* obs_scene_add appends: the first item added is at the bottom. */
	expect_order(&fx, 0, 1, 2);

	obs_sceneitem_set_order(NULL, OBS_ORDER_MOVE_UP);
	assert_int_equal(fx.reorders, 0);

	obs_sceneitem_set_order(a, OBS_ORDER_MOVE_UP);
	expect_order(&fx, 1, 0, 2);
	assert_int_equal(fx.reorders, 1);

	obs_sceneitem_set_order(a, OBS_ORDER_MOVE_TOP);
	expect_order(&fx, 1, 2, 0);

	/* Moving the top item up keeps it on top, and still signals. */
	obs_sceneitem_set_order(a, OBS_ORDER_MOVE_UP);
	expect_order(&fx, 1, 2, 0);
	assert_int_equal(fx.reorders, 3); /* characterized, not endorsed */

	obs_sceneitem_set_order(a, OBS_ORDER_MOVE_BOTTOM);
	expect_order(&fx, 0, 1, 2);

	obs_sceneitem_set_order(a, OBS_ORDER_MOVE_DOWN);
	expect_order(&fx, 0, 1, 2);

	obs_sceneitem_set_order(c, OBS_ORDER_MOVE_DOWN);
	expect_order(&fx, 0, 2, 1);
	assert_int_equal(fx.reorders, 6);

	order_teardown(&fx);
}

static void scene_item_set_order_position(void **state)
{
	(void)state;
	REQUIRE_OBS();

	struct order_fixture fx;
	order_setup(&fx);
	obs_sceneitem_t *a = fx.item[0];

	obs_sceneitem_set_order_position(NULL, 1);
	assert_int_equal(fx.reorders, 0);

	obs_sceneitem_set_order_position(a, 2);
	expect_order(&fx, 1, 2, 0);
	assert_int_equal(fx.reorders, 1);

	obs_sceneitem_set_order_position(a, 1);
	expect_order(&fx, 1, 0, 2);

	obs_sceneitem_set_order_position(a, 0);
	expect_order(&fx, 0, 1, 2);

	/* A position past the top is clamped to the top. */
	obs_sceneitem_set_order_position(a, 99);
	expect_order(&fx, 1, 2, 0);

	/* A negative position lands at index 1, above the bottom item. */
	obs_sceneitem_set_order_position(a, -5); /* characterized, not endorsed */
	expect_order(&fx, 1, 0, 2);
	assert_int_equal(fx.reorders, 5);

	order_teardown(&fx);
}

static void scene_item_save_and_add(void **state)
{
	(void)state;
	REQUIRE_OBS();

	obs_scene_t *src_scene = obs_scene_create_private("char save scene");
	obs_scene_t *dst_scene = obs_scene_create_private("char restore scene");
	/* obs_sceneitems_add() finds the source by UUID or by name, so it is
	 * public. */
	obs_source_t *leaf = obs_source_create(LEAF_ID, "char save leaf", NULL, NULL);
	assert_non_null(src_scene);
	assert_non_null(dst_scene);
	assert_non_null(leaf);

	obs_sceneitem_t *item = obs_scene_add(src_scene, leaf);
	assert_non_null(item);
	obs_sceneitem_set_id(item, 4242);
	obs_sceneitem_set_locked(item, true);
	obs_sceneitem_set_alignment(item, OBS_ALIGN_RIGHT | OBS_ALIGN_BOTTOM);

	obs_data_array_t *arr = obs_data_array_create();
	obs_sceneitem_save(item, arr);
	assert_int_equal(obs_data_array_count(arr), 1);

	obs_data_t *saved = obs_data_array_item(arr, 0);
	assert_string_equal(obs_data_get_string(saved, "name"), "char save leaf");
	assert_string_equal(obs_data_get_string(saved, "source_uuid"), obs_source_get_uuid(leaf));
	assert_int_equal(obs_data_get_int(saved, "id"), 4242);
	assert_true(obs_data_get_bool(saved, "locked"));
	assert_true(obs_data_get_bool(saved, "visible"));
	assert_int_equal(obs_data_get_int(saved, "align"), OBS_ALIGN_RIGHT | OBS_ALIGN_BOTTOM);
	assert_false(obs_data_get_bool(saved, "group_item_backup"));
	obs_data_release(saved);

	/* An entry for a source that does not exist is skipped. */
	obs_data_t *missing = obs_data_create();
	obs_data_set_string(missing, "name", "char save missing");
	obs_data_array_push_back(arr, missing);
	obs_data_release(missing);

	/* A group backup entry is skipped, even for a source that exists. */
	obs_data_t *backup = obs_data_create();
	obs_data_set_string(backup, "name", "char save leaf");
	obs_data_set_bool(backup, "group_item_backup", true);
	obs_data_array_push_back(arr, backup);
	obs_data_release(backup);

	int adds = 0;
	signal_handler_t *sh = obs_source_get_signal_handler(obs_scene_get_source(dst_scene));
	signal_handler_connect(sh, "item_add", counter_cb, &adds);
	obs_sceneitems_add(dst_scene, arr);
	signal_handler_disconnect(sh, "item_add", counter_cb, &adds);
	assert_int_equal(adds, 1);

	obs_sceneitem_t *restored = obs_scene_find_source(dst_scene, "char save leaf");
	assert_non_null(restored);
	assert_ptr_not_equal(restored, item);
	assert_ptr_equal(obs_sceneitem_get_source(restored), leaf);
	assert_int_equal(obs_sceneitem_get_id(restored), 4242);
	assert_true(obs_sceneitem_locked(restored));
	assert_int_equal(obs_sceneitem_get_alignment(restored), OBS_ALIGN_RIGHT | OBS_ALIGN_BOTTOM);
	assert_ptr_equal(obs_scene_find_sceneitem_by_id(dst_scene, 4242), restored);

	obs_data_array_release(arr);
	obs_scene_release(dst_scene);
	obs_scene_release(src_scene);
	obs_source_remove(leaf);
	obs_source_release(leaf);
	obs_wait_for_destroy_queue();
}

/* ------------------------------------------------------------------------ */
/* Canvases                                                                  */

static void canvas_refs(void **state)
{
	(void)state;
	REQUIRE_OBS();

	assert_null(obs_canvas_get_ref(NULL));
	assert_null(obs_canvas_get_weak_canvas(NULL));
	assert_null(obs_weak_canvas_get_canvas(NULL));
	obs_weak_canvas_addref(NULL);
	obs_weak_canvas_release(NULL);

	obs_canvas_t *canvas = obs_canvas_create_private("char ref canvas", NULL, 0);
	assert_non_null(canvas);

	obs_canvas_t *ref = obs_canvas_get_ref(canvas);
	assert_ptr_equal(ref, canvas);
	obs_canvas_release(ref);

	obs_weak_canvas_t *weak = obs_canvas_get_weak_canvas(canvas);
	assert_non_null(weak);
	obs_weak_canvas_addref(weak);
	obs_weak_canvas_release(weak);

	/* One weak reference is left and the canvas is still alive. */
	obs_canvas_t *strong = obs_weak_canvas_get_canvas(weak);
	assert_ptr_equal(strong, canvas);
	obs_canvas_release(strong);

	obs_canvas_release(canvas);
	/* The last strong reference is gone: the weak one no longer upgrades. */
	assert_null(obs_weak_canvas_get_canvas(weak));
	obs_weak_canvas_release(weak);
	obs_wait_for_destroy_queue();
}

static void canvas_without_video(void **state)
{
	(void)state;
	REQUIRE_OBS();

	obs_canvas_t *canvas = obs_canvas_create_private("char novideo canvas", NULL, 0);
	assert_non_null(canvas);
	assert_false(obs_canvas_has_video(canvas));
	assert_null(obs_canvas_get_video(canvas));

	struct obs_video_info ovi;
	memset(&ovi, 0xab, sizeof(ovi));
	assert_false(obs_canvas_get_video_info(canvas, &ovi));
	/* The output is left untouched. */
	assert_int_equal(ovi.base_width, 0xabababab);

	/* No video was reset, so the main canvas has no mix either. */
	obs_canvas_t *main_canvas = obs_get_main_canvas();
	assert_non_null(main_canvas);
	assert_false(obs_canvas_has_video(main_canvas));
	assert_false(obs_canvas_get_video_info(main_canvas, &ovi));
	obs_canvas_release(main_canvas);

	obs_canvas_release(canvas);
	obs_wait_for_destroy_queue();
}

static void canvas_channels(void **state)
{
	(void)state;
	REQUIRE_OBS();

	obs_canvas_t *canvas = obs_canvas_create_private("char channel canvas", NULL, 0);
	obs_source_t *a = obs_source_create_private(LEAF_ID, "char channel a", NULL);
	obs_source_t *b = obs_source_create_private(LEAF_ID, "char channel b", NULL);
	assert_non_null(canvas);
	assert_non_null(a);
	assert_non_null(b);

	int changes = 0;
	signal_handler_t *sh = obs_canvas_get_signal_handler(canvas);
	signal_handler_connect(sh, "channel_change", counter_cb, &changes);

	assert_null(obs_canvas_get_channel(canvas, 0));

	obs_canvas_set_channel(canvas, 0, a);
	assert_int_equal(changes, 1);
	obs_source_t *got = obs_canvas_get_channel(canvas, 0);
	assert_ptr_equal(got, a);
	obs_source_release(got);
	assert_null(obs_canvas_get_channel(canvas, 1));

	/* Setting the same source again signals nothing. */
	obs_canvas_set_channel(canvas, 0, a);
	assert_int_equal(changes, 1);

	obs_canvas_set_channel(canvas, 0, b);
	assert_int_equal(changes, 2);
	got = obs_canvas_get_channel(canvas, 0);
	assert_ptr_equal(got, b);
	obs_source_release(got);

	obs_canvas_set_channel(canvas, MAX_CHANNELS - 1, a);
	got = obs_canvas_get_channel(canvas, MAX_CHANNELS - 1);
	assert_ptr_equal(got, a);
	obs_source_release(got);
	assert_int_equal(changes, 3);

	obs_canvas_set_channel(canvas, 0, NULL);
	obs_canvas_set_channel(canvas, MAX_CHANNELS - 1, NULL);
	assert_null(obs_canvas_get_channel(canvas, 0));
	assert_null(obs_canvas_get_channel(canvas, MAX_CHANNELS - 1));
	assert_int_equal(changes, 5);

	signal_handler_disconnect(sh, "channel_change", counter_cb, &changes);
	obs_source_release(a);
	obs_source_release(b);
	obs_canvas_release(canvas);
	obs_wait_for_destroy_queue();
}

static void canvas_scene_remove_and_move(void **state)
{
	(void)state;
	REQUIRE_OBS();

	obs_canvas_t *src = obs_canvas_create_private("char move src", NULL, 0);
	obs_canvas_t *dst = obs_canvas_create_private("char move dst", NULL, 0);
	assert_non_null(src);
	assert_non_null(dst);

	obs_scene_t *moved = obs_canvas_scene_create(src, "char moved scene");
	obs_scene_t *removed = obs_canvas_scene_create(src, "char removed scene");
	assert_non_null(moved);
	assert_non_null(removed);

	obs_canvas_t *owner = obs_source_get_canvas(obs_scene_get_source(moved));
	assert_ptr_equal(owner, src);
	obs_canvas_release(owner);

	obs_scene_t *found = obs_canvas_get_scene_by_name(src, "char moved scene");
	assert_ptr_equal(found, moved);
	obs_scene_release(found);

	obs_canvas_move_scene(moved, dst);
	assert_null(obs_canvas_get_scene_by_name(src, "char moved scene"));
	found = obs_canvas_get_scene_by_name(dst, "char moved scene");
	assert_ptr_equal(found, moved);
	obs_scene_release(found);
	owner = obs_source_get_canvas(obs_scene_get_source(moved));
	assert_ptr_equal(owner, dst);
	obs_canvas_release(owner);

	obs_canvas_scene_remove(removed);
	assert_null(obs_canvas_get_scene_by_name(src, "char removed scene"));
	/* The scene is left with no canvas. */
	assert_null(obs_source_get_canvas(obs_scene_get_source(removed)));

	obs_source_remove(obs_scene_get_source(moved));
	obs_scene_release(moved);
	obs_scene_release(removed);
	obs_canvas_release(src);
	obs_canvas_release(dst);
	obs_wait_for_destroy_queue();
}

/* ------------------------------------------------------------------------ */
/* Transitions                                                               */

static void transition_invalid_sources(void **state)
{
	(void)state;
	REQUIRE_OBS();

	uint32_t cx = 7, cy = 7;
	assert_int_equal(obs_transition_get_alignment(NULL), 0);
	obs_transition_get_size(NULL, &cx, &cy);
	assert_int_equal(cx, 0);
	assert_int_equal(cy, 0);
	assert_false(obs_transition_fixed(NULL));
	obs_transition_enable_fixed(NULL, true, 100);

	/* A source that is not a transition reads as the defaults and ignores
	 * the setters. */
	obs_source_t *leaf = obs_source_create_private(LEAF_ID, "char not a transition", NULL);
	assert_non_null(leaf);
	obs_transition_set_alignment(leaf, OBS_ALIGN_RIGHT);
	obs_transition_set_size(leaf, 10, 20);
	obs_transition_enable_fixed(leaf, true, 100);
	assert_int_equal(obs_transition_get_alignment(leaf), 0);
	cx = cy = 7;
	obs_transition_get_size(leaf, &cx, &cy);
	assert_int_equal(cx, 0);
	assert_int_equal(cy, 0);
	assert_false(obs_transition_fixed(leaf));
	obs_source_release(leaf);
	obs_wait_for_destroy_queue();
}

static obs_source_t *create_transition(const char *name)
{
	obs_source_t *tr = obs_source_create_private(TRANSITION_ID, name, NULL);
	if (!tr || obs_source_get_type(tr) != OBS_SOURCE_TYPE_TRANSITION) {
		fprintf(stderr, "skipped: transition could not be created without graphics\n");
		obs_source_release(tr);
		return NULL;
	}
	return tr;
}

static void transition_getters_setters(void **state)
{
	(void)state;
	REQUIRE_OBS();

	obs_source_t *tr = create_transition("char transition");
	if (!tr)
		skip();

	/* Defaults from obs_transition_init(). */
	assert_int_equal(obs_transition_get_alignment(tr), OBS_ALIGN_LEFT | OBS_ALIGN_TOP);
	uint32_t cx = 7, cy = 7;
	obs_transition_get_size(tr, &cx, &cy);
	assert_int_equal(cx, 0);
	assert_int_equal(cy, 0);
	assert_false(obs_transition_fixed(tr));

	obs_transition_set_alignment(tr, OBS_ALIGN_CENTER);
	assert_int_equal(obs_transition_get_alignment(tr), OBS_ALIGN_CENTER);
	/* Any value is stored, also bits that are no alignment. */
	obs_transition_set_alignment(tr, 0xff00); /* characterized, not endorsed */
	assert_int_equal(obs_transition_get_alignment(tr), 0xff00);

	obs_transition_set_size(tr, 1920, 1080);
	obs_transition_get_size(tr, &cx, &cy);
	assert_int_equal(cx, 1920);
	assert_int_equal(cy, 1080);

	obs_transition_enable_fixed(tr, true, 500);
	assert_true(obs_transition_fixed(tr));
	obs_transition_enable_fixed(tr, false, 0);
	assert_false(obs_transition_fixed(tr));

	/* No getter: it only must not disturb the other state. */
	obs_transition_set_manual_torque(tr, 4.0f, 0.01f);
	assert_int_equal(obs_transition_get_alignment(tr), 0xff00);
	assert_false(obs_transition_is_active(tr));

	obs_source_release(tr);
	obs_wait_for_destroy_queue();
}

static void transition_force_stop(void **state)
{
	(void)state;
	REQUIRE_OBS();

	obs_source_t *tr = create_transition("char force stop");
	if (!tr)
		skip();

	int stops = 0;
	signal_handler_t *sh = obs_source_get_signal_handler(tr);
	signal_handler_connect(sh, "transition_stop", counter_cb, &stops);

	transition_stop_calls = 0;
	assert_false(obs_transition_is_active(tr));
	/* It runs the stop callback and signals even when nothing is
	 * transitioning. */
	obs_transition_force_stop(tr); /* characterized, not endorsed */
	assert_int_equal(transition_stop_calls, 1);
	assert_int_equal(stops, 1);
	obs_transition_force_stop(tr);
	assert_int_equal(transition_stop_calls, 2);
	assert_int_equal(stops, 2);

	signal_handler_disconnect(sh, "transition_stop", counter_cb, &stops);
	obs_source_release(tr);
	obs_wait_for_destroy_queue();
}

/* ------------------------------------------------------------------------ */

static int setup(void **state)
{
	(void)state;
	obs_up = obs_startup("en-US", NULL, NULL);
	if (!obs_up) {
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
		return 0;
	}
	register_sources();
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	if (obs_up)
		obs_shutdown();
	obs_up = false;
	return 0;
}

int main(void)
{
#ifdef _WIN32
	/* CTest marks exit 77 as skipped. Do not run source destruction without
	 * graphics on Windows until #2547 diagnoses the Cargo-core crash. */
	return 77;
#endif
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(scene_get_ref),
		cmocka_unit_test(scene_find_source_recursive),
		cmocka_unit_test(scene_item_selected),
		cmocka_unit_test(scene_item_set_order),
		cmocka_unit_test(scene_item_set_order_position),
		cmocka_unit_test(scene_item_save_and_add),
		cmocka_unit_test(canvas_refs),
		cmocka_unit_test(canvas_without_video),
		cmocka_unit_test(canvas_channels),
		cmocka_unit_test(canvas_scene_remove_and_move),
		cmocka_unit_test(transition_invalid_sources),
		cmocka_unit_test(transition_getters_setters),
		cmocka_unit_test(transition_force_stop),
	};

	return cmocka_run_group_tests(tests, setup, teardown);
}
