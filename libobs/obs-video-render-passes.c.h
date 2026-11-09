#include "libobs/build/libobs.pch.h"

/* Defined by obs-video.c.h in this unity group; keep the existing helper. */
static inline void set_render_size(uint32_t width, uint32_t height);

static inline void draw_mix_texture(const size_t mix_idx)
{
	gs_texture_t *tex = obs->video.mixes.array[mix_idx]->render_texture;
	gs_effect_t *effect = obs_get_base_effect(OBS_EFFECT_DEFAULT);
	gs_eparam_t *param = gs_effect_get_param_by_name(effect, "image");
	gs_effect_set_texture_srgb(param, tex);

	/* A copy of the other mix's canvas, alpha included: blending would
	 * multiply the colour by an alpha that the canvas can have at 0 where
	 * colour shows (Subtract blending), and those areas would turn black. */
	gs_blend_state_push();
	gs_enable_blending(false);
	gs_enable_framebuffer_srgb(true);
	while (gs_effect_loop(effect, "Draw"))
		gs_draw_sprite(tex, 0, 0, 0);
	gs_enable_framebuffer_srgb(false);
	gs_blend_state_pop();
}

static void render_convert_plane(gs_effect_t *effect, gs_texture_t *target, const char *tech_name)
{
	gs_technique_t *tech = gs_effect_get_technique(effect, tech_name);

	const uint32_t width = gs_texture_get_width(target);
	const uint32_t height = gs_texture_get_height(target);

	gs_set_render_target(target, NULL);
	set_render_size(width, height);

	size_t passes = gs_technique_begin(tech);
	for (size_t i = 0; i < passes; i++) {
		gs_technique_begin_pass(tech, i);
		gs_draw(GS_TRIS, 0, 3);
		gs_technique_end_pass(tech);
	}
	gs_technique_end(tech);
}
