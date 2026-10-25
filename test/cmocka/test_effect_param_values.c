/*
 * Characterization of the parameter values of libobs/graphics/effect.c
 * (gs_effect_set_*, gs_effect_get_*_val*, gs_effect_set_default,
 * gs_effect_set_next_sampler) before their Rust port (#2409).
 *
 * The functions are EXPORT, so the test calls them through the real libobs
 * on a hand-built struct gs_effect_param: none of them touches the graphics
 * subsystem, so there is no graphics context and the test runs on every
 * platform. After the port the same calls run the Rust shims.
 *
 * What the cases pin:
 * - each setter's stored size and bytes: bool and int as an int (4 bytes),
 *   vec3 as 12 bytes (sizeof(float) * 3, not sizeof(struct vec3)), vec2,
 *   vec4 and matrix4 as the whole struct, a texture as struct
 *   gs_shader_texture with srgb false or true, a color through
 *   vec4_from_bgra (0xAARRGGBB to {r, g, b, a} / 255);
 * - changed: set when the bytes differ or the size changes, not when the
 *   same bytes are set again;
 * - the getters: NULL for a size of 0, a bmem copy of the stored bytes
 *   otherwise, at most the stored size;
 * - gs_effect_set_default: the default bytes back; without a default it
 *   passes the NULL array on, logs "invalid data" and keeps the value;
 * - gs_effect_set_next_sampler: only on a texture parameter, silently
 *   ignored otherwise;
 * - NULL parameter or data: one LOG_ERROR line each, nothing changed.
 */

#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <setjmp.h>
#include <cmocka.h>

#include "graphics/effect.h"
#include "graphics/matrix4.h"
#include "graphics/vec2.h"
#include "graphics/vec3.h"
#include "graphics/vec4.h"
#include "util/base.h"
#include "util/bmem.h"

/* The last LOG_ERROR line and how many there were; everything else goes to
 * the previous handler. */
static log_handler_t prev_handler;
static void *prev_param;
static int errors;
static char last_error[256];

static void log_handler(int lvl, const char *format, va_list args, void *param)
{
	(void)param;
	if (lvl == LOG_ERROR) {
		va_list copy;
		va_copy(copy, args);
		vsnprintf(last_error, sizeof(last_error), format, copy);
		va_end(copy);
		errors++;
		return;
	}
	if (prev_handler)
		prev_handler(lvl, format, args, prev_param);
}

static int setup(void **state)
{
	(void)state;
	base_get_log_handler(&prev_handler, &prev_param);
	base_set_log_handler(log_handler, NULL);
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	base_set_log_handler(prev_handler, prev_param);
	return 0;
}

static void reset_errors(void)
{
	errors = 0;
	last_error[0] = '\0';
}

static void param_init(struct gs_effect_param *param, enum gs_shader_param_type type)
{
	memset(param, 0, sizeof(*param));
	param->type = type;
}

static void param_free(struct gs_effect_param *param)
{
	da_free(param->cur_val);
	da_free(param->default_val);
}

static void assert_cur(struct gs_effect_param *param, const void *bytes, size_t size)
{
	assert_int_equal(param->cur_val.num, size);
	if (size)
		assert_memory_equal(param->cur_val.array, bytes, size);
}

static void float_changes_only_on_new_bytes(void **state)
{
	(void)state;
	struct gs_effect_param p;
	param_init(&p, GS_SHADER_PARAM_FLOAT);
	reset_errors();

	float v = 1.5f;
	gs_effect_set_float(&p, v);
	assert_cur(&p, &v, sizeof(v));
	assert_true(p.changed);

	/* The same bytes again: not changed. */
	p.changed = false;
	gs_effect_set_float(&p, v);
	assert_false(p.changed);

	/* Different bytes: changed. */
	v = 2.0f;
	gs_effect_set_float(&p, v);
	assert_cur(&p, &v, sizeof(v));
	assert_true(p.changed);
	assert_int_equal(errors, 0);
	param_free(&p);
}

static void size_change_always_marks_changed(void **state)
{
	(void)state;
	struct gs_effect_param p;
	param_init(&p, GS_SHADER_PARAM_VEC2);

	struct vec2 v2;
	vec2_set(&v2, 1.0f, 2.0f);
	gs_effect_set_vec2(&p, &v2);
	assert_cur(&p, &v2, sizeof(v2));

	/* The first 4 bytes are the same, the size is not: changed. */
	p.changed = false;
	gs_effect_set_float(&p, 1.0f);
	float one = 1.0f;
	assert_cur(&p, &one, sizeof(one));
	assert_true(p.changed);

	/* A size of 0: changed, nothing stored. */
	p.changed = false;
	gs_effect_set_val(&p, &one, 0);
	assert_int_equal(p.cur_val.num, 0);
	assert_true(p.changed);
	param_free(&p);
}

static void setter_sizes_and_bytes(void **state)
{
	(void)state;
	struct gs_effect_param p;
	param_init(&p, GS_SHADER_PARAM_INT);

	int one = 1, zero = 0, n = -7;
	gs_effect_set_bool(&p, true);
	assert_cur(&p, &one, sizeof(int));
	gs_effect_set_bool(&p, false);
	assert_cur(&p, &zero, sizeof(int));
	gs_effect_set_int(&p, n);
	assert_cur(&p, &n, sizeof(int));

	struct vec3 v3;
	vec3_set(&v3, 1.0f, 2.0f, 3.0f);
	gs_effect_set_vec3(&p, &v3);
	assert_int_equal(p.cur_val.num, sizeof(float) * 3);
	assert_memory_equal(p.cur_val.array, v3.ptr, sizeof(float) * 3);

	struct vec4 v4;
	vec4_set(&v4, 1.0f, 2.0f, 3.0f, 4.0f);
	gs_effect_set_vec4(&p, &v4);
	assert_cur(&p, &v4, sizeof(v4));

	struct matrix4 m;
	matrix4_identity(&m);
	m.t.x = 5.0f;
	gs_effect_set_matrix4(&p, &m);
	assert_cur(&p, &m, sizeof(m));

	uint8_t raw[3] = {1, 2, 3};
	gs_effect_set_val(&p, raw, sizeof(raw));
	assert_cur(&p, raw, sizeof(raw));
	param_free(&p);
}

static void texture_and_color(void **state)
{
	(void)state;
	struct gs_effect_param p;
	param_init(&p, GS_SHADER_PARAM_TEXTURE);

	gs_texture_t *tex = (gs_texture_t *)(uintptr_t)0x1234;
	struct gs_shader_texture st = {0};
	st.tex = tex;
	st.srgb = false;
	gs_effect_set_texture(&p, tex);
	assert_int_equal(p.cur_val.num, sizeof(st));
	assert_ptr_equal(((struct gs_shader_texture *)p.cur_val.array)->tex, tex);
	assert_false(((struct gs_shader_texture *)p.cur_val.array)->srgb);

	gs_effect_set_texture_srgb(&p, tex);
	assert_true(((struct gs_shader_texture *)p.cur_val.array)->srgb);

	/* 0xAARRGGBB: 0x80 alpha, 0xFF red, 0x40 green, 0x20 blue. */
	gs_effect_set_color(&p, 0x80FF4020);
	assert_int_equal(p.cur_val.num, sizeof(struct vec4));
	const float *f = (const float *)p.cur_val.array;
	assert_true(f[0] == 255 / 255.0f);
	assert_true(f[1] == 0x40 / 255.0f);
	assert_true(f[2] == 0x20 / 255.0f);
	assert_true(f[3] == 0x80 / 255.0f);
	param_free(&p);
}

static void getters_copy_the_stored_bytes(void **state)
{
	(void)state;
	struct gs_effect_param p;
	param_init(&p, GS_SHADER_PARAM_VEC4);
	reset_errors();

	/* Nothing stored: NULL and 0, no error. */
	assert_null(gs_effect_get_val(&p));
	assert_int_equal(gs_effect_get_val_size(&p), 0);
	assert_null(gs_effect_get_default_val(&p));
	assert_int_equal(gs_effect_get_default_val_size(&p), 0);
	assert_int_equal(errors, 0);

	struct vec4 v4, d4;
	vec4_set(&v4, 1.0f, 2.0f, 3.0f, 4.0f);
	vec4_set(&d4, 5.0f, 6.0f, 7.0f, 8.0f);
	gs_effect_set_vec4(&p, &v4);
	da_push_back_array(p.default_val, (uint8_t *)&d4, sizeof(d4));

	void *val = gs_effect_get_val(&p);
	assert_non_null(val);
	assert_int_equal(gs_effect_get_val_size(&p), sizeof(v4));
	assert_memory_equal(val, &v4, sizeof(v4));
	bfree(val);

	void *def = gs_effect_get_default_val(&p);
	assert_non_null(def);
	assert_int_equal(gs_effect_get_default_val_size(&p), sizeof(d4));
	assert_memory_equal(def, &d4, sizeof(d4));
	bfree(def);
	param_free(&p);
}

static void set_default_puts_the_default_back(void **state)
{
	(void)state;
	struct gs_effect_param p;
	param_init(&p, GS_SHADER_PARAM_VEC4);
	reset_errors();

	struct vec4 v4, d4;
	vec4_set(&v4, 1.0f, 2.0f, 3.0f, 4.0f);
	vec4_set(&d4, 5.0f, 6.0f, 7.0f, 8.0f);
	da_push_back_array(p.default_val, (uint8_t *)&d4, sizeof(d4));
	gs_effect_set_vec4(&p, &v4);
	p.changed = false;
	gs_effect_set_default(&p);
	assert_cur(&p, &d4, sizeof(d4));
	assert_true(p.changed);
	assert_int_equal(errors, 0);
	param_free(&p);

	/* No default: the NULL array goes on, "invalid data", value kept. */
	param_init(&p, GS_SHADER_PARAM_VEC4);
	gs_effect_set_vec4(&p, &v4);
	p.changed = false;
	reset_errors();
	gs_effect_set_default(&p);
	assert_int_equal(errors, 1);
	assert_string_equal(last_error, "effect_setval_inline: invalid data");
	assert_cur(&p, &v4, sizeof(v4));
	assert_false(p.changed);
	param_free(&p);
}

static void next_sampler_only_on_textures(void **state)
{
	(void)state;
	struct gs_effect_param p;
	gs_samplerstate_t *sampler = (gs_samplerstate_t *)(uintptr_t)0x5678;
	reset_errors();

	param_init(&p, GS_SHADER_PARAM_TEXTURE);
	gs_effect_set_next_sampler(&p, sampler);
	assert_ptr_equal(p.next_sampler, sampler);

	param_init(&p, GS_SHADER_PARAM_FLOAT);
	gs_effect_set_next_sampler(&p, sampler);
	assert_null(p.next_sampler);
	assert_int_equal(errors, 0);
}

static void null_param_and_data_log_once(void **state)
{
	(void)state;
	struct gs_effect_param p;
	param_init(&p, GS_SHADER_PARAM_VEC4);
	struct vec4 v4;
	vec4_set(&v4, 1.0f, 2.0f, 3.0f, 4.0f);
	gs_effect_set_vec4(&p, &v4);
	p.changed = false;

	reset_errors();
	gs_effect_set_float(NULL, 1.0f);
	assert_int_equal(errors, 1);
	assert_string_equal(last_error, "effect_setval_inline: invalid param");

	reset_errors();
	gs_effect_set_default(NULL);
	assert_int_equal(errors, 1);
	assert_string_equal(last_error, "effect_setval_inline: invalid param");

	reset_errors();
	gs_effect_set_val(&p, NULL, 16);
	assert_int_equal(errors, 1);
	assert_string_equal(last_error, "effect_setval_inline: invalid data");

	reset_errors();
	gs_effect_set_vec4(&p, NULL);
	assert_int_equal(errors, 1);
	assert_string_equal(last_error, "effect_setval_inline: invalid data");

	reset_errors();
	assert_null(gs_effect_get_val(NULL));
	assert_int_equal(errors, 1);
	assert_string_equal(last_error, "gs_effect_get_val: invalid param");

	reset_errors();
	assert_null(gs_effect_get_default_val(NULL));
	assert_int_equal(errors, 1);
	assert_string_equal(last_error, "gs_effect_get_default_val: invalid param");

	reset_errors();
	gs_effect_set_next_sampler(NULL, NULL);
	assert_int_equal(errors, 1);
	assert_string_equal(last_error, "gs_effect_set_next_sampler: invalid param");

	/* The sizes: 0 without an error. */
	reset_errors();
	assert_int_equal(gs_effect_get_val_size(NULL), 0);
	assert_int_equal(gs_effect_get_default_val_size(NULL), 0);
	assert_int_equal(errors, 0);

	/* Nothing changed by the failed calls. */
	assert_cur(&p, &v4, sizeof(v4));
	assert_false(p.changed);
	param_free(&p);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(float_changes_only_on_new_bytes), cmocka_unit_test(size_change_always_marks_changed),
		cmocka_unit_test(setter_sizes_and_bytes),          cmocka_unit_test(texture_and_color),
		cmocka_unit_test(getters_copy_the_stored_bytes),   cmocka_unit_test(set_default_puts_the_default_back),
		cmocka_unit_test(next_sampler_only_on_textures),   cmocka_unit_test(null_param_and_data_log_once),
	};
	return cmocka_run_group_tests(tests, setup, teardown);
}
