/* Characterization of the callers of the canvas flag rules
 * (libobs/obs-canvas-rules.c, from obs-canvas.c) through the real libobs
 * (testing-policy.md, "Characterization tests for units without C tests").
 * test_canvas_rules_steps pins the rules on their own; this pins what the
 * callers in obs-canvas.c do with them:
 * - the flags each constructor keeps (obs_create_main_canvas(),
 *   obs_canvas_create(), obs_canvas_create_private(), obs_load_canvas()) and
 *   what obs_save_canvas() writes or refuses;
 * - obs_canvas_reset_video(): no video info, the main canvas, and while video
 *   is active (a raw video callback on the main mix) it refuses; otherwise the
 *   canvas gets a mix and signals video_reset;
 * - obs_canvas_set_name(): NULL, "", the same name and the main canvas are
 *   refused; a public canvas is renamed in the name table and signals
 *   canvas_rename globally, a private one signals only its own rename;
 * - obs_canvas_remove(): once, never for the main canvas;
 * - the SCENE_REF rule of insert, remove and destroy for a scene, a group and
 *   a plain source on the main canvas and a scene on a canvas without it,
 *   and the group walk of obs_canvas_remove_source() (a removed group leaves
 *   every scene of its canvas; a removed plain source's items stay);
 * - obs_canvas_rename_source()'s global source_rename, main canvas only;
 * - obs_canvas_set_channel()'s channel_change, whose handler may replace the
 *   source: the replacement is stored and activated without a reference of
 *   its own, and the reference taken on the source passed in is kept.
 * Cases marked "characterized, not endorsed" are odd but real.
 *
 * obs_startup() and obs_reset_video() need a display and OpenGL (run under
 * xvfb-run); without them the cases skip. Public API only, so it runs against
 * libobs, the hybrid libobs and libobs-c-reference. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include "cmocka-compat.h"

#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include <obs.h>

#define PLAIN_ID "test_canvas_rules_callers_plain"
#define CANVAS_SIZE 64

static bool obs_up;

#define REQUIRE_OBS()                                                                   \
	do {                                                                            \
		if (!obs_up) {                                                          \
			fprintf(stderr, "skipped: obs_startup or obs_reset_video failed\n"); \
			skip();                                                         \
		}                                                                       \
	} while (false)

/* ------------------------------------------------------------------------ */
/* Fixture source type: a plain input that lives in a canvas                  */

static const char *plain_name(void *data)
{
	(void)data;
	return "canvas rules callers plain";
}

static void *plain_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	return source;
}

static void plain_destroy(void *data)
{
	(void)data;
}

static uint32_t plain_size(void *data)
{
	(void)data;
	return CANVAS_SIZE;
}

static void register_sources(void)
{
	struct obs_source_info plain = {
		.id = PLAIN_ID,
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_VIDEO | OBS_SOURCE_REQUIRES_CANVAS,
		.get_name = plain_name,
		.create = plain_create,
		.destroy = plain_destroy,
		.get_width = plain_size,
		.get_height = plain_size,
	};
	obs_register_source(&plain);
}

static struct obs_video_info video_info(void)
{
	struct obs_video_info ovi = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = 30,
		.fps_den = 1,
		.base_width = CANVAS_SIZE,
		.base_height = CANVAS_SIZE,
		.output_width = CANVAS_SIZE,
		.output_height = CANVAS_SIZE,
		.output_format = VIDEO_FORMAT_NV12,
		.colorspace = VIDEO_CS_709,
		.range = VIDEO_RANGE_PARTIAL,
		.scale_type = OBS_SCALE_BICUBIC,
	};
	return ovi;
}

/* ------------------------------------------------------------------------ */
/* Signal recorders                                                          */

struct counter {
	int calls;
	char last[64];
};

static void count_cb(void *data, calldata_t *cd)
{
	struct counter *c = data;
	const char *name = calldata_string(cd, "new_name");
	c->calls++;
	snprintf(c->last, sizeof(c->last), "%s", name ? name : "");
}

static bool source_alive(obs_weak_source_t *weak)
{
	obs_source_t *s = obs_weak_source_get_source(weak);
	obs_source_release(s);
	return s != NULL;
}

/* ------------------------------------------------------------------------ */
/* Constructors, save and load                                               */

static obs_canvas_t *load_with_flags(long long flags, bool private)
{
	obs_data_t *data = obs_data_create();
	obs_data_set_string(data, "name", private ? "rules loaded private" : "rules loaded");
	obs_data_set_int(data, "flags", flags);
	obs_data_set_bool(data, "private", private);
	obs_canvas_t *canvas = obs_load_canvas(data);
	obs_data_release(data);
	return canvas;
}

static void constructor_flags(void **state)
{
	(void)state;
	REQUIRE_OBS();

	obs_canvas_t *main_canvas = obs_get_main_canvas();
	assert_int_equal(obs_canvas_get_flags(main_canvas), MAIN | PROGRAM);
	obs_canvas_release(main_canvas);

	/* MAIN, REMOVED and unknown bits are dropped */
	obs_canvas_t *c = obs_canvas_create("rules ctor", NULL, MAIN | PROGRAM | 0x80000000u | (1u << 5));
	obs_canvas_t *p = obs_canvas_create_private("rules ctor private", NULL, 0xffffffffu);
	assert_int_equal(obs_canvas_get_flags(c), PROGRAM);
	assert_int_equal(obs_canvas_get_flags(p), PROGRAM | EPHEMERAL);
	assert_false(obs_canvas_removed(c));
	assert_false(obs_canvas_removed(p));
	obs_canvas_release(c);
	obs_canvas_release(p);

	/* characterized, not endorsed: the saved int64 is truncated before the
	 * mask, so bits above 31 are dropped and -1 keeps every public flag */
	long long saved[] = {-1, 0x100000001LL, 0x10000001eLL, 0x80000001LL, PROGRAM};
	uint32_t kept[] = {0x1e, 0, 0x1e, 0, PROGRAM};
	for (size_t i = 0; i < sizeof(saved) / sizeof(saved[0]); i++) {
		obs_canvas_t *loaded = load_with_flags(saved[i], false);
		assert_non_null(loaded);
		assert_int_equal(obs_canvas_get_flags(loaded), kept[i]);
		assert_false(obs_canvas_removed(loaded));
		obs_canvas_release(loaded);
	}
	obs_wait_for_destroy_queue();
}

static void save_rules(void **state)
{
	(void)state;
	REQUIRE_OBS();

	obs_canvas_t *program = obs_canvas_create("rules save program", NULL, PROGRAM);
	obs_canvas_t *preview = obs_canvas_create("rules save preview", NULL, PREVIEW);
	obs_canvas_t *private = obs_canvas_create_private("rules save private", NULL, SCENE_REF);

	obs_data_t *data = obs_save_canvas(program);
	assert_non_null(data);
	assert_int_equal(obs_data_get_int(data, "flags"), PROGRAM);
	assert_false(obs_data_get_bool(data, "private"));
	obs_data_release(data);

	data = obs_save_canvas(private);
	assert_non_null(data);
	assert_int_equal(obs_data_get_int(data, "flags"), SCENE_REF);
	assert_true(obs_data_get_bool(data, "private"));
	obs_canvas_t *reloaded = obs_load_canvas(data);
	assert_int_equal(obs_canvas_get_flags(reloaded), SCENE_REF);
	obs_canvas_release(reloaded);
	obs_data_release(data);

	/* an ephemeral canvas is not saved, nor a removed one */
	assert_null(obs_save_canvas(preview));
	obs_canvas_remove(program);
	assert_null(obs_save_canvas(program));

	obs_canvas_release(program);
	obs_canvas_release(preview);
	obs_canvas_release(private);
	obs_wait_for_destroy_queue();
}

/* ------------------------------------------------------------------------ */
/* Reset, rename, remove                                                     */

static void raw_video_cb(void *param, struct video_data *frame)
{
	(void)param;
	(void)frame;
}

static void reset_rules(void **state)
{
	(void)state;
	REQUIRE_OBS();

	struct obs_video_info ovi = video_info();
	obs_canvas_t *canvas = obs_canvas_create("rules reset", NULL, PROGRAM);
	struct counter resets = {0};
	signal_handler_t *sh = obs_canvas_get_signal_handler(canvas);
	signal_handler_connect(sh, "video_reset", count_cb, &resets);

	assert_false(obs_canvas_has_video(canvas));
	assert_false(obs_canvas_reset_video(canvas, NULL));
	assert_int_equal(resets.calls, 0);

	/* the main canvas is reset only through obs_reset_video() */
	obs_canvas_t *main_canvas = obs_get_main_canvas();
	assert_false(obs_canvas_reset_video(main_canvas, &ovi));
	obs_canvas_release(main_canvas);

	assert_true(obs_canvas_reset_video(canvas, &ovi));
	assert_true(obs_canvas_has_video(canvas));
	assert_int_equal(resets.calls, 1);

	/* while video is active (a raw callback on the main mix) it refuses,
	 * before touching the mix */
	obs_add_raw_video_callback(NULL, raw_video_cb, NULL);
	assert_true(obs_video_active());
	assert_false(obs_canvas_reset_video(canvas, &ovi));
	assert_int_equal(resets.calls, 1);
	assert_true(obs_canvas_has_video(canvas));
	obs_remove_raw_video_callback(raw_video_cb, NULL);
	assert_false(obs_video_active());

	assert_true(obs_canvas_reset_video(canvas, &ovi));
	assert_int_equal(resets.calls, 2);

	signal_handler_disconnect(sh, "video_reset", count_cb, &resets);
	obs_canvas_release(canvas);
	obs_wait_for_destroy_queue();
}

static void set_name_rules(void **state)
{
	(void)state;
	REQUIRE_OBS();

	obs_canvas_t *pub = obs_canvas_create("rules name public", NULL, 0);
	obs_canvas_t *priv = obs_canvas_create_private("rules name private", NULL, 0);
	obs_canvas_t *main_canvas = obs_get_main_canvas();
	struct counter pub_renames = {0}, priv_renames = {0}, main_renames = {0}, global = {0};
	signal_handler_connect(obs_canvas_get_signal_handler(pub), "rename", count_cb, &pub_renames);
	signal_handler_connect(obs_canvas_get_signal_handler(priv), "rename", count_cb, &priv_renames);
	signal_handler_connect(obs_canvas_get_signal_handler(main_canvas), "rename", count_cb, &main_renames);
	signal_handler_connect(obs_get_signal_handler(), "canvas_rename", count_cb, &global);

	/* refused: no name, an empty name, the same name, the main canvas */
	obs_canvas_set_name(pub, NULL);
	obs_canvas_set_name(pub, "");
	obs_canvas_set_name(pub, "rules name public");
	obs_canvas_set_name(main_canvas, "rules name not main");
	assert_string_equal(obs_canvas_get_name(pub), "rules name public");
	assert_string_equal(obs_canvas_get_name(main_canvas), "Main");
	assert_int_equal(pub_renames.calls, 0);
	assert_int_equal(main_renames.calls, 0);
	assert_int_equal(global.calls, 0);

	/* a public canvas: the name table and the global signal */
	obs_canvas_set_name(pub, "rules name public 2");
	assert_string_equal(obs_canvas_get_name(pub), "rules name public 2");
	assert_int_equal(pub_renames.calls, 1);
	assert_string_equal(pub_renames.last, "rules name public 2");
	assert_int_equal(global.calls, 1);
	obs_canvas_t *found = obs_get_canvas_by_name("rules name public 2");
	assert_ptr_equal(found, pub);
	obs_canvas_release(found);
	assert_null(obs_get_canvas_by_name("rules name public"));

	/* a private canvas: its own signal only, and no name table */
	obs_canvas_set_name(priv, "rules name private 2");
	assert_string_equal(obs_canvas_get_name(priv), "rules name private 2");
	assert_int_equal(priv_renames.calls, 1);
	assert_int_equal(global.calls, 1);
	assert_null(obs_get_canvas_by_name("rules name private 2"));

	/* characterized, not endorsed: a removed canvas can still be renamed */
	obs_canvas_remove(pub);
	obs_canvas_set_name(pub, "rules name public 3");
	assert_string_equal(obs_canvas_get_name(pub), "rules name public 3");
	assert_int_equal(pub_renames.calls, 2);

	signal_handler_disconnect(obs_canvas_get_signal_handler(pub), "rename", count_cb, &pub_renames);
	signal_handler_disconnect(obs_canvas_get_signal_handler(priv), "rename", count_cb, &priv_renames);
	signal_handler_disconnect(obs_canvas_get_signal_handler(main_canvas), "rename", count_cb, &main_renames);
	signal_handler_disconnect(obs_get_signal_handler(), "canvas_rename", count_cb, &global);
	obs_canvas_release(main_canvas);
	obs_canvas_release(pub);
	obs_canvas_release(priv);
	obs_wait_for_destroy_queue();
}

static void remove_rules(void **state)
{
	(void)state;
	REQUIRE_OBS();

	obs_canvas_t *pub = obs_canvas_create("rules remove", NULL, PROGRAM);
	obs_canvas_t *main_canvas = obs_get_main_canvas();
	struct counter own = {0}, global = {0}, main_own = {0};
	signal_handler_connect(obs_canvas_get_signal_handler(pub), "remove", count_cb, &own);
	signal_handler_connect(obs_canvas_get_signal_handler(main_canvas), "remove", count_cb, &main_own);
	signal_handler_connect(obs_get_signal_handler(), "canvas_remove", count_cb, &global);

	obs_canvas_remove(pub);
	assert_true(obs_canvas_removed(pub));
	assert_int_equal(obs_canvas_get_flags(pub), PROGRAM | 0x80000000u);
	assert_int_equal(own.calls, 1);
	assert_int_equal(global.calls, 1);

	/* the second remove does nothing */
	obs_canvas_remove(pub);
	assert_int_equal(own.calls, 1);
	assert_int_equal(global.calls, 1);

	obs_canvas_remove(main_canvas);
	assert_false(obs_canvas_removed(main_canvas));
	assert_int_equal(main_own.calls, 0);
	assert_int_equal(global.calls, 1);

	signal_handler_disconnect(obs_canvas_get_signal_handler(pub), "remove", count_cb, &own);
	signal_handler_disconnect(obs_canvas_get_signal_handler(main_canvas), "remove", count_cb, &main_own);
	signal_handler_disconnect(obs_get_signal_handler(), "canvas_remove", count_cb, &global);
	obs_canvas_release(main_canvas);
	obs_canvas_release(pub);
	obs_wait_for_destroy_queue();
}

/* ------------------------------------------------------------------------ */
/* Scene references, the group walk and source renames                       */

static void scene_refs(void **state)
{
	(void)state;
	REQUIRE_OBS();

	/* the main canvas holds SCENE_REF: a scene outlives its creator's
	 * reference until it is removed */
	obs_scene_t *scene = obs_scene_create("rules ref scene");
	obs_weak_source_t *weak_scene = obs_source_get_weak_source(obs_scene_get_source(scene));
	obs_scene_release(scene);
	assert_true(source_alive(weak_scene));
	obs_source_t *s = obs_weak_source_get_source(weak_scene);
	obs_source_remove(s);
	obs_source_release(s);
	obs_wait_for_destroy_queue();
	assert_false(source_alive(weak_scene));
	obs_weak_source_release(weak_scene);

	/* a plain source on the main canvas gets no canvas reference */
	obs_source_t *plain = obs_source_create(PLAIN_ID, "rules ref plain", NULL, NULL);
	assert_non_null(plain);
	obs_canvas_t *owner = obs_source_get_canvas(plain);
	obs_canvas_t *main_canvas = obs_get_main_canvas();
	assert_ptr_equal(owner, main_canvas);
	obs_canvas_release(owner);
	obs_source_t *by_name = obs_canvas_get_source_by_name(main_canvas, "rules ref plain");
	assert_ptr_equal(by_name, plain);
	obs_source_release(by_name);
	obs_weak_source_t *weak_plain = obs_source_get_weak_source(plain);
	obs_source_release(plain);
	obs_wait_for_destroy_queue();
	assert_false(source_alive(weak_plain));
	obs_weak_source_release(weak_plain);
	assert_null(obs_canvas_get_source_by_name(main_canvas, "rules ref plain"));
	obs_canvas_release(main_canvas);

	/* a canvas without SCENE_REF does not hold its scenes */
	obs_canvas_t *noref = obs_canvas_create("rules noref", NULL, ACTIVATE | MIX_AUDIO);
	scene = obs_canvas_scene_create(noref, "rules noref scene");
	obs_weak_source_t *weak_noref = obs_source_get_weak_source(obs_scene_get_source(scene));
	obs_scene_t *found = obs_canvas_get_scene_by_name(noref, "rules noref scene");
	assert_ptr_equal(found, scene);
	obs_scene_release(found);
	obs_scene_release(scene);
	obs_wait_for_destroy_queue();
	assert_false(source_alive(weak_noref));
	assert_null(obs_canvas_get_scene_by_name(noref, "rules noref scene"));
	obs_weak_source_release(weak_noref);

	/* a SCENE_REF canvas releases its scenes when it is destroyed */
	obs_canvas_t *ref = obs_canvas_create("rules ref canvas", NULL, SCENE_REF);
	scene = obs_canvas_scene_create(ref, "rules ref canvas scene");
	obs_weak_source_t *weak_ref = obs_source_get_weak_source(obs_scene_get_source(scene));
	obs_scene_release(scene);
	assert_true(source_alive(weak_ref));
	obs_canvas_release(ref);
	obs_wait_for_destroy_queue();
	assert_false(source_alive(weak_ref));
	obs_weak_source_release(weak_ref);

	obs_canvas_release(noref);
	obs_wait_for_destroy_queue();
}

static void group_walk(void **state)
{
	(void)state;
	REQUIRE_OBS();

	obs_scene_t *a = obs_scene_create("rules walk a");
	obs_scene_t *b = obs_scene_create("rules walk b");
	obs_sceneitem_t *item = obs_scene_add_group2(a, "rules walk group", false);
	assert_non_null(item);
	obs_source_t *group = obs_sceneitem_get_source(item);
	obs_canvas_t *owner = obs_source_get_canvas(group);
	obs_canvas_t *main_canvas = obs_get_main_canvas();
	assert_ptr_equal(owner, main_canvas);
	obs_canvas_release(owner);
	assert_non_null(obs_scene_add(b, group));

	obs_source_t *plain = obs_source_create(PLAIN_ID, "rules walk plain", NULL, NULL);
	assert_non_null(plain);
	assert_non_null(obs_scene_add(a, plain));
	assert_non_null(obs_scene_add(b, plain));

	/* removing the group from its canvas removes it from every scene of the
	 * canvas */
	obs_source_get_ref(group);
	obs_source_remove(group);
	assert_null(obs_scene_find_source(a, "rules walk group"));
	assert_null(obs_scene_find_source(b, "rules walk group"));
	obs_source_t *gone = obs_canvas_get_source_by_name(main_canvas, "rules walk group");
	assert_null(gone);
	obs_source_release(group);

	/* the walk is for groups only: a plain source leaves its canvas, but its
	 * items stay in the scenes until they are removed */
	obs_source_remove(plain);
	assert_null(obs_canvas_get_source_by_name(main_canvas, "rules walk plain"));
	obs_sceneitem_t *left_a = obs_scene_find_source(a, "rules walk plain");
	obs_sceneitem_t *left_b = obs_scene_find_source(b, "rules walk plain");
	assert_non_null(left_a);
	assert_non_null(left_b);
	obs_sceneitem_remove(left_a);
	obs_sceneitem_remove(left_b);
	obs_source_release(plain);

	obs_canvas_release(main_canvas);
	obs_source_remove(obs_scene_get_source(a));
	obs_source_remove(obs_scene_get_source(b));
	obs_scene_release(a);
	obs_scene_release(b);
	obs_wait_for_destroy_queue();
}

static void rename_source_signals(void **state)
{
	(void)state;
	REQUIRE_OBS();

	obs_canvas_t *main_canvas = obs_get_main_canvas();
	obs_canvas_t *other = obs_canvas_create("rules rename other", NULL, PROGRAM);
	obs_scene_t *on_main = obs_scene_create("rules rename main scene");
	obs_scene_t *on_other = obs_canvas_scene_create(other, "rules rename other scene");
	struct counter main_own = {0}, other_own = {0}, global = {0};
	signal_handler_connect(obs_canvas_get_signal_handler(main_canvas), "source_rename", count_cb, &main_own);
	signal_handler_connect(obs_canvas_get_signal_handler(other), "source_rename", count_cb, &other_own);
	signal_handler_connect(obs_get_signal_handler(), "source_rename", count_cb, &global);

	obs_source_set_name(obs_scene_get_source(on_main), "rules rename main scene 2");
	assert_int_equal(main_own.calls, 1);
	assert_int_equal(global.calls, 1);
	assert_string_equal(global.last, "rules rename main scene 2");

	/* only the main canvas signals source renames globally */
	obs_source_set_name(obs_scene_get_source(on_other), "rules rename other scene 2");
	assert_int_equal(other_own.calls, 1);
	assert_string_equal(other_own.last, "rules rename other scene 2");
	assert_int_equal(global.calls, 1);
	obs_scene_t *found = obs_canvas_get_scene_by_name(other, "rules rename other scene 2");
	assert_ptr_equal(found, on_other);
	obs_scene_release(found);

	signal_handler_disconnect(obs_canvas_get_signal_handler(main_canvas), "source_rename", count_cb, &main_own);
	signal_handler_disconnect(obs_canvas_get_signal_handler(other), "source_rename", count_cb, &other_own);
	signal_handler_disconnect(obs_get_signal_handler(), "source_rename", count_cb, &global);
	obs_source_remove(obs_scene_get_source(on_main));
	obs_scene_release(on_main);
	obs_scene_release(on_other);
	obs_canvas_release(other);
	obs_canvas_release(main_canvas);
	obs_wait_for_destroy_queue();
}

/* ------------------------------------------------------------------------ */
/* Channels                                                                  */

static obs_source_t *replacement;
static int channel_changes;

static void replace_cb(void *data, calldata_t *cd)
{
	(void)data;
	channel_changes++;
	if (replacement)
		calldata_set_ptr(cd, "source", replacement);
}

static void channel_replace(void **state)
{
	(void)state;
	REQUIRE_OBS();

	obs_canvas_t *canvas = obs_canvas_create("rules channel", NULL, PROGRAM);
	obs_source_t *a = obs_source_create_private(PLAIN_ID, "rules channel a", NULL);
	obs_source_t *b = obs_source_create_private(PLAIN_ID, "rules channel b", NULL);
	assert_non_null(canvas);
	assert_non_null(a);
	assert_non_null(b);
	obs_weak_source_t *weak_a = obs_source_get_weak_source(a);
	obs_weak_source_t *weak_b = obs_source_get_weak_source(b);
	signal_handler_t *sh = obs_canvas_get_signal_handler(canvas);
	signal_handler_connect(sh, "channel_change", replace_cb, NULL);
	channel_changes = 0;

	/* the handler replaces a with b: b is stored and activated */
	replacement = b;
	obs_canvas_set_channel(canvas, 0, a);
	replacement = NULL;
	assert_int_equal(channel_changes, 1);
	obs_source_t *got = obs_canvas_get_channel(canvas, 0);
	assert_ptr_equal(got, b);
	obs_source_release(got);
	assert_true(obs_source_active(b));
	assert_false(obs_source_active(a));

	/* setting b itself is the same source: nothing happens */
	obs_canvas_set_channel(canvas, 0, b);
	assert_int_equal(channel_changes, 1);

	/* characterized, not endorsed: b was stored without a reference of its
	 * own, so clearing the channel releases one more reference on b than the
	 * canvas took (the test's extra one), and the reference taken on a is
	 * never released */
	obs_source_get_ref(b);
	obs_canvas_set_channel(canvas, 0, NULL);
	assert_int_equal(channel_changes, 2);
	assert_false(obs_source_active(b));
	obs_source_release(a);
	obs_source_release(b);
	obs_wait_for_destroy_queue();
	assert_false(source_alive(weak_b));
	assert_true(source_alive(weak_a));
	/* drop the reference the canvas kept on a */
	obs_source_t *leaked = obs_weak_source_get_source(weak_a);
	obs_source_release(leaked);
	obs_source_release(leaked);
	obs_wait_for_destroy_queue();
	assert_false(source_alive(weak_a));

	signal_handler_disconnect(sh, "channel_change", replace_cb, NULL);
	obs_weak_source_release(weak_a);
	obs_weak_source_release(weak_b);
	obs_canvas_release(canvas);
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
	struct obs_video_info ovi = video_info();
	int ret = obs_reset_video(&ovi);
	if (ret != OBS_VIDEO_SUCCESS) {
		fprintf(stderr, "skipped: obs_reset_video failed: %d\n", ret);
		obs_shutdown();
		obs_up = false;
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
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(constructor_flags), cmocka_unit_test(save_rules),
		cmocka_unit_test(reset_rules),       cmocka_unit_test(set_name_rules),
		cmocka_unit_test(remove_rules),      cmocka_unit_test(scene_refs),
		cmocka_unit_test(group_walk),        cmocka_unit_test(rename_source_signals),
		cmocka_unit_test(channel_replace),
	};

	return cmocka_run_group_tests(tests, setup, teardown);
}
