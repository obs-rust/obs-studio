/* Original extracted canvas texture draw entry points
 * (libobs/obs-render-canvas-texture.c.h) over a real-header core, canvases
 * and mixes, with the real field adapters (obs-render-canvas-texture-fields.c.h)
 * under their own names, and one recorder for every libobs and graphics call
 * the draw makes. The decisions are the original obs-canvas-draw.c.h
 * (oracle/canvas_draw.c's oracle_obs_canvas_draw_*).
 *
 * The recorder logs each call with its arguments (pointers as fixture ids,
 * floats as bits) and, when asked, changes every mix's render_space and
 * render_texture on each call, so a field read moved past a call shows in
 * the log. Excluded: a NULL canvas or main canvas (the C reads canvas->mix
 * through NULL and crashes; the Rust draws nothing, the one intentional
 * difference) and an uninitialized core. The real device and the real
 * canvases stay unchanged Tier 2 (test_render_canvas_texture). */
#include "libobs/build/libobs.pch.h"
#include <stdarg.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define RCT_LOG 4096
#define RCT_TOKENS 12
#define RCT_EFFECT 8
#define RCT_IMAGE 9
#define RCT_MULTIPLIER 10

static struct obs_core rct_core;
static struct obs_canvas rct_canvases[3];
static struct obs_core_video_mix rct_mixes[2];
static unsigned char rct_tokens[RCT_TOKENS];
static char rct_log[RCT_LOG];
static size_t rct_len;
static int rct_color_space;
static float rct_white;
static bool rct_srgb;
static int rct_passes;
static int rct_loops;
static bool rct_null_effect;
static bool rct_null_params;
static bool rct_clobber_on;
static int rct_clobbers;

#ifdef __GNUC__
__attribute__((format(printf, 1, 2)))
#endif
static void rct_record(const char *format, ...)
{
	va_list args;
	va_start(args, format);
	int n = vsnprintf(rct_log + rct_len, RCT_LOG - rct_len, format, args);
	va_end(args);
	if (n < 0 || (size_t)n >= RCT_LOG - rct_len)
		abort();
	rct_len += (size_t)n;
}

/* A token's index, -1 for NULL, 99 for anything else. */
static int rct_id(const void *ptr)
{
	if (!ptr)
		return -1;
	for (int i = 0; i < RCT_TOKENS; ++i)
		if (ptr == (const void *)&rct_tokens[i])
			return i;
	return 99;
}

static int rct_object_id(const void *ptr)
{
	for (int i = 0; i < 3; ++i)
		if (ptr == (const void *)&rct_canvases[i])
			return 10 + i;
	for (int i = 0; i < 2; ++i)
		if (ptr == (const void *)&rct_mixes[i])
			return 20 + i;
	return rct_id(ptr);
}

static unsigned rct_bits(float value)
{
	uint32_t bits;
	memcpy(&bits, &value, sizeof(bits));
	return (unsigned)bits;
}

/* After each call: every mix gets another space and texture. */
static void rct_clobber(void)
{
	if (!rct_clobber_on)
		return;
	rct_clobbers++;
	for (int i = 0; i < 2; ++i) {
		rct_mixes[i].render_space = (enum gs_color_space)((rct_clobbers + i) % 4);
		rct_mixes[i].render_texture = (gs_texture_t *)&rct_tokens[(rct_clobbers + 3 * i) % 8];
	}
}

enum gs_color_space oracle_rct_color_space(void);
enum gs_color_space oracle_rct_color_space(void)
{
	rct_record("color_space;");
	rct_clobber();
	return (enum gs_color_space)rct_color_space;
}

float oracle_rct_white_level(void);
float oracle_rct_white_level(void)
{
	rct_record("white_level;");
	rct_clobber();
	return rct_white;
}

bool oracle_rct_srgb_enabled(void);
bool oracle_rct_srgb_enabled(void)
{
	rct_record("srgb_enabled;");
	rct_clobber();
	return rct_srgb;
}

void oracle_rct_enable_srgb(bool enable);
void oracle_rct_enable_srgb(bool enable)
{
	rct_record("enable_srgb %d;", enable ? 1 : 0);
	rct_clobber();
}

gs_effect_t *oracle_rct_base_effect(enum obs_base_effect effect);
gs_effect_t *oracle_rct_base_effect(enum obs_base_effect effect)
{
	rct_record("base_effect %d;", (int)effect);
	rct_clobber();
	return rct_null_effect ? NULL : (gs_effect_t *)&rct_tokens[RCT_EFFECT];
}

gs_eparam_t *oracle_rct_param(const gs_effect_t *effect, const char *name);
gs_eparam_t *oracle_rct_param(const gs_effect_t *effect, const char *name)
{
	rct_record("param %d %s;", rct_id(effect), name);
	rct_clobber();
	if (rct_null_params)
		return NULL;
	if (strcmp(name, "image") == 0)
		return (gs_eparam_t *)&rct_tokens[RCT_IMAGE];
	if (strcmp(name, "multiplier") == 0)
		return (gs_eparam_t *)&rct_tokens[RCT_MULTIPLIER];
	return (gs_eparam_t *)&rct_tokens[RCT_TOKENS - 1];
}

void oracle_rct_set_texture_srgb(gs_eparam_t *param, gs_texture_t *texture);
void oracle_rct_set_texture_srgb(gs_eparam_t *param, gs_texture_t *texture)
{
	rct_record("set_texture_srgb %d %d;", rct_id(param), rct_object_id(texture));
	rct_clobber();
}

void oracle_rct_set_float(gs_eparam_t *param, float value);
void oracle_rct_set_float(gs_eparam_t *param, float value)
{
	rct_record("set_float %d %08x;", rct_id(param), rct_bits(value));
	rct_clobber();
}

void oracle_rct_blend_push(void);
void oracle_rct_blend_push(void)
{
	rct_record("blend_push;");
	rct_clobber();
}

void oracle_rct_blend_pop(void);
void oracle_rct_blend_pop(void)
{
	rct_record("blend_pop;");
	rct_clobber();
}

void oracle_rct_blend_function(enum gs_blend_type src_c, enum gs_blend_type dest_c, enum gs_blend_type src_a,
			       enum gs_blend_type dest_a);
void oracle_rct_blend_function(enum gs_blend_type src_c, enum gs_blend_type dest_c, enum gs_blend_type src_a,
			       enum gs_blend_type dest_a)
{
	rct_record("blend_function %d %d %d %d;", (int)src_c, (int)dest_c, (int)src_a, (int)dest_a);
	rct_clobber();
}

bool oracle_rct_effect_loop(gs_effect_t *effect, const char *technique);
bool oracle_rct_effect_loop(gs_effect_t *effect, const char *technique)
{
	rct_record("loop %d %s;", rct_id(effect), technique);
	rct_clobber();
	if (rct_loops < rct_passes) {
		rct_loops++;
		return true;
	}
	rct_loops = 0;
	return false;
}

void oracle_rct_draw_sprite(gs_texture_t *texture, uint32_t flip, uint32_t width, uint32_t height);
void oracle_rct_draw_sprite(gs_texture_t *texture, uint32_t flip, uint32_t width, uint32_t height)
{
	rct_record("draw_sprite %d %u %u %u;", rct_object_id(texture), (unsigned)flip, (unsigned)width,
		   (unsigned)height);
	rct_clobber();
}

/* The real field adapters over this core, under their own names: the Rust
 * side reads through them, and obs-tests-core links them. */
#define obs (&rct_core)
#include "render-canvas-texture-fields.inc"

void oracle_obs_render_main_texture(void);
void oracle_obs_render_main_texture_src_color_only(void);
void oracle_obs_render_canvas_texture(obs_canvas_t *canvas);
void oracle_obs_render_canvas_texture_src_color_only(obs_canvas_t *canvas);
bool oracle_obs_canvas_draw_skip(bool has_mix, bool texture_rendered);
enum obs_source_draw_technique oracle_obs_canvas_draw_technique(int source_space, int current_space);
bool oracle_obs_canvas_draw_reads_white_level(int current_space);
float oracle_obs_canvas_draw_multiplier(float sdr_white_level);

#define obs_render_main_texture oracle_obs_render_main_texture
#define obs_render_main_texture_src_color_only oracle_obs_render_main_texture_src_color_only
#define obs_render_canvas_texture oracle_obs_render_canvas_texture
#define obs_render_canvas_texture_src_color_only oracle_obs_render_canvas_texture_src_color_only
#define obs_canvas_draw_skip oracle_obs_canvas_draw_skip
#define obs_canvas_draw_technique oracle_obs_canvas_draw_technique
#define obs_canvas_draw_reads_white_level oracle_obs_canvas_draw_reads_white_level
#define obs_canvas_draw_multiplier oracle_obs_canvas_draw_multiplier
#define gs_get_color_space oracle_rct_color_space
#define obs_get_video_sdr_white_level oracle_rct_white_level
#define gs_framebuffer_srgb_enabled oracle_rct_srgb_enabled
#define gs_enable_framebuffer_srgb oracle_rct_enable_srgb
#define obs_get_base_effect oracle_rct_base_effect
#define gs_effect_get_param_by_name oracle_rct_param
#define gs_effect_set_texture_srgb oracle_rct_set_texture_srgb
#define gs_effect_set_float oracle_rct_set_float
#define gs_blend_state_push oracle_rct_blend_push
#define gs_blend_state_pop oracle_rct_blend_pop
#define gs_blend_function_separate oracle_rct_blend_function
#define gs_effect_loop oracle_rct_effect_loop
#define gs_draw_sprite oracle_rct_draw_sprite
#include "libobs/obs-render-canvas-texture.c.h"
#undef gs_draw_sprite
#undef gs_effect_loop
#undef gs_blend_function_separate
#undef gs_blend_state_pop
#undef gs_blend_state_push
#undef gs_effect_set_float
#undef gs_effect_set_texture_srgb
#undef gs_effect_get_param_by_name
#undef obs_get_base_effect
#undef gs_enable_framebuffer_srgb
#undef gs_framebuffer_srgb_enabled
#undef obs_get_video_sdr_white_level
#undef gs_get_color_space
#undef obs_canvas_draw_multiplier
#undef obs_canvas_draw_reads_white_level
#undef obs_canvas_draw_technique
#undef obs_canvas_draw_skip
#undef obs_render_canvas_texture_src_color_only
#undef obs_render_canvas_texture
#undef obs_render_main_texture_src_color_only
#undef obs_render_main_texture
#undef obs

typedef void (*rct_main_fn)(void);
typedef void (*rct_canvas_fn)(obs_canvas_t *canvas);

/* One draw. state (all small ints, checked):
 *   [0] entry: 0 main, 1 main src color only, 2 canvas, 3 canvas src color only
 *   [1] the main canvas (0..2); [2] the canvas argument (0..2)
 *   [3..5] each canvas's mix: -1 none, 0 or 1
 *   [6..7] each mix's texture_rendered; [8..9] each mix's render_space
 *   [10..11] each mix's texture: -1 NULL, else token 0..7
 *   [12] gs_get_color_space(); [13] the framebuffer sRGB flag;
 *   [14] the technique's passes (0..3); [15] NULL effect; [16] NULL params;
 *   [17] clobber the mixes on each call.
 * Writes the log (NUL-terminated) and, in report, whether the core and the
 * canvases were left as they were and, without clobbering, the mixes. */
void oracle_render_canvas_texture_run(rct_main_fn main_over, rct_main_fn main_src, rct_canvas_fn canvas_over,
				      rct_canvas_fn canvas_src, const int *state, float white, char *log,
				      size_t log_len, int *report);
void oracle_render_canvas_texture_run(rct_main_fn main_over, rct_main_fn main_src, rct_canvas_fn canvas_over,
				      rct_canvas_fn canvas_src, const int *state, float white, char *log,
				      size_t log_len, int *report)
{
	if (state[0] < 0 || state[0] > 3 || state[1] < 0 || state[1] > 2 || state[2] < 0 || state[2] > 2 ||
	    state[14] < 0 || state[14] > 3 || log_len < RCT_LOG)
		abort();
	for (int i = 0; i < 3; ++i)
		if (state[3 + i] < -1 || state[3 + i] > 1)
			abort();
	for (int i = 0; i < 2; ++i)
		if (state[10 + i] < -1 || state[10 + i] > 7)
			abort();

	memset(&rct_core, 0x5a, sizeof(rct_core));
	memset(rct_canvases, 0x5a, sizeof(rct_canvases));
	memset(rct_mixes, 0x5a, sizeof(rct_mixes));
	rct_core.data.main_canvas = &rct_canvases[state[1]];
	for (int i = 0; i < 3; ++i)
		rct_canvases[i].mix = state[3 + i] < 0 ? NULL : &rct_mixes[state[3 + i]];
	for (int i = 0; i < 2; ++i) {
		rct_mixes[i].texture_rendered = state[6 + i] != 0;
		rct_mixes[i].render_space = (enum gs_color_space)state[8 + i];
		rct_mixes[i].render_texture = state[10 + i] < 0 ? NULL : (gs_texture_t *)&rct_tokens[state[10 + i]];
	}
	rct_color_space = state[12];
	rct_srgb = state[13] != 0;
	rct_passes = state[14];
	rct_null_effect = state[15] != 0;
	rct_null_params = state[16] != 0;
	rct_clobber_on = state[17] != 0;
	rct_white = white;
	rct_loops = 0;
	rct_clobbers = 0;
	rct_len = 0;
	rct_log[0] = 0;

	struct obs_core core;
	struct obs_canvas canvases[3];
	struct obs_core_video_mix mixes[2];
	memcpy(&core, &rct_core, sizeof(core));
	memcpy(canvases, rct_canvases, sizeof(canvases));
	memcpy(mixes, rct_mixes, sizeof(mixes));

	obs_canvas_t *canvas = &rct_canvases[state[2]];
	switch (state[0]) {
	case 0:
		main_over();
		break;
	case 1:
		main_src();
		break;
	case 2:
		canvas_over(canvas);
		break;
	default:
		canvas_src(canvas);
		break;
	}

	memcpy(log, rct_log, rct_len + 1);
	report[0] = memcmp(&core, &rct_core, sizeof(core)) == 0 &&
		    memcmp(canvases, rct_canvases, sizeof(canvases)) == 0;
	report[1] = rct_clobber_on || memcmp(mixes, rct_mixes, sizeof(mixes)) == 0;
	memset(&rct_core, 0, sizeof(rct_core));
	memset(rct_canvases, 0, sizeof(rct_canvases));
	memset(rct_mixes, 0, sizeof(rct_mixes));
}

/* The blend and effect enum values the port relies on. */
void oracle_render_canvas_texture_values(int *out);
void oracle_render_canvas_texture_values(int *out)
{
	out[0] = GS_BLEND_ZERO;
	out[1] = GS_BLEND_ONE;
	out[2] = GS_BLEND_INVSRCALPHA;
	out[3] = OBS_EFFECT_DEFAULT;
	out[4] = (int)sizeof(enum gs_blend_type);
	out[5] = (int)sizeof(enum gs_color_space);
}
