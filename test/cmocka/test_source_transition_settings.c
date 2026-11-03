/*
 * Characterization of the transition settings functions of
 * libobs/obs-source-transition.c (obs-rust/obs-studio#3529), before they move
 * to their own file and are ported: obs_transition_is_active(), the scale
 * type, alignment and size setters and getters, obs_transition_save() and
 * the fixed duration switch.
 *
 * test_transition_scale_type pins the scale type through the matrix a source
 * is drawn with. These cases pin each getter after each setter, the values a
 * new transition starts with, what each getter returns for NULL and for a
 * source that is not a transition, and every key and value
 * obs_transition_save() writes, through the real libobs. Odd behaviour is
 * characterized, not endorsed.
 *
 * Transitions take core video for their render targets, so the test only
 * builds on Linux and runs under xvfb-run; without a display the cases skip.
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include <obs.h>

#define CHILD_ID "test_tr_settings_child"
#define TRANSITION_ID "test_tr_settings_transition"

static bool obs_up;

static const char *child_name(void *unused)
{
	UNUSED_PARAMETER(unused);
	return "settings child";
}

static void *child_create(obs_data_t *settings, obs_source_t *source)
{
	UNUSED_PARAMETER(settings);
	return source;
}

static void child_destroy(void *data)
{
	UNUSED_PARAMETER(data);
}

static uint32_t child_size(void *data)
{
	UNUSED_PARAMETER(data);
	return 16;
}

static struct obs_source_info child_info = {
	.id = CHILD_ID,
	.type = OBS_SOURCE_TYPE_INPUT,
	.output_flags = OBS_SOURCE_VIDEO,
	.get_name = child_name,
	.create = child_create,
	.destroy = child_destroy,
	.get_width = child_size,
	.get_height = child_size,
};

static const char *tr_name(void *unused)
{
	UNUSED_PARAMETER(unused);
	return "settings transition";
}

static void *tr_create(obs_data_t *settings, obs_source_t *source)
{
	UNUSED_PARAMETER(settings);
	return source;
}

static void tr_destroy(void *data)
{
	UNUSED_PARAMETER(data);
}

static float mix_a(void *data, float t)
{
	UNUSED_PARAMETER(data);
	return 1.0f - t;
}

static float mix_b(void *data, float t)
{
	UNUSED_PARAMETER(data);
	return t;
}

static bool tr_audio_render(void *data, uint64_t *ts_out, struct obs_source_audio_mix *audio, uint32_t mixers,
			    size_t channels, size_t sample_rate)
{
	return obs_transition_audio_render(data, ts_out, audio, mixers, channels, sample_rate, mix_a, mix_b);
}

static struct obs_source_info transition_info = {
	.id = TRANSITION_ID,
	.type = OBS_SOURCE_TYPE_TRANSITION,
	.output_flags = OBS_SOURCE_VIDEO,
	.get_name = tr_name,
	.create = tr_create,
	.destroy = tr_destroy,
	.audio_render = tr_audio_render,
};

/* The sources of the running case, released by case_teardown if a failed
 * assertion ended the case first. */
static struct {
	obs_source_t *tr;
	obs_source_t *a;
	obs_source_t *b;
	obs_source_t *input;
	obs_data_t *saved;
} live;

static int case_teardown(void **state)
{
	UNUSED_PARAMETER(state);
	if (live.tr)
		obs_transition_clear(live.tr);
	obs_source_release(live.tr);
	obs_source_release(live.a);
	obs_source_release(live.b);
	obs_source_release(live.input);
	obs_data_release(live.saved);
	memset(&live, 0, sizeof(live));
	return 0;
}

static void skip_without_core(void)
{
	if (!obs_up)
		skip();
}

static obs_source_t *make_transition(const char *name)
{
	obs_source_t *tr = obs_source_create_private(TRANSITION_ID, name, NULL);
	assert_non_null(tr);
	assert_int_equal(obs_source_get_type(tr), OBS_SOURCE_TYPE_TRANSITION);
	return tr;
}

static obs_source_t *make_child(const char *name)
{
	obs_source_t *child = obs_source_create_private(CHILD_ID, name, NULL);
	assert_non_null(child);
	return child;
}

static void assert_size(obs_source_t *tr, uint32_t want_cx, uint32_t want_cy)
{
	uint32_t cx = 0xdeadbeef;
	uint32_t cy = 0xdeadbeef;
	obs_transition_get_size(tr, &cx, &cy);
	assert_int_equal(cx, want_cx);
	assert_int_equal(cy, want_cy);
}

/* A new transition: aligned left and top (obs_transition_init()), size 0x0,
 * scale type MAX_ONLY, no fixed duration, not transitioning. */
static void new_transition_defaults(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	live.tr = make_transition("defaults");

	assert_int_equal(obs_transition_get_alignment(live.tr), OBS_ALIGN_LEFT | OBS_ALIGN_TOP);
	assert_size(live.tr, 0, 0);
	assert_int_equal(obs_transition_get_scale_type(live.tr), OBS_TRANSITION_SCALE_MAX_ONLY);
	assert_false(obs_transition_fixed(live.tr));
	assert_false(obs_transition_is_active(live.tr));
}

static void alignment_after_set(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	live.tr = make_transition("alignment");

	static const uint32_t values[] = {OBS_ALIGN_CENTER, OBS_ALIGN_RIGHT | OBS_ALIGN_BOTTOM,
					  OBS_ALIGN_LEFT,   OBS_ALIGN_TOP,
					  0x5a5a5a5a,       UINT32_MAX,
					  OBS_ALIGN_CENTER};
	for (size_t i = 0; i < sizeof(values) / sizeof(values[0]); i++) {
		/* Any value is stored as given, unchecked. */
		obs_transition_set_alignment(live.tr, values[i]);
		assert_int_equal(obs_transition_get_alignment(live.tr), values[i]);
	}
}

static void size_after_set(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	live.tr = make_transition("size");

	obs_transition_set_size(live.tr, 1920, 1080);
	assert_size(live.tr, 1920, 1080);
	obs_transition_set_size(live.tr, 0, 77);
	assert_size(live.tr, 0, 77);
	obs_transition_set_size(live.tr, UINT32_MAX, 1);
	assert_size(live.tr, UINT32_MAX, 1);
	obs_transition_set_size(live.tr, 0, 0);
	assert_size(live.tr, 0, 0);
}

static void scale_type_after_set(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	live.tr = make_transition("scale type");

	obs_transition_set_scale_type(live.tr, OBS_TRANSITION_SCALE_STRETCH);
	assert_int_equal(obs_transition_get_scale_type(live.tr), OBS_TRANSITION_SCALE_STRETCH);
	obs_transition_set_scale_type(live.tr, OBS_TRANSITION_SCALE_ASPECT);
	assert_int_equal(obs_transition_get_scale_type(live.tr), OBS_TRANSITION_SCALE_ASPECT);
	obs_transition_set_scale_type(live.tr, OBS_TRANSITION_SCALE_MAX_ONLY);
	assert_int_equal(obs_transition_get_scale_type(live.tr), OBS_TRANSITION_SCALE_MAX_ONLY);

	/* Out of range is stored as MAX_ONLY, from any earlier value. */
	static const int out_of_range[] = {OBS_TRANSITION_SCALE_STRETCH + 1, -1, INT32_MAX, INT32_MIN};
	for (size_t i = 0; i < sizeof(out_of_range) / sizeof(out_of_range[0]); i++) {
		obs_transition_set_scale_type(live.tr, OBS_TRANSITION_SCALE_STRETCH);
		obs_transition_set_scale_type(live.tr, (enum obs_transition_scale_type)out_of_range[i]);
		assert_int_equal(obs_transition_get_scale_type(live.tr), OBS_TRANSITION_SCALE_MAX_ONLY);
	}
}

static void fixed_after_enable(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	live.tr = make_transition("fixed");

	obs_transition_enable_fixed(live.tr, true, 300);
	assert_true(obs_transition_fixed(live.tr));
	obs_transition_enable_fixed(live.tr, false, 300);
	assert_false(obs_transition_fixed(live.tr));
	obs_transition_enable_fixed(live.tr, true, 0);
	assert_true(obs_transition_fixed(live.tr));
	obs_transition_enable_fixed(live.tr, false, 0);
	assert_false(obs_transition_fixed(live.tr));
}

/* Transitioning from the start until obs_transition_set() or
 * obs_transition_clear() ends it. Nothing renders the transition, so a
 * started one stays transitioning; obs_transition_force_stop() only signals
 * the stop and leaves it transitioning (characterized, not endorsed). */
static void is_active_follows_the_transition(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	live.tr = make_transition("is active");
	live.a = make_child("is active A");
	live.b = make_child("is active B");

	obs_transition_set(live.tr, live.a);
	assert_false(obs_transition_is_active(live.tr));

	assert_true(obs_transition_start(live.tr, OBS_TRANSITION_MODE_AUTO, 60000, live.b));
	assert_true(obs_transition_is_active(live.tr));

	obs_transition_force_stop(live.tr);
	assert_true(obs_transition_is_active(live.tr));

	obs_transition_set(live.tr, live.b);
	assert_false(obs_transition_is_active(live.tr));

	assert_true(obs_transition_start(live.tr, OBS_TRANSITION_MODE_AUTO, 60000, live.a));
	assert_true(obs_transition_is_active(live.tr));
	obs_transition_clear(live.tr);
	assert_false(obs_transition_is_active(live.tr));
}

/* NULL and a source that is not a transition: every setter does nothing and
 * every getter returns its default. obs_transition_get_size() zeroes both
 * outputs. */
static void not_a_transition_defaults(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	live.input = make_child("not a transition");

	obs_source_t *sources[] = {NULL, live.input};
	for (size_t i = 0; i < 2; i++) {
		obs_source_t *s = sources[i];

		obs_transition_set_scale_type(s, OBS_TRANSITION_SCALE_STRETCH);
		obs_transition_set_alignment(s, OBS_ALIGN_CENTER);
		obs_transition_set_size(s, 640, 480);
		obs_transition_enable_fixed(s, true, 300);

		assert_int_equal(obs_transition_get_scale_type(s), OBS_TRANSITION_SCALE_MAX_ONLY);
		assert_int_equal(obs_transition_get_alignment(s), 0);
		assert_size(s, 0, 0);
		assert_false(obs_transition_fixed(s));
	}

	/* obs_transition_is_active() has no check: it reads the flags of any
	 * source, which are false for an input. */
	assert_false(obs_transition_is_active(live.input));
}

/* obs_transition_save() is not exported: obs_save_source() calls it for a
 * transition, into the data it returns. */
static obs_data_t *save(obs_source_t *tr)
{
	obs_data_release(live.saved);
	live.saved = obs_save_source(tr);
	assert_non_null(live.saved);
	return live.saved;
}

static size_t count_transition_keys(obs_data_t *data)
{
	size_t n = 0;
	for (obs_data_item_t *item = obs_data_first(data); item; obs_data_item_next(&item))
		if (strncmp(obs_data_item_get_name(item), "transition_", 11) == 0)
			n++;
	return n;
}

/* The six keys obs_transition_save() writes and their values, and no other
 * transition_ key. */
static void assert_saved(obs_data_t *data, const char *source_a, int64_t alignment, int64_t mode, int64_t scale_type,
			 int64_t cx, int64_t cy)
{
	assert_int_equal(count_transition_keys(data), 6);
	assert_true(obs_data_has_user_value(data, "transition_source_a"));
	assert_string_equal(obs_data_get_string(data, "transition_source_a"), source_a);
	assert_int_equal(obs_data_get_int(data, "transition_alignment"), alignment);
	assert_int_equal(obs_data_get_int(data, "transition_mode"), mode);
	assert_int_equal(obs_data_get_int(data, "transition_scale_type"), scale_type);
	assert_int_equal(obs_data_get_int(data, "transition_cx"), cx);
	assert_int_equal(obs_data_get_int(data, "transition_cy"), cy);
}

/* With no child, the name is written as "", not left out. */
static void save_with_no_child(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	live.tr = make_transition("save empty");

	assert_saved(save(live.tr), "", OBS_ALIGN_LEFT | OBS_ALIGN_TOP, OBS_TRANSITION_MODE_AUTO,
		     OBS_TRANSITION_SCALE_MAX_ONLY, 0, 0);
}

static void save_writes_the_settings(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	live.tr = make_transition("save settings");
	live.a = make_child("save A");

	obs_transition_set_alignment(live.tr, OBS_ALIGN_RIGHT | OBS_ALIGN_BOTTOM);
	obs_transition_set_scale_type(live.tr, OBS_TRANSITION_SCALE_STRETCH);
	obs_transition_set_size(live.tr, 1280, 720);
	/* Not saved. */
	obs_transition_enable_fixed(live.tr, true, 300);
	obs_transition_set(live.tr, live.a);

	assert_saved(save(live.tr), "save A", OBS_ALIGN_RIGHT | OBS_ALIGN_BOTTOM, OBS_TRANSITION_MODE_AUTO,
		     OBS_TRANSITION_SCALE_STRETCH, 1280, 720);

	/* Each save reads the fields again. */
	obs_transition_set_alignment(live.tr, UINT32_MAX);
	obs_transition_set_size(live.tr, UINT32_MAX, 0);
	assert_saved(save(live.tr), "save A", UINT32_MAX, OBS_TRANSITION_MODE_AUTO, OBS_TRANSITION_SCALE_STRETCH,
		     UINT32_MAX, 0);
}

/* While transitioning, the saved source is slot B (the destination), and
 * the saved mode is the mode of the running transition. */
static void save_while_transitioning(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	live.tr = make_transition("save running");
	live.a = make_child("running A");
	live.b = make_child("running B");
	obs_transition_set_size(live.tr, 64, 32);

	obs_transition_set(live.tr, live.a);
	assert_true(obs_transition_start(live.tr, OBS_TRANSITION_MODE_MANUAL, 0, live.b));

	assert_saved(save(live.tr), "running B", OBS_ALIGN_LEFT | OBS_ALIGN_TOP, OBS_TRANSITION_MODE_MANUAL,
		     OBS_TRANSITION_SCALE_MAX_ONLY, 64, 32);

	/* obs_transition_force_stop() leaves it transitioning: still slot B. */
	obs_transition_force_stop(live.tr);
	assert_saved(save(live.tr), "running B", OBS_ALIGN_LEFT | OBS_ALIGN_TOP, OBS_TRANSITION_MODE_MANUAL,
		     OBS_TRANSITION_SCALE_MAX_ONLY, 64, 32);

	/* Once obs_transition_set() ends it, slot A again; the mode stays. */
	obs_transition_set(live.tr, live.a);
	assert_false(obs_transition_is_active(live.tr));
	assert_saved(save(live.tr), "running A", OBS_ALIGN_LEFT | OBS_ALIGN_TOP, OBS_TRANSITION_MODE_MANUAL,
		     OBS_TRANSITION_SCALE_MAX_ONLY, 64, 32);
}

static int group_setup(void **state)
{
	UNUSED_PARAMETER(state);
	obs_up = obs_startup("en-US", NULL, NULL);
	if (!obs_up) {
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
		return 0;
	}

	struct obs_video_info ovi = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = 30,
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

	obs_register_source(&child_info);
	obs_register_source(&transition_info);
	return 0;
}

static int group_teardown(void **state)
{
	UNUSED_PARAMETER(state);
	if (obs_up)
		obs_shutdown();
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_teardown(new_transition_defaults, case_teardown),
		cmocka_unit_test_teardown(alignment_after_set, case_teardown),
		cmocka_unit_test_teardown(size_after_set, case_teardown),
		cmocka_unit_test_teardown(scale_type_after_set, case_teardown),
		cmocka_unit_test_teardown(fixed_after_enable, case_teardown),
		cmocka_unit_test_teardown(is_active_follows_the_transition, case_teardown),
		cmocka_unit_test_teardown(not_a_transition_defaults, case_teardown),
		cmocka_unit_test_teardown(save_with_no_child, case_teardown),
		cmocka_unit_test_teardown(save_writes_the_settings, case_teardown),
		cmocka_unit_test_teardown(save_while_transitioning, case_teardown),
	};
	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
