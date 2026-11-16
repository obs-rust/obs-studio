#include <setjmp.h>
#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>
#include <cmocka.h>

/* C-source characterization, not a GPU image-quality test. Opaque graphics
 * handles are identity tokens; only the mix field read by the real helper is
 * modeled. Every graphics dependency records its call and checks arguments.
 * The generated include contains the actual production bodies, not copies. */
typedef struct texture {
	int token;
} gs_texture_t;
typedef struct effect {
	int token;
} gs_effect_t;
typedef struct parameter {
	int token;
} gs_eparam_t;
typedef struct technique {
	int token;
} gs_technique_t;
enum { OBS_EFFECT_DEFAULT, GS_TRIS };
struct mix {
	gs_texture_t *render_texture;
};
static struct {
	struct {
		struct {
			struct mix **array;
		} mixes;
	} video;
} core, *obs = &core;
static gs_texture_t textures[2];
static gs_effect_t effect_token;
static gs_eparam_t parameter_token;
static gs_technique_t technique_token;
static gs_texture_t *expected_texture;
static const char *expected_technique;
static uint32_t width, height;
static size_t passes, loop_index, pass_index;
static char calls[512];
static size_t call_count;
static void record(char call)
{
	assert_true(call_count + 1 < sizeof(calls));
	calls[call_count++] = call;
	calls[call_count] = 0;
}
static gs_effect_t *obs_get_base_effect(int kind)
{
	assert_int_equal(kind, OBS_EFFECT_DEFAULT);
	record('E');
	return &effect_token;
}
static gs_eparam_t *gs_effect_get_param_by_name(gs_effect_t *effect, const char *name)
{
	assert_ptr_equal(effect, &effect_token);
	assert_string_equal(name, "image");
	record('P');
	return &parameter_token;
}
static void gs_effect_set_texture_srgb(gs_eparam_t *parameter, gs_texture_t *texture)
{
	assert_ptr_equal(parameter, &parameter_token);
	assert_ptr_equal(texture, expected_texture);
	record('I');
}
static void gs_blend_state_push(void)
{
	record('[');
}
static void gs_enable_blending(bool enabled)
{
	assert_false(enabled);
	record('B');
}
static void gs_enable_framebuffer_srgb(bool enabled)
{
	record(enabled ? '+' : '-');
}
static bool gs_effect_loop(gs_effect_t *effect, const char *name)
{
	assert_ptr_equal(effect, &effect_token);
	assert_string_equal(name, "Draw");
	record('L');
	return loop_index++ < passes;
}
static void gs_draw_sprite(gs_texture_t *texture, uint32_t flags, uint32_t w, uint32_t h)
{
	assert_ptr_equal(texture, expected_texture);
	assert_int_equal(flags, 0);
	assert_int_equal(w, 0);
	assert_int_equal(h, 0);
	record('S');
}
static void gs_blend_state_pop(void)
{
	record(']');
}
static gs_technique_t *gs_effect_get_technique(gs_effect_t *effect, const char *name)
{
	assert_ptr_equal(effect, &effect_token);
	assert_ptr_equal(name, expected_technique);
	record('T');
	return &technique_token;
}
static uint32_t gs_texture_get_width(gs_texture_t *texture)
{
	assert_ptr_equal(texture, expected_texture);
	record('W');
	return width;
}
static uint32_t gs_texture_get_height(gs_texture_t *texture)
{
	assert_ptr_equal(texture, expected_texture);
	record('H');
	return height;
}
static void gs_set_render_target(gs_texture_t *texture, void *depth)
{
	assert_ptr_equal(texture, expected_texture);
	assert_null(depth);
	record('R');
}
/* Keep the existing helper as a dependency, not part of this extraction. */
static inline void set_render_size(uint32_t w, uint32_t h)
{
	assert_int_equal(w, width);
	assert_int_equal(h, height);
	record('Z');
}
static size_t gs_technique_begin(gs_technique_t *technique)
{
	assert_ptr_equal(technique, &technique_token);
	record('A');
	return passes;
}
static bool gs_technique_begin_pass(gs_technique_t *technique, size_t index)
{
	assert_ptr_equal(technique, &technique_token);
	assert_int_equal(index, pass_index++);
	record('b');
	/* Characterized, not endorsed: the C helper ignores this result. */
	return false;
}
static void gs_draw(int mode, uint32_t start, uint32_t count)
{
	assert_int_equal(mode, GS_TRIS);
	assert_int_equal(start, 0);
	assert_int_equal(count, 3);
	record('D');
}
static void gs_technique_end_pass(gs_technique_t *technique)
{
	assert_ptr_equal(technique, &technique_token);
	record('e');
}
static void gs_technique_end(gs_technique_t *technique)
{
	assert_ptr_equal(technique, &technique_token);
	record('F');
}

#include "video-render-passes-under-test.h"

static void mix_passes(void **state)
{
	(void)state;
	struct mix mix0 = {&textures[0]}, mix1 = {&textures[1]};
	struct mix *mixes[] = {&mix0, &mix1};
	obs->video.mixes.array = mixes;
	for (size_t index = 0; index < 2; index++) {
		for (size_t count = 0; count < 3; count++) {
			expected_texture = &textures[index];
			passes = count;
			loop_index = call_count = 0;
			draw_mix_texture(index);
			const char *expected[] = {"EPI[B+L-]", "EPI[B+LSL-]", "EPI[B+LSLSL-]"};
			assert_string_equal(calls, expected[count]);
			assert_int_equal(loop_index, count + 1);
		}
	}
	obs->video.mixes.array = NULL;
}
static void conversion_passes(void **state)
{
	(void)state;
	const uint32_t sizes[][2] = {{0, 0}, {1920, 1080}, {UINT32_MAX, 1}};
	const char *names[] = {"Planar_Y", "Planar_U", "Planar_V"};
	for (size_t plane = 0; plane < 3; plane++) {
		for (size_t count = 0; count < 3; count++) {
			width = sizes[plane][0];
			height = sizes[plane][1];
			expected_texture = &textures[plane % 2];
			expected_technique = names[plane];
			passes = count;
			pass_index = call_count = 0;
			render_convert_plane(&effect_token, expected_texture, expected_technique);
			const char *expected[] = {"TWHRZAF", "TWHRZAbDeF", "TWHRZAbDebDeF"};
			assert_string_equal(calls, expected[count]);
			assert_int_equal(pass_index, count);
		}
	}
}
int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(mix_passes), cmocka_unit_test(conversion_passes)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
