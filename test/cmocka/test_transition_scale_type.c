/*
 * A transition scale type outside enum obs_transition_scale_type must act as
 * the default, OBS_TRANSITION_SCALE_MAX_ONLY (obs-rust/obs-studio#2064).
 *
 * recalculate_transition_matrix() only sets the scale for the three enum
 * values, so any other value left it uninitialized and put a garbage float
 * into the matrix a transition draws its sources with. A scene collection
 * reaches it through obs_load_sources(), which passes "transition_scale_type"
 * to the transition unchecked, and a caller through
 * obs_transition_set_scale_type().
 *
 * The cases fit a 32x16 source into a 64x64 transition, where the three
 * scale types give three different matrices, and read the matrix the source
 * is drawn with through obs_transition_video_render_direct(). They check the
 * getter and that matrix for the in-range types (behaviour that must not
 * change) and for out-of-range ones, set through the setter and loaded from
 * obs_data. Core video needs a display on Linux (run under xvfb-run); without
 * one the cases skip.
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include <obs.h>
#include <graphics/matrix4.h>

#define CHILD_ID "test_scale_type_child"
#define TRANSITION_ID "test_scale_type_transition"
#define TR_SIZE 64
#define CHILD_CX 32
#define CHILD_CY 16

static bool obs_up;

/* The matrix the last child render ran with. */
static struct matrix4 drawn;
static int draws;

struct child {
	uint32_t cx;
	uint32_t cy;
};

static const char *child_name(void *unused)
{
	UNUSED_PARAMETER(unused);
	return "scale type child";
}

static void *child_create(obs_data_t *settings, obs_source_t *source)
{
	UNUSED_PARAMETER(source);
	struct child *c = bzalloc(sizeof(*c));
	c->cx = (uint32_t)obs_data_get_int(settings, "cx");
	c->cy = (uint32_t)obs_data_get_int(settings, "cy");
	return c;
}

static void child_destroy(void *data)
{
	bfree(data);
}

static uint32_t child_width(void *data)
{
	return ((struct child *)data)->cx;
}

static uint32_t child_height(void *data)
{
	return ((struct child *)data)->cy;
}

static void child_render(void *data, gs_effect_t *effect)
{
	UNUSED_PARAMETER(data);
	UNUSED_PARAMETER(effect);
	gs_matrix_get(&drawn);
	draws++;
}

static struct obs_source_info child_info = {
	.id = CHILD_ID,
	.type = OBS_SOURCE_TYPE_INPUT,
	.output_flags = OBS_SOURCE_VIDEO | OBS_SOURCE_CUSTOM_DRAW,
	.get_name = child_name,
	.create = child_create,
	.destroy = child_destroy,
	.get_width = child_width,
	.get_height = child_height,
	.video_render = child_render,
};

static const char *tr_name(void *unused)
{
	UNUSED_PARAMETER(unused);
	return "scale type transition";
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

static void tr_video_render(void *data, gs_effect_t *effect)
{
	UNUSED_PARAMETER(effect);
	obs_transition_video_render(data, NULL);
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
	.video_render = tr_video_render,
	.audio_render = tr_audio_render,
};

static obs_source_t *create_child(const char *name)
{
	obs_data_t *settings = obs_data_create();
	obs_data_set_int(settings, "cx", CHILD_CX);
	obs_data_set_int(settings, "cy", CHILD_CY);
	obs_source_t *child = obs_source_create_private(CHILD_ID, name, settings);
	obs_data_release(settings);
	assert_non_null(child);
	return child;
}

/* The matrix source A of `tr` is drawn with. */
static void draw_matrix(obs_source_t *tr, struct matrix4 *out)
{
	draws = 0;
	obs_enter_graphics();
	gs_matrix_push();
	gs_matrix_identity();
	bool rendered = obs_transition_video_render_direct(tr, OBS_TRANSITION_SOURCE_A);
	gs_matrix_pop();
	obs_leave_graphics();
	UNUSED_PARAMETER(rendered);
	assert_int_equal(draws, 1);
	matrix4_copy(out, &drawn);
}

struct fit {
	float sx, sy;
	float px, py;
};

/* 32x16 in 64x64, centred. MAX_ONLY: the source fits, so no
 * scale. ASPECT: x2 to the width. STRETCH: x2 and x4 to fill. */
static const struct fit fit_max_only = {1.0f, 1.0f, 16.0f, 24.0f};
static const struct fit fit_aspect = {2.0f, 2.0f, 0.0f, 16.0f};
static const struct fit fit_stretch = {2.0f, 4.0f, 0.0f, 0.0f};

static const struct fit *expected_fit(enum obs_transition_scale_type type)
{
	switch (type) {
	case OBS_TRANSITION_SCALE_ASPECT:
		return &fit_aspect;
	case OBS_TRANSITION_SCALE_STRETCH:
		return &fit_stretch;
	case OBS_TRANSITION_SCALE_MAX_ONLY:
	default:
		return &fit_max_only;
	}
}

static bool row_is(const struct vec4 *row, float x, float y, float z, float w)
{
	return row->x == x && row->y == y && row->z == z && row->w == w;
}

static void assert_fit(const struct matrix4 *m, const struct fit *f, const char *label)
{
	bool ok = row_is(&m->x, f->sx, 0.0f, 0.0f, 0.0f) && row_is(&m->y, 0.0f, f->sy, 0.0f, 0.0f) &&
		  row_is(&m->z, 0.0f, 0.0f, 1.0f, 0.0f) && row_is(&m->t, f->px, f->py, 0.0f, 1.0f);
	if (!ok)
		print_error("%s: drawn with scale (%g, %g) offset (%g, %g), expected scale (%g, %g) offset "
			    "(%g, %g)\n",
			    label, m->x.x, m->y.y, m->t.x, m->t.y, f->sx, f->sy, f->px, f->py);
	assert_true(ok);
}

/* The sources of the running check, released by case_teardown if a failed
 * assertion ended the case before the check released them. */
static struct {
	obs_source_t *tr;
	obs_source_t *a;
	obs_source_t *b;
	obs_data_t *saved;
} live;

static void release_live(void)
{
	if (live.tr)
		obs_transition_clear(live.tr);
	obs_source_release(live.tr);
	obs_source_release(live.a);
	obs_source_release(live.b);
	obs_data_release(live.saved);
	memset(&live, 0, sizeof(live));
}

static int case_teardown(void **state)
{
	UNUSED_PARAMETER(state);
	release_live();
	return 0;
}

/* Set `type` with the setter, then start the transition, which recalculates
 * its matrices, and read the matrix of source A. */
static void check_setter(int type, enum obs_transition_scale_type want)
{
	char label[64];
	snprintf(label, sizeof(label), "set_scale_type(%d)", type);

	live.tr = obs_source_create_private(TRANSITION_ID, "set", NULL);
	live.a = create_child("set A");
	live.b = create_child("set B");
	assert_non_null(live.tr);

	obs_transition_set_size(live.tr, TR_SIZE, TR_SIZE);
	obs_transition_set_alignment(live.tr, OBS_ALIGN_CENTER);
	obs_transition_set_scale_type(live.tr, (enum obs_transition_scale_type)type);

	obs_transition_set(live.tr, live.a);
	assert_true(obs_transition_start(live.tr, OBS_TRANSITION_MODE_AUTO, 60000, live.b));

	struct matrix4 m;
	draw_matrix(live.tr, &m);
	assert_fit(&m, expected_fit(want), label);
	assert_int_equal(obs_transition_get_scale_type(live.tr), want);

	release_live();
}

static obs_source_t *loaded;

static void grab_transition(void *param, obs_source_t *source)
{
	UNUSED_PARAMETER(param);
	if (obs_source_get_type(source) == OBS_SOURCE_TYPE_TRANSITION) {
		assert_null(loaded);
		loaded = obs_source_get_ref(source);
	}
}

/* Load a scene collection's source array holding a child and a transition
 * on it with "transition_scale_type" = `type`, and read the matrix of
 * source A. */
static void check_load(int64_t type, enum obs_transition_scale_type want)
{
	static int loads;
	char label[64];
	char a_name[32];
	char load_name[32];
	snprintf(label, sizeof(label), "load transition_scale_type %lld", (long long)type);
	snprintf(a_name, sizeof(a_name), "load A %d", loads);
	snprintf(load_name, sizeof(load_name), "load transition %d", loads);
	loads++;

	obs_data_array_t *array = obs_data_array_create();

	obs_data_t *child = obs_data_create();
	obs_data_t *settings = obs_data_create();
	obs_data_set_int(settings, "cx", CHILD_CX);
	obs_data_set_int(settings, "cy", CHILD_CY);
	obs_data_set_string(child, "id", CHILD_ID);
	obs_data_set_string(child, "name", a_name);
	obs_data_set_obj(child, "settings", settings);
	obs_data_array_push_back(array, child);
	obs_data_release(settings);
	obs_data_release(child);

	obs_data_t *tr_data = obs_data_create();
	obs_data_set_string(tr_data, "id", TRANSITION_ID);
	obs_data_set_string(tr_data, "name", load_name);
	obs_data_set_string(tr_data, "transition_source_a", a_name);
	obs_data_set_int(tr_data, "transition_alignment", OBS_ALIGN_CENTER);
	obs_data_set_int(tr_data, "transition_mode", OBS_TRANSITION_MODE_AUTO);
	obs_data_set_int(tr_data, "transition_scale_type", type);
	obs_data_set_int(tr_data, "transition_cx", TR_SIZE);
	obs_data_set_int(tr_data, "transition_cy", TR_SIZE);
	obs_data_array_push_back(array, tr_data);
	obs_data_release(tr_data);

	loaded = NULL;
	obs_load_sources(array, grab_transition, NULL);
	obs_data_array_release(array);

	live.tr = loaded;
	loaded = NULL;
	assert_non_null(live.tr);

	struct matrix4 m;
	draw_matrix(live.tr, &m);
	assert_fit(&m, expected_fit(want), label);
	assert_int_equal(obs_transition_get_scale_type(live.tr), want);

	/* Saving writes the scale type it uses. */
	live.saved = obs_save_source(live.tr);
	assert_int_equal(obs_data_get_int(live.saved, "transition_scale_type"), want);

	release_live();
}

static void skip_without_core(void)
{
	if (!obs_up)
		skip();
}

static void in_range_setter_unchanged(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	check_setter(OBS_TRANSITION_SCALE_MAX_ONLY, OBS_TRANSITION_SCALE_MAX_ONLY);
	check_setter(OBS_TRANSITION_SCALE_ASPECT, OBS_TRANSITION_SCALE_ASPECT);
	check_setter(OBS_TRANSITION_SCALE_STRETCH, OBS_TRANSITION_SCALE_STRETCH);
}

static void in_range_load_unchanged(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	check_load(OBS_TRANSITION_SCALE_MAX_ONLY, OBS_TRANSITION_SCALE_MAX_ONLY);
	check_load(OBS_TRANSITION_SCALE_ASPECT, OBS_TRANSITION_SCALE_ASPECT);
	check_load(OBS_TRANSITION_SCALE_STRETCH, OBS_TRANSITION_SCALE_STRETCH);
}

static void out_of_range_setter_is_max_only(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	check_setter(OBS_TRANSITION_SCALE_STRETCH + 1, OBS_TRANSITION_SCALE_MAX_ONLY);
	check_setter(-1, OBS_TRANSITION_SCALE_MAX_ONLY);
	check_setter(INT32_MAX, OBS_TRANSITION_SCALE_MAX_ONLY);
}

static void out_of_range_load_is_max_only(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	check_load(OBS_TRANSITION_SCALE_STRETCH + 1, OBS_TRANSITION_SCALE_MAX_ONLY);
	check_load(-1, OBS_TRANSITION_SCALE_MAX_ONLY);
	/* Would truncate to OBS_TRANSITION_SCALE_STRETCH as an int. */
	check_load(((int64_t)1 << 32) + OBS_TRANSITION_SCALE_STRETCH, OBS_TRANSITION_SCALE_MAX_ONLY);
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
		cmocka_unit_test_teardown(in_range_setter_unchanged, case_teardown),
		cmocka_unit_test_teardown(in_range_load_unchanged, case_teardown),
		cmocka_unit_test_teardown(out_of_range_setter_is_max_only, case_teardown),
		cmocka_unit_test_teardown(out_of_range_load_is_max_only, case_teardown),
	};
	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
