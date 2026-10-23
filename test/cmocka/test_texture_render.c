#include <setjmp.h>
#include <stdarg.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>

#include <graphics/graphics-internal.h>
#include <graphics/matrix4.h>
#include <util/bmem.h>

/*
 * Characterization of libobs/graphics/texture-render.c.h (gs_texrender_*),
 * before the port to obs-graphics.
 *
 * No GPU. The fixture builds a graphics_t by hand and enters it with
 * gs_enter_context. Its exports are a fake device that records every call
 * texrender reaches through graphics.c, in order. The texrender functions
 * are called through libobs's exported symbols, so the same test runs
 * against the C build, the C reference and hybrid libobs.
 *
 * Matrix push, identity and pop never reach the device: the test reads the
 * matrix stack instead. A null texture or zstencil passed to
 * gs_texture_destroy / gs_zstencil_destroy returns in graphics.c before the
 * device, so those calls do not show in the log.
 *
 * Characterized, not endorsed:
 * - A failed texture create still stores the new size, so a second begin at
 *   that size does not retry and returns false. Another size retries.
 * - A second begin before end saves the texrender's own target as the
 *   previous one, so both ends restore that target, not the original.
 * - End without begin restores the zeroed previous state (no target, no
 *   zstencil, GS_CS_SRGB) and sets rendered, so the next begin fails until
 *   reset.
 * - The viewport size is the uint32_t size cast to int.
 */

/* printf-checked, so clang's -Wformat-nonliteral accepts the vsnprintf. */
#if defined(__GNUC__) || defined(__clang__)
#define RECORD_FORMAT __attribute__((format(printf, 1, 2)))
#else
#define RECORD_FORMAT
#endif

static char textures[16];
static char zstencils[16];
static char screen_target;
static char screen_zs;
static char device_token;

static size_t next_texture;
static size_t next_zstencil;
static bool fail_texture;
static bool fail_zstencil;

static gs_texture_t *device_target;
static gs_zstencil_t *device_zs;
static enum gs_color_space device_space;
static struct gs_rect device_viewport;
static int projection_depth;

static char call_log[8192];

RECORD_FORMAT static void record(const char *format, ...)
{
	size_t len = strlen(call_log);
	va_list args;

	if (len)
		call_log[len++] = ';';

	va_start(args, format);
	vsnprintf(call_log + len, sizeof(call_log) - len, format, args);
	va_end(args);
}

static const char *handle_name(const void *ptr)
{
	static char names[4][8];
	static size_t next;
	char *name = names[next++ % 4];
	const char *p = ptr;

	if (!p)
		return "-";
	if (p == &screen_target)
		return "S";
	if (p == &screen_zs)
		return "SZ";
	if (p >= textures && p < textures + sizeof(textures))
		snprintf(name, sizeof(names[0]), "T%d", (int)(p - textures));
	else if (p >= zstencils && p < zstencils + sizeof(zstencils))
		snprintf(name, sizeof(names[0]), "Z%d", (int)(p - zstencils));
	else
		return "?";
	return name;
}

static void fake_enter_context(gs_device_t *device)
{
	assert_ptr_equal(device, &device_token);
}

static void fake_leave_context(gs_device_t *device)
{
	assert_ptr_equal(device, &device_token);
}

static gs_texture_t *fake_texture_create(gs_device_t *device, uint32_t width, uint32_t height,
					 enum gs_color_format color_format, uint32_t levels, const uint8_t **data,
					 uint32_t flags)
{
	assert_ptr_equal(device, &device_token);
	record("tex_create %u %u f%d l%u %s 0x%x", width, height, (int)color_format, levels, data ? "data" : "null",
	       flags);
	if (fail_texture)
		return NULL;
	assert_true(next_texture + 1 < sizeof(textures));
	return (gs_texture_t *)&textures[++next_texture];
}

static gs_zstencil_t *fake_zstencil_create(gs_device_t *device, uint32_t width, uint32_t height,
					   enum gs_zstencil_format format)
{
	assert_ptr_equal(device, &device_token);
	record("zs_create %u %u f%d", width, height, (int)format);
	if (fail_zstencil)
		return NULL;
	assert_true(next_zstencil + 1 < sizeof(zstencils));
	return (gs_zstencil_t *)&zstencils[++next_zstencil];
}

static void fake_texture_destroy(gs_texture_t *tex)
{
	record("tex_destroy %s", handle_name(tex));
}

static void fake_zstencil_destroy(gs_zstencil_t *zs)
{
	record("zs_destroy %s", handle_name(zs));
}

static void fake_get_viewport(const gs_device_t *device, struct gs_rect *rect)
{
	assert_ptr_equal(device, &device_token);
	record("vp_get");
	*rect = device_viewport;
}

static void fake_set_viewport(gs_device_t *device, int x, int y, int width, int height)
{
	assert_ptr_equal(device, &device_token);
	record("vp_set %d %d %d %d", x, y, width, height);
	device_viewport.x = x;
	device_viewport.y = y;
	device_viewport.cx = width;
	device_viewport.cy = height;
}

static void fake_projection_push(gs_device_t *device)
{
	assert_ptr_equal(device, &device_token);
	record("proj_push");
	projection_depth++;
}

static void fake_projection_pop(gs_device_t *device)
{
	assert_ptr_equal(device, &device_token);
	record("proj_pop");
	projection_depth--;
}

static gs_texture_t *fake_get_render_target(const gs_device_t *device)
{
	assert_ptr_equal(device, &device_token);
	record("rt_get %s", handle_name(device_target));
	return device_target;
}

static gs_zstencil_t *fake_get_zstencil_target(const gs_device_t *device)
{
	assert_ptr_equal(device, &device_token);
	record("zs_get %s", handle_name(device_zs));
	return device_zs;
}

static enum gs_color_space fake_get_color_space(gs_device_t *device)
{
	assert_ptr_equal(device, &device_token);
	record("cs_get %d", (int)device_space);
	return device_space;
}

static void fake_set_render_target_with_color_space(gs_device_t *device, gs_texture_t *tex, gs_zstencil_t *zstencil,
						    enum gs_color_space space)
{
	assert_ptr_equal(device, &device_token);
	record("rt_set %s %s cs%d", handle_name(tex), handle_name(zstencil), (int)space);
	device_target = tex;
	device_zs = zstencil;
	device_space = space;
}

static graphics_t *graphics;

static int setup(void **state)
{
	struct matrix4 top;

	(void)state;

	graphics = bzalloc(sizeof(struct graphics_subsystem));
	matrix4_identity(&top);
	da_push_back(graphics->matrix_stack, &top);
	pthread_mutex_init(&graphics->mutex, NULL);
	pthread_mutex_init(&graphics->effect_mutex, NULL);

	graphics->device = (gs_device_t *)&device_token;
	graphics->exports.device_enter_context = fake_enter_context;
	graphics->exports.device_leave_context = fake_leave_context;
	graphics->exports.device_texture_create = fake_texture_create;
	graphics->exports.device_zstencil_create = fake_zstencil_create;
	graphics->exports.gs_texture_destroy = fake_texture_destroy;
	graphics->exports.gs_zstencil_destroy = fake_zstencil_destroy;
	graphics->exports.device_get_viewport = fake_get_viewport;
	graphics->exports.device_set_viewport = fake_set_viewport;
	graphics->exports.device_projection_push = fake_projection_push;
	graphics->exports.device_projection_pop = fake_projection_pop;
	graphics->exports.device_get_render_target = fake_get_render_target;
	graphics->exports.device_get_zstencil_target = fake_get_zstencil_target;
	graphics->exports.device_get_color_space = fake_get_color_space;
	graphics->exports.device_set_render_target_with_color_space = fake_set_render_target_with_color_space;

	next_texture = 0;
	next_zstencil = 0;
	fail_texture = false;
	fail_zstencil = false;
	device_target = (gs_texture_t *)&screen_target;
	device_zs = (gs_zstencil_t *)&screen_zs;
	device_space = GS_CS_709_EXTENDED;
	device_viewport.x = 1;
	device_viewport.y = 2;
	device_viewport.cx = 300;
	device_viewport.cy = 200;
	projection_depth = 0;
	call_log[0] = 0;

	gs_enter_context(graphics);
	return 0;
}

static int teardown(void **state)
{
	(void)state;

	gs_leave_context();
	pthread_mutex_destroy(&graphics->mutex);
	pthread_mutex_destroy(&graphics->effect_mutex);
	da_free(graphics->matrix_stack);
	da_free(graphics->viewport_stack);
	bfree(graphics);
	graphics = NULL;
	return 0;
}

static void clear_log(void)
{
	call_log[0] = 0;
}

static void set_scaled_matrix(void)
{
	struct matrix4 m;

	matrix4_identity(&m);
	m.x.x = 2.0f;
	m.t.y = 5.0f;
	gs_matrix_set(&m);
}

static bool matrix_is_identity(void)
{
	struct matrix4 m, identity;

	gs_matrix_get(&m);
	matrix4_identity(&identity);
	return memcmp(&m, &identity, sizeof(m)) == 0;
}

static bool matrix_is_scaled(void)
{
	struct matrix4 m;

	gs_matrix_get(&m);
	return m.x.x == 2.0f && m.t.y == 5.0f;
}

static void create_destroy_test(void **state)
{
	(void)state;

	long allocs = bnum_allocs();
	gs_texrender_t *tr = gs_texrender_create(GS_RGBA, GS_ZS_NONE);

	assert_non_null(tr);
	assert_int_equal(bnum_allocs(), allocs + 1);
	assert_null(gs_texrender_get_texture(tr));
	assert_int_equal(gs_texrender_get_format(tr), GS_RGBA);

	gs_texrender_destroy(tr);
	assert_int_equal(bnum_allocs(), allocs);
	assert_string_equal(call_log, "");

	tr = gs_texrender_create(GS_R10G10B10A2, GS_Z24_S8);
	assert_int_equal(gs_texrender_get_format(tr), GS_R10G10B10A2);
	assert_true(gs_texrender_begin(tr, 8, 4));
	gs_texrender_end(tr);
	/* begin grew the context's viewport stack: count from here. */
	allocs = bnum_allocs();
	clear_log();
	gs_texrender_destroy(tr);
	assert_string_equal(call_log, "tex_destroy T1;zs_destroy Z1");
	assert_int_equal(bnum_allocs(), allocs - 1);
}

static void null_test(void **state)
{
	(void)state;

	gs_texrender_destroy(NULL);
	assert_false(gs_texrender_begin(NULL, 64, 32));
	assert_false(gs_texrender_begin_with_color_space(NULL, 64, 32, GS_CS_709_SCRGB));
	gs_texrender_end(NULL);
	gs_texrender_reset(NULL);
	assert_null(gs_texrender_get_texture(NULL));

	assert_string_equal(call_log, "");
	assert_int_equal(graphics->cur_matrix, 0);
	assert_int_equal(graphics->viewport_stack.num, 0);
	assert_int_equal(projection_depth, 0);
}

static void begin_end_test(void **state)
{
	(void)state;

	gs_texrender_t *tr = gs_texrender_create(GS_RGBA, GS_ZS_NONE);

	set_scaled_matrix();
	assert_true(gs_texrender_begin(tr, 64, 32));
	assert_string_equal(call_log, "tex_create 64 32 f3 l1 null 0x4;"
				      "vp_get;proj_push;"
				      "rt_get S;zs_get SZ;cs_get 2;"
				      "rt_set T1 - cs0;"
				      "vp_set 0 0 64 32");
	assert_ptr_equal(gs_texrender_get_texture(tr), &textures[1]);
	assert_true(matrix_is_identity());
	assert_int_equal(graphics->cur_matrix, 1);
	assert_int_equal(graphics->viewport_stack.num, 1);
	assert_int_equal(projection_depth, 1);

	clear_log();
	gs_texrender_end(tr);
	assert_string_equal(call_log, "rt_set S SZ cs2;proj_pop;vp_set 1 2 300 200");
	assert_true(matrix_is_scaled());
	assert_int_equal(graphics->cur_matrix, 0);
	assert_int_equal(graphics->viewport_stack.num, 0);
	assert_int_equal(projection_depth, 0);
	assert_ptr_equal(gs_texrender_get_texture(tr), &textures[1]);

	gs_texrender_destroy(tr);
}

static void zstencil_test(void **state)
{
	(void)state;

	gs_texrender_t *tr = gs_texrender_create(GS_BGRA, GS_Z24_S8);

	assert_true(gs_texrender_begin(tr, 16, 8));
	assert_string_equal(call_log, "tex_create 16 8 f5 l1 null 0x4;zs_create 16 8 f2;"
				      "vp_get;proj_push;"
				      "rt_get S;zs_get SZ;cs_get 2;"
				      "rt_set T1 Z1 cs0;"
				      "vp_set 0 0 16 8");
	clear_log();
	gs_texrender_end(tr);
	assert_string_equal(call_log, "rt_set S SZ cs2;proj_pop;vp_set 1 2 300 200");
	gs_texrender_destroy(tr);
}

static void rendered_reset_test(void **state)
{
	(void)state;

	gs_texrender_t *tr = gs_texrender_create(GS_RGBA, GS_ZS_NONE);

	assert_true(gs_texrender_begin(tr, 64, 32));
	gs_texrender_end(tr);

	clear_log();
	assert_false(gs_texrender_begin(tr, 64, 32));
	assert_false(gs_texrender_begin(tr, 128, 64));
	assert_string_equal(call_log, "");

	gs_texrender_reset(tr);
	gs_texrender_reset(tr);
	assert_true(gs_texrender_begin(tr, 64, 32));
	assert_string_equal(call_log, "vp_get;proj_push;"
				      "rt_get S;zs_get SZ;cs_get 2;"
				      "rt_set T1 - cs0;"
				      "vp_set 0 0 64 32");
	gs_texrender_end(tr);
	gs_texrender_destroy(tr);
}

static void resize_test(void **state)
{
	(void)state;

	gs_texrender_t *tr = gs_texrender_create(GS_RGBA16F, GS_Z32F);

	assert_true(gs_texrender_begin(tr, 64, 32));
	gs_texrender_end(tr);
	gs_texrender_reset(tr);

	clear_log();
	assert_true(gs_texrender_begin(tr, 64, 33));
	assert_string_equal(call_log, "tex_destroy T1;zs_destroy Z1;"
				      "tex_create 64 33 f9 l1 null 0x4;zs_create 64 33 f3;"
				      "vp_get;proj_push;"
				      "rt_get S;zs_get SZ;cs_get 2;"
				      "rt_set T2 Z2 cs0;"
				      "vp_set 0 0 64 33");
	assert_ptr_equal(gs_texrender_get_texture(tr), &textures[2]);
	gs_texrender_end(tr);
	gs_texrender_reset(tr);

	clear_log();
	assert_true(gs_texrender_begin(tr, 65, 33));
	assert_string_equal(call_log, "tex_destroy T2;zs_destroy Z2;"
				      "tex_create 65 33 f9 l1 null 0x4;zs_create 65 33 f3;"
				      "vp_get;proj_push;"
				      "rt_get S;zs_get SZ;cs_get 2;"
				      "rt_set T3 Z3 cs0;"
				      "vp_set 0 0 65 33");
	gs_texrender_end(tr);
	gs_texrender_destroy(tr);
}

static void zero_size_test(void **state)
{
	(void)state;

	gs_texrender_t *tr = gs_texrender_create(GS_RGBA, GS_ZS_NONE);

	assert_false(gs_texrender_begin(tr, 0, 32));
	assert_false(gs_texrender_begin(tr, 32, 0));
	assert_false(gs_texrender_begin(tr, 0, 0));
	assert_string_equal(call_log, "");
	assert_null(gs_texrender_get_texture(tr));

	assert_true(gs_texrender_begin(tr, 32, 32));
	gs_texrender_end(tr);
	gs_texrender_reset(tr);
	clear_log();
	assert_false(gs_texrender_begin(tr, 0, 32));
	assert_string_equal(call_log, "");
	assert_ptr_equal(gs_texrender_get_texture(tr), &textures[1]);
	gs_texrender_destroy(tr);
}

static void texture_failure_test(void **state)
{
	(void)state;

	gs_texrender_t *tr = gs_texrender_create(GS_RGBA, GS_Z16);

	fail_texture = true;
	assert_false(gs_texrender_begin(tr, 64, 32));
	assert_string_equal(call_log, "tex_create 64 32 f3 l1 null 0x4");
	assert_null(gs_texrender_get_texture(tr));

	/* Characterized, not endorsed: the size is stored, so the same size
	 * does not retry. */
	fail_texture = false;
	clear_log();
	assert_false(gs_texrender_begin(tr, 64, 32));
	assert_string_equal(call_log, "");

	assert_true(gs_texrender_begin(tr, 64, 31));
	assert_string_equal(call_log, "tex_create 64 31 f3 l1 null 0x4;zs_create 64 31 f1;"
				      "vp_get;proj_push;"
				      "rt_get S;zs_get SZ;cs_get 2;"
				      "rt_set T1 Z1 cs0;"
				      "vp_set 0 0 64 31");
	gs_texrender_end(tr);
	assert_int_equal(graphics->cur_matrix, 0);
	assert_int_equal(graphics->viewport_stack.num, 0);
	gs_texrender_destroy(tr);
}

static void zstencil_failure_test(void **state)
{
	(void)state;

	gs_texrender_t *tr = gs_texrender_create(GS_RGBA, GS_Z24_S8);

	fail_zstencil = true;
	assert_false(gs_texrender_begin(tr, 64, 32));
	assert_string_equal(call_log, "tex_create 64 32 f3 l1 null 0x4;zs_create 64 32 f2;tex_destroy T1");
	assert_null(gs_texrender_get_texture(tr));
	assert_int_equal(graphics->cur_matrix, 0);
	assert_int_equal(graphics->viewport_stack.num, 0);
	assert_int_equal(projection_depth, 0);

	fail_zstencil = false;
	clear_log();
	assert_false(gs_texrender_begin(tr, 64, 32));
	assert_string_equal(call_log, "");

	clear_log();
	gs_texrender_destroy(tr);
	assert_string_equal(call_log, "");
}

static void color_space_test(void **state)
{
	(void)state;

	gs_texrender_t *tr = gs_texrender_create(GS_RGBA16F, GS_ZS_NONE);

	device_space = GS_CS_SRGB_16F;
	assert_true(gs_texrender_begin_with_color_space(tr, 10, 20, GS_CS_709_SCRGB));
	assert_string_equal(call_log, "tex_create 10 20 f9 l1 null 0x4;"
				      "vp_get;proj_push;"
				      "rt_get S;zs_get SZ;cs_get 1;"
				      "rt_set T1 - cs3;"
				      "vp_set 0 0 10 20");
	assert_int_equal(device_space, GS_CS_709_SCRGB);
	clear_log();
	gs_texrender_end(tr);
	assert_string_equal(call_log, "rt_set S SZ cs1;proj_pop;vp_set 1 2 300 200");
	assert_int_equal(device_space, GS_CS_SRGB_16F);
	assert_int_equal(gs_texrender_get_format(tr), GS_RGBA16F);

	gs_texrender_reset(tr);
	clear_log();
	assert_true(gs_texrender_begin(tr, 10, 20));
	assert_string_equal(call_log, "vp_get;proj_push;"
				      "rt_get S;zs_get SZ;cs_get 1;"
				      "rt_set T1 - cs0;"
				      "vp_set 0 0 10 20");
	gs_texrender_end(tr);
	gs_texrender_destroy(tr);
}

static void double_begin_test(void **state)
{
	(void)state;

	gs_texrender_t *tr = gs_texrender_create(GS_RGBA, GS_ZS_NONE);

	set_scaled_matrix();
	assert_true(gs_texrender_begin(tr, 64, 32));
	clear_log();
	assert_true(gs_texrender_begin(tr, 64, 32));
	assert_string_equal(call_log, "vp_get;proj_push;"
				      "rt_get T1;zs_get -;cs_get 0;"
				      "rt_set T1 - cs0;"
				      "vp_set 0 0 64 32");
	assert_int_equal(graphics->cur_matrix, 2);
	assert_int_equal(graphics->viewport_stack.num, 2);
	assert_int_equal(projection_depth, 2);

	/* Characterized, not endorsed: the second begin saved the texrender's
	 * own target, so neither end restores S. */
	clear_log();
	gs_texrender_end(tr);
	assert_string_equal(call_log, "rt_set T1 - cs0;proj_pop;vp_set 0 0 64 32");
	clear_log();
	gs_texrender_end(tr);
	assert_string_equal(call_log, "rt_set T1 - cs0;proj_pop;vp_set 1 2 300 200");
	assert_ptr_equal(device_target, &textures[1]);
	assert_true(matrix_is_scaled());
	assert_int_equal(graphics->cur_matrix, 0);
	assert_int_equal(graphics->viewport_stack.num, 0);
	assert_int_equal(projection_depth, 0);

	gs_texrender_destroy(tr);
}

static void end_without_begin_test(void **state)
{
	(void)state;

	gs_texrender_t *tr = gs_texrender_create(GS_RGBA, GS_ZS_NONE);

	/* Characterized, not endorsed: the zeroed previous state is restored,
	 * the empty stacks do not pop, and rendered is set. */
	gs_texrender_end(tr);
	assert_string_equal(call_log, "rt_set - - cs0;proj_pop");
	assert_int_equal(graphics->cur_matrix, 0);
	assert_int_equal(graphics->viewport_stack.num, 0);
	assert_int_equal(projection_depth, -1);

	clear_log();
	assert_false(gs_texrender_begin(tr, 64, 32));
	assert_string_equal(call_log, "");
	gs_texrender_reset(tr);
	assert_true(gs_texrender_begin(tr, 64, 32));
	gs_texrender_end(tr);

	/* A second end after a pair restores the same previous state again. */
	clear_log();
	gs_texrender_end(tr);
	assert_string_equal(call_log, "rt_set - - cs0;proj_pop");
	gs_texrender_destroy(tr);
}

static void viewport_cast_test(void **state)
{
	(void)state;

	gs_texrender_t *tr = gs_texrender_create(GS_A8, GS_ZS_NONE);

	assert_true(gs_texrender_begin(tr, 0x80000000u, 0xffffffffu));
	assert_string_equal(call_log, "tex_create 2147483648 4294967295 f1 l1 null 0x4;"
				      "vp_get;proj_push;"
				      "rt_get S;zs_get SZ;cs_get 2;"
				      "rt_set T1 - cs0;"
				      "vp_set 0 0 -2147483648 -1");
	gs_texrender_end(tr);
	gs_texrender_destroy(tr);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(create_destroy_test, setup, teardown),
		cmocka_unit_test_setup_teardown(null_test, setup, teardown),
		cmocka_unit_test_setup_teardown(begin_end_test, setup, teardown),
		cmocka_unit_test_setup_teardown(zstencil_test, setup, teardown),
		cmocka_unit_test_setup_teardown(rendered_reset_test, setup, teardown),
		cmocka_unit_test_setup_teardown(resize_test, setup, teardown),
		cmocka_unit_test_setup_teardown(zero_size_test, setup, teardown),
		cmocka_unit_test_setup_teardown(texture_failure_test, setup, teardown),
		cmocka_unit_test_setup_teardown(zstencil_failure_test, setup, teardown),
		cmocka_unit_test_setup_teardown(color_space_test, setup, teardown),
		cmocka_unit_test_setup_teardown(double_begin_test, setup, teardown),
		cmocka_unit_test_setup_teardown(end_without_begin_test, setup, teardown),
		cmocka_unit_test_setup_teardown(viewport_cast_test, setup, teardown),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
