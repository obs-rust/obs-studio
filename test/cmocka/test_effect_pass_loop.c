/*
 * Characterization of the technique and pass loop of libobs/graphics/
 * effect-passes.c (gs_effect_loop, the technique and pass begin/end
 * functions, gs_effect_update_params and the parameter upload) before its
 * Rust port.
 *
 * The test compiles the C file into itself with recording stubs for the
 * device functions it calls (gs_load_vertexshader, gs_load_pixelshader,
 * gs_shader_set_val, gs_shader_set_next_sampler, gs_shader_get_param_info,
 * gs_shader_set_texture) and for gs_get_effect, which reads the cur_effect
 * the loop writes through the effect's graphics subsystem, as the real one
 * does for the thread's. No graphics context. Linux only, as
 * test_audio_buffering_subsample: the link drops the unused functions of the
 * compiled C (--gc-sections).
 * The effects are built by hand; gs_effect_get_technique is the real
 * lookup in libobs.
 *
 * What the cases pin: the call sequence and the effect state of one- and
 * two-pass loops; a missing technique and an active effect (a warning, no
 * change); the upload of the value, of the default when there is no value,
 * of nothing when there is neither; changed_only on gs_effect_update_params;
 * the next sampler before the changed check; changed cleared after an
 * upload; textures set to NULL at the end of a pass; the reset at the end
 * of a technique; gs_technique_begin_pass_by_name.
 */

#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <setjmp.h>
#include <cmocka.h>

#include "libobs/graphics/effect-passes.c.h"
#include "graphics/graphics-internal.h"

/* ------------------------------------------------------------------------ */
/* The recording device. */

#define MAX_EVENTS 64
static char events[MAX_EVENTS][96];
static size_t num_events;
static struct graphics_subsystem *stub_graphics;

static void record(const char *fmt, ...)
{
	if (num_events >= MAX_EVENTS)
		return;
	va_list args;
	va_start(args, fmt);
	vsnprintf(events[num_events++], sizeof(events[0]), fmt, args);
	va_end(args);
}

/* Shaders and samplers are plain tags (small integers as pointers). */
#define TAG(p) ((unsigned)(uintptr_t)(p))

void gs_load_vertexshader(gs_shader_t *vertshader)
{
	record("load_vs %u", TAG(vertshader));
}

void gs_load_pixelshader(gs_shader_t *pixelshader)
{
	record("load_ps %u", TAG(pixelshader));
}

void gs_shader_set_val(gs_sparam_t *param, const void *val, size_t size)
{
	char hex[64] = {0};
	for (size_t i = 0; i < size && i < 24; i++)
		snprintf(hex + i * 2, 3, "%02x", ((const uint8_t *)val)[i]);
	record("set_val %u %zu %s", TAG(param), size, hex);
}

void gs_shader_set_next_sampler(gs_sparam_t *param, gs_samplerstate_t *sampler)
{
	record("next_sampler %u %u", TAG(param), TAG(sampler));
}

/* sparam tags >= 100 are texture parameters. */
void gs_shader_get_param_info(const gs_sparam_t *param, struct gs_shader_param_info *info)
{
	info->name = "p";
	info->type = TAG(param) >= 100 ? GS_SHADER_PARAM_TEXTURE : GS_SHADER_PARAM_FLOAT;
}

void gs_shader_set_texture(gs_sparam_t *param, gs_texture_t *val)
{
	record("set_texture %u %u", TAG(param), TAG(val));
}

gs_effect_t *gs_get_effect(void)
{
	return stub_graphics ? stub_graphics->cur_effect : NULL;
}

static bool has_event(const char *e)
{
	for (size_t i = 0; i < num_events; i++)
		if (strcmp(events[i], e) == 0)
			return true;
	return false;
}

static void assert_events(const char *const *expected, size_t n)
{
	for (size_t i = 0; i < n && i < num_events; i++)
		assert_string_equal(events[i], expected[i]);
	assert_int_equal(num_events, n);
}

#define ASSERT_EVENTS(...)                                                \
	do {                                                              \
		const char *const e_[] = {__VA_ARGS__};                   \
		assert_events(e_, sizeof(e_) / sizeof(e_[0]));            \
	} while (0)

/* ------------------------------------------------------------------------ */
/* Hand-built effects. */

struct fixture {
	struct gs_effect effect;
	struct gs_effect_param params[4];
};

static void set_bytes(struct gs_effect_param *p, const void *data, size_t size)
{
	da_resize(p->cur_val, size);
	memcpy(p->cur_val.array, data, size);
}

static void push_param(pass_shaderparam_array_t *arr, struct gs_effect_param *eparam, unsigned sparam_tag)
{
	struct pass_shaderparam sp = {eparam, (gs_sparam_t *)(uintptr_t)sparam_tag};
	da_push_back(*arr, &sp);
}

/* Technique "Draw" with `passes` passes. Pass i uses shaders 10+i (vertex)
 * and 20+i (pixel). The vertex shader of every pass takes param 0 (sparam
 * 1) and param 1 (sparam 2); the pixel shader takes param 2 (sparam 3) and
 * the texture param 3 (sparam 100). */
static void fixture_init(struct fixture *f, size_t passes)
{
	memset(f, 0, sizeof(*f));
	stub_graphics = bzalloc(sizeof(*stub_graphics));
	f->effect.graphics = stub_graphics;
	for (size_t i = 0; i < 4; i++)
		f->params[i].type = i == 3 ? GS_SHADER_PARAM_TEXTURE : GS_SHADER_PARAM_FLOAT;
	da_push_back_array(f->effect.params, f->params, 4);
	struct gs_effect_param *p = f->effect.params.array;

	struct gs_effect_technique tech = {0};
	tech.name = bstrdup("Draw");
	tech.effect = &f->effect;
	for (size_t i = 0; i < passes; i++) {
		struct gs_effect_pass pass = {0};
		pass.name = bstrdup(i == 0 ? "First" : "Second");
		pass.vertshader = (gs_shader_t *)(uintptr_t)(10 + i);
		pass.pixelshader = (gs_shader_t *)(uintptr_t)(20 + i);
		push_param(&pass.vertshader_params, &p[0], 1);
		push_param(&pass.vertshader_params, &p[1], 2);
		push_param(&pass.pixelshader_params, &p[2], 3);
		push_param(&pass.pixelshader_params, &p[3], 100);
		da_push_back(tech.passes, &pass);
	}
	da_push_back(f->effect.techniques, &tech);
	num_events = 0;
}

static struct gs_effect_param *param(struct fixture *f, size_t i)
{
	return f->effect.params.array + i;
}

static void fixture_free(struct fixture *f)
{
	for (size_t i = 0; i < f->effect.techniques.num; i++) {
		struct gs_effect_technique *t = f->effect.techniques.array + i;
		for (size_t j = 0; j < t->passes.num; j++) {
			bfree(t->passes.array[j].name);
			da_free(t->passes.array[j].vertshader_params);
			da_free(t->passes.array[j].pixelshader_params);
		}
		da_free(t->passes);
		bfree(t->name);
	}
	da_free(f->effect.techniques);
	for (size_t i = 0; i < f->effect.params.num; i++) {
		da_free(f->effect.params.array[i].cur_val);
		da_free(f->effect.params.array[i].default_val);
	}
	da_free(f->effect.params);
	bfree(stub_graphics);
	stub_graphics = NULL;
}

/* ------------------------------------------------------------------------ */

static void one_pass_loop(void **state)
{
	(void)state;
	struct fixture f;
	fixture_init(&f, 1);
	float a = 1.0f, c = 2.0f;
	set_bytes(param(&f, 0), &a, sizeof(a));
	set_bytes(param(&f, 2), &c, sizeof(c));
	param(&f, 0)->changed = true;

	assert_true(gs_effect_loop(&f.effect, "Draw"));
	assert_true(f.effect.looping);
	assert_int_equal(f.effect.loop_pass, 1);
	assert_ptr_equal(f.effect.cur_technique, f.effect.techniques.array);
	assert_ptr_equal(f.effect.cur_pass, f.effect.techniques.array[0].passes.array);
	assert_ptr_equal(stub_graphics->cur_effect, &f.effect);
	/* Shaders, then the full upload: param 1 has neither value nor
	 * default and is skipped; changed is cleared afterwards. */
	ASSERT_EVENTS("load_vs 10", "load_ps 20", "set_val 1 4 0000803f", "set_val 3 4 00000040");
	assert_false(param(&f, 0)->changed);

	/* Second call: end the pass (texture set to NULL), no next pass: the
	 * technique ends. */
	num_events = 0;
	assert_false(gs_effect_loop(&f.effect, "Draw"));
	ASSERT_EVENTS("set_texture 100 0", "load_vs 0", "load_ps 0");
	assert_false(f.effect.looping);
	assert_int_equal(f.effect.loop_pass, 0);
	assert_null(f.effect.cur_technique);
	assert_null(f.effect.cur_pass);
	assert_null(stub_graphics->cur_effect);
	/* Every value emptied. */
	for (size_t i = 0; i < 4; i++)
		assert_int_equal(param(&f, i)->cur_val.num, 0);
	fixture_free(&f);
}

static void two_pass_loop(void **state)
{
	(void)state;
	struct fixture f;
	fixture_init(&f, 2);
	float a = 1.0f;
	set_bytes(param(&f, 0), &a, sizeof(a));

	assert_true(gs_effect_loop(&f.effect, "Draw"));
	num_events = 0;
	assert_true(gs_effect_loop(&f.effect, "Draw"));
	assert_int_equal(f.effect.loop_pass, 2);
	assert_ptr_equal(f.effect.cur_pass, f.effect.techniques.array[0].passes.array + 1);
	ASSERT_EVENTS("set_texture 100 0", "load_vs 11", "load_ps 21", "set_val 1 4 0000803f");
	num_events = 0;
	assert_false(gs_effect_loop(&f.effect, "Draw"));
	assert_false(f.effect.looping);
	assert_int_equal(f.effect.loop_pass, 0);
	fixture_free(&f);
}

static void missing_technique_and_active_effect(void **state)
{
	(void)state;
	struct fixture f;
	fixture_init(&f, 1);

	assert_false(gs_effect_loop(&f.effect, "Nope"));
	assert_false(f.effect.looping);
	assert_null(f.effect.cur_technique);
	assert_int_equal(num_events, 0);

	/* Another effect active: refused. */
	struct gs_effect other = {0};
	stub_graphics->cur_effect = &other;
	assert_false(gs_effect_loop(&f.effect, "Draw"));
	assert_false(f.effect.looping);
	assert_int_equal(num_events, 0);
	stub_graphics->cur_effect = NULL;

	assert_false(gs_effect_loop(NULL, "Draw"));
	fixture_free(&f);
}

static void default_upload_and_next_sampler(void **state)
{
	(void)state;
	struct fixture f;
	fixture_init(&f, 1);
	/* param 1: no value, a default: the default is copied in and sent. */
	float d = 3.0f;
	da_push_back_array(param(&f, 1)->default_val, (uint8_t *)&d, sizeof(d));
	/* param 3: a texture with a next sampler and a value. */
	gs_texture_t *tex = (gs_texture_t *)(uintptr_t)7;
	struct gs_shader_texture st = {0};
	st.tex = tex;
	set_bytes(param(&f, 3), &st, sizeof(st));
	param(&f, 3)->next_sampler = (gs_samplerstate_t *)(uintptr_t)55;

	assert_true(gs_effect_loop(&f.effect, "Draw"));
	assert_true(has_event("set_val 2 4 00004040"));
	assert_int_equal(param(&f, 1)->cur_val.num, sizeof(d));
	assert_true(has_event("next_sampler 100 55"));
	/* The sampler stays until the technique ends. */
	assert_ptr_equal(param(&f, 3)->next_sampler, (gs_samplerstate_t *)(uintptr_t)55);
	num_events = 0;
	assert_false(gs_effect_loop(&f.effect, "Draw"));
	assert_null(param(&f, 3)->next_sampler);
	fixture_free(&f);
}

static void update_params_sends_only_changed(void **state)
{
	(void)state;
	struct fixture f;
	fixture_init(&f, 1);
	float a = 1.0f, c = 2.0f;
	set_bytes(param(&f, 0), &a, sizeof(a));
	set_bytes(param(&f, 2), &c, sizeof(c));
	param(&f, 3)->next_sampler = (gs_samplerstate_t *)(uintptr_t)56;
	assert_true(gs_effect_loop(&f.effect, "Draw"));

	/* Nothing changed: only the next sampler (set before the changed
	 * check). */
	num_events = 0;
	gs_effect_update_params(&f.effect);
	ASSERT_EVENTS("next_sampler 100 56");

	/* param 2 changed: sent, then cleared. */
	num_events = 0;
	c = 4.0f;
	set_bytes(param(&f, 2), &c, sizeof(c));
	param(&f, 2)->changed = true;
	gs_effect_update_params(&f.effect);
	ASSERT_EVENTS("set_val 3 4 00008040", "next_sampler 100 56");
	assert_false(param(&f, 2)->changed);

	/* No current pass: nothing. */
	gs_technique_end_pass(f.effect.cur_technique);
	num_events = 0;
	param(&f, 2)->changed = true;
	gs_effect_update_params(&f.effect);
	assert_int_equal(num_events, 0);
	assert_true(param(&f, 2)->changed);
	gs_effect_update_params(NULL);
	gs_technique_end(f.effect.cur_technique);
	fixture_free(&f);
}

static void begin_pass_by_name_and_bounds(void **state)
{
	(void)state;
	struct fixture f;
	fixture_init(&f, 2);
	struct gs_effect_technique *tech = f.effect.techniques.array;

	assert_int_equal(gs_technique_begin(tech), 2);
	assert_ptr_equal(stub_graphics->cur_effect, &f.effect);
	assert_true(gs_technique_begin_pass_by_name(tech, "Second"));
	assert_ptr_equal(f.effect.cur_pass, tech->passes.array + 1);
	assert_false(gs_technique_begin_pass_by_name(tech, "Third"));
	assert_false(gs_technique_begin_pass(tech, 2));
	assert_ptr_equal(f.effect.cur_pass, tech->passes.array + 1);
	gs_technique_end_pass(tech);
	assert_null(f.effect.cur_pass);
	/* Ending a pass with none current: nothing. */
	num_events = 0;
	gs_technique_end_pass(tech);
	assert_int_equal(num_events, 0);
	gs_technique_end(tech);

	/* NULL techniques. */
	assert_int_equal(gs_technique_begin(NULL), 0);
	assert_false(gs_technique_begin_pass(NULL, 0));
	assert_false(gs_technique_begin_pass_by_name(NULL, "First"));
	gs_technique_end(NULL);
	gs_technique_end_pass(NULL);
	fixture_free(&f);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(one_pass_loop),
		cmocka_unit_test(two_pass_loop),
		cmocka_unit_test(missing_technique_and_active_effect),
		cmocka_unit_test(default_upload_and_next_sampler),
		cmocka_unit_test(update_params_sends_only_changed),
		cmocka_unit_test(begin_pass_by_name_and_bounds),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
