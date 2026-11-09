/* Characterization of the effect destroy (libobs/graphics/effect.c.h,
 * #4103) through the real libobs: gs_effect_destroy() and the
 * gs_effect_actually_destroy() it shares with gs_destroy().
 *
 * - An effect made by gs_effect_create() with no file name is not cached:
 *   gs_effect_destroy() frees all of it (its params with their annotations,
 *   its techniques with their passes and shaders, the arrays) and the
 *   allocation count is back to where it was before the create, also after
 *   the effect has drawn.
 * - A text that does not parse: gs_effect_create() destroys the half-built
 *   effect itself, leaving only the error string.
 * - gs_effect_destroy(NULL) does nothing.
 * - An effect from gs_effect_create_from_file() is cached: gs_effect_destroy()
 *   leaves it, gs_effect_create_from_file() hands the same effect back
 *   without allocating, and it still draws.
 * - gs_destroy() frees the cached effects: after it the allocation count is
 *   back to where it was before gs_create().
 *
 * The cases make the OpenGL device with gs_create() and enter its context
 * on the test thread, without obs_startup(): bnum_allocs() counts the whole
 * process, and the core's threads would allocate while it is read. The
 * device needs a display on Linux, so the test runs under xvfb-run there.
 * It skips only when no display is set at all; with one set, a failed
 * gs_create() fails the test. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <obs.h>
#include <util/bmem.h>
#include <util/platform.h>

#define EFFECT_FILE "test_effect_destroy.effect"

static graphics_t *graphics;

/* Two techniques, one with two passes; params with and without
 * annotations, an annotation with a value and one with a default. */
static const char effect_text[] =
	"uniform float4x4 ViewProj;\n"
	"uniform float4 color <string name = \"Color\"; float minimum = 0.0;> = {1.0, 0.5, 0.25, 1.0};\n"
	"uniform float gain <int step = 2;> = 1.5;\n"
	"uniform float level;\n"
	"\n"
	"struct VertInOut {\n"
	"\tfloat4 pos : POSITION;\n"
	"};\n"
	"\n"
	"VertInOut VSDefault(VertInOut vert_in)\n"
	"{\n"
	"\tVertInOut vert_out;\n"
	"\tvert_out.pos = mul(float4(vert_in.pos.xyz, 1.0), ViewProj);\n"
	"\treturn vert_out;\n"
	"}\n"
	"\n"
	"float4 PSColor(VertInOut vert_in) : TARGET\n"
	"{\n"
	"\treturn color * gain;\n"
	"}\n"
	"\n"
	"float4 PSLevel(VertInOut vert_in) : TARGET\n"
	"{\n"
	"\treturn float4(level, level, level, 1.0);\n"
	"}\n"
	"\n"
	"technique Draw\n"
	"{\n"
	"\tpass\n"
	"\t{\n"
	"\t\tvertex_shader = VSDefault(vert_in);\n"
	"\t\tpixel_shader  = PSColor(vert_in);\n"
	"\t}\n"
	"\tpass second\n"
	"\t{\n"
	"\t\tvertex_shader = VSDefault(vert_in);\n"
	"\t\tpixel_shader  = PSLevel(vert_in);\n"
	"\t}\n"
	"}\n"
	"\n"
	"technique Level\n"
	"{\n"
	"\tpass\n"
	"\t{\n"
	"\t\tvertex_shader = VSDefault(vert_in);\n"
	"\t\tpixel_shader  = PSLevel(vert_in);\n"
	"\t}\n"
	"}\n";

/* Parses up to the technique, then stops at a missing '{'. */
static const char bad_text[] = "uniform float4x4 ViewProj;\n"
			       "uniform float gain <int step = 2;> = 1.5;\n"
			       "technique Draw\n"
			       "pass\n";

struct effect_shape {
	bool draw, level, color, gain;
	size_t color_annotations, gain_annotations;
};

static struct effect_shape shape_of(gs_effect_t *effect)
{
	struct effect_shape shape = {0};
	gs_eparam_t *color = gs_effect_get_param_by_name(effect, "color");
	gs_eparam_t *gain = gs_effect_get_param_by_name(effect, "gain");

	shape.draw = gs_effect_get_technique(effect, "Draw") != NULL;
	shape.level = gs_effect_get_technique(effect, "Level") != NULL;
	shape.color = color != NULL;
	shape.gain = gain != NULL;
	shape.color_annotations = color ? gs_param_get_num_annotations(color) : 0;
	shape.gain_annotations = gain ? gs_param_get_num_annotations(gain) : 0;
	return shape;
}

static void assert_shape(const struct effect_shape *shape)
{
	assert_true(shape->draw);
	assert_true(shape->level);
	assert_true(shape->color);
	assert_true(shape->gain);
	assert_int_equal(shape->color_annotations, 2);
	assert_int_equal(shape->gain_annotations, 1);
}

/* Each pass of the technique once, as a draw does; true when every begin
 * succeeded. */
static bool draw_with(gs_effect_t *effect, const char *technique)
{
	gs_technique_t *tech = gs_effect_get_technique(effect, technique);
	if (!tech)
		return false;

	bool ok = true;
	size_t passes = gs_technique_begin(tech);
	if (passes == 0)
		ok = false;
	for (size_t i = 0; i < passes; i++) {
		if (!gs_technique_begin_pass(tech, i)) {
			ok = false;
			continue;
		}
		gs_technique_end_pass(tech);
	}
	gs_technique_end(tech);
	return ok;
}

static void uncached_destroy_frees_everything(void **state)
{
	(void)state;
	if (!graphics)
		skip();

	gs_enter_context(graphics);
	long before = bnum_allocs();
	char *errors = NULL;
	gs_effect_t *effect = gs_effect_create(effect_text, NULL, &errors);
	if (!effect)
		fprintf(stderr, "gs_effect_create failed: %s\n", errors ? errors : "(no errors)");
	bfree(errors);
	long created = bnum_allocs();
	struct effect_shape shape = {0};
	if (effect)
		shape = shape_of(effect);
	gs_effect_destroy(effect);
	long after = bnum_allocs();
	gs_leave_context();

	assert_non_null(effect);
	assert_shape(&shape);
	assert_true(created > before);
	if (after != before)
		fprintf(stderr, "allocations: %ld before the create, %ld after the destroy\n", before, after);
	assert_int_equal(after, before);
}

static void uncached_destroy_after_draw(void **state)
{
	(void)state;
	if (!graphics)
		skip();

	gs_enter_context(graphics);
	long before = bnum_allocs();
	gs_effect_t *effect = gs_effect_create(effect_text, NULL, NULL);
	bool drew = false, drew_level = false;
	if (effect) {
		drew = draw_with(effect, "Draw");
		drew_level = draw_with(effect, "Level");
	}
	gs_effect_destroy(effect);
	long after = bnum_allocs();
	gs_leave_context();

	assert_non_null(effect);
	assert_true(drew);
	assert_true(drew_level);
	if (after != before)
		fprintf(stderr, "allocations: %ld before the create, %ld after the destroy\n", before, after);
	assert_int_equal(after, before);
}

/* gs_effect_create() calls gs_effect_destroy() on the effect it could not
 * parse: with a file name its effect_path is set, but it is not cached yet,
 * so the destroy frees it. Only the error string is left. */
static void failed_create_destroys_effect(void **state)
{
	(void)state;
	if (!graphics)
		skip();

	gs_enter_context(graphics);
	long before = bnum_allocs();
	char *errors = NULL;
	gs_effect_t *effect = gs_effect_create(bad_text, "bad.effect", &errors);
	long after = bnum_allocs();
	bool had_errors = errors && *errors;
	bfree(errors);
	long freed = bnum_allocs();
	gs_leave_context();

	assert_null(effect);
	assert_true(had_errors);
	assert_int_equal(after, before + 1);
	assert_int_equal(freed, before);
}

static void destroy_null_does_nothing(void **state)
{
	(void)state;

	long before = bnum_allocs();
	gs_effect_destroy(NULL);
	assert_int_equal(bnum_allocs(), before);

	if (!graphics)
		return;
	gs_enter_context(graphics);
	before = bnum_allocs();
	gs_effect_destroy(NULL);
	long after = bnum_allocs();
	gs_leave_context();
	assert_int_equal(after, before);
}

static void cached_effect_survives_destroy(void **state)
{
	(void)state;
	if (!graphics)
		skip();

	gs_enter_context(graphics);
	char *errors = NULL;
	gs_effect_t *effect = gs_effect_create_from_file(EFFECT_FILE, &errors);
	if (!effect)
		fprintf(stderr, "gs_effect_create_from_file failed: %s\n", errors ? errors : "(no errors)");
	bfree(errors);

	long before = bnum_allocs();
	gs_effect_destroy(effect);
	long destroyed = bnum_allocs();
	gs_effect_destroy(effect);
	long destroyed_twice = bnum_allocs();
	gs_effect_t *again = gs_effect_create_from_file(EFFECT_FILE, NULL);
	long found = bnum_allocs();
	struct effect_shape shape = {0};
	bool drew = false;
	if (again) {
		shape = shape_of(again);
		drew = draw_with(again, "Draw");
	}
	gs_leave_context();

	assert_non_null(effect);
	assert_ptr_equal(again, effect);
	assert_int_equal(destroyed, before);
	assert_int_equal(destroyed_twice, before);
	assert_int_equal(found, before);
	assert_shape(&shape);
	assert_true(drew);
}

static bool display_set(void)
{
	return getenv("DISPLAY") || getenv("WAYLAND_DISPLAY");
}

/* gs_create() of the OpenGL device; NULL when it fails. */
static graphics_t *create_graphics(void)
{
	graphics_t *created = NULL;
	int ret = gs_create(&created, GRAPHICS_MODULE_PATH, 0);
	if (ret != GS_SUCCESS) {
		fprintf(stderr, "gs_create failed: %d\n", ret);
		return NULL;
	}
	return created;
}

/* Its own device: gs_destroy() frees the cached effect, so nothing
 * gs_create() and the effect allocated is left. */
static void gs_destroy_frees_cached_effect(void **state)
{
	(void)state;

	long before = bnum_allocs();
	graphics_t *own = create_graphics();
	if (!own) {
		if (display_set()) {
			fprintf(stderr, "gs_create failed with a display set\n");
			fail();
		}
		skip();
	}

	gs_enter_context(own);
	long started = bnum_allocs();
	gs_effect_t *effect = gs_effect_create_from_file(EFFECT_FILE, NULL);
	long created = bnum_allocs();
	gs_effect_destroy(effect);
	long destroyed = bnum_allocs();
	gs_leave_context();
	gs_destroy(own);
	long after = bnum_allocs();

	assert_non_null(effect);
	assert_true(created > started);
	assert_int_equal(destroyed, created);
	if (after != before)
		fprintf(stderr, "allocations: %ld before gs_create, %ld after gs_destroy\n", before, after);
	assert_int_equal(after, before);
}

static int write_effect_file(void)
{
	if (!os_quick_write_utf8_file(EFFECT_FILE, effect_text, strlen(effect_text), false)) {
		fprintf(stderr, "could not write %s\n", EFFECT_FILE);
		return -1;
	}
	return 0;
}

static int group_setup(void **state)
{
	(void)state;
	if (write_effect_file() != 0)
		return -1;
	graphics = create_graphics();
	if (!graphics) {
		if (display_set()) {
			fprintf(stderr, "gs_create failed with a display set\n");
			return -1;
		}
		fprintf(stderr, "skipped: gs_create failed (no display?)\n");
	}
	return 0;
}

static int group_teardown(void **state)
{
	(void)state;
	gs_destroy(graphics);
	graphics = NULL;
	return 0;
}

static int shutdown_setup(void **state)
{
	(void)state;
	return write_effect_file();
}

static int shutdown_teardown(void **state)
{
	(void)state;
	os_unlink(EFFECT_FILE);
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(uncached_destroy_frees_everything), cmocka_unit_test(uncached_destroy_after_draw),
		cmocka_unit_test(failed_create_destroys_effect),     cmocka_unit_test(destroy_null_does_nothing),
		cmocka_unit_test(cached_effect_survives_destroy),
	};
	const struct CMUnitTest shutdown_tests[] = {
		cmocka_unit_test(gs_destroy_frees_cached_effect),
	};
	int failed = cmocka_run_group_tests_name("test_effect_destroy", tests, group_setup, group_teardown);
	failed += cmocka_run_group_tests_name("test_effect_destroy_shutdown", shutdown_tests, shutdown_setup,
					      shutdown_teardown);
	return failed;
}
