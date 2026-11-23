#include "libobs/build/libobs.pch.h"

static void obs_render_canvas_texture_internal(obs_canvas_t *canvas, enum gs_blend_type src_c,
					       enum gs_blend_type dest_c, enum gs_blend_type src_a,
					       enum gs_blend_type dest_a)
{
	struct obs_core_video_mix *video;
	gs_texture_t *tex;
	gs_effect_t *effect;
	gs_eparam_t *param;

	video = canvas->mix;
	if (obs_canvas_draw_skip(video != NULL, video && os_atomic_load_bool(&video->texture_rendered)))
		return;

	const enum gs_color_space source_space = video->render_space;
	const enum gs_color_space current_space = gs_get_color_space();
	const char *tech_name =
		obs_source_draw_technique_name(obs_canvas_draw_technique((int)source_space, (int)current_space));
	float multiplier = 1.f;
	if (obs_canvas_draw_reads_white_level((int)current_space))
		multiplier = obs_canvas_draw_multiplier(obs_get_video_sdr_white_level());

	const bool previous = gs_framebuffer_srgb_enabled();
	gs_enable_framebuffer_srgb(true);

	tex = video->render_texture;
	effect = obs_get_base_effect(OBS_EFFECT_DEFAULT);
	param = gs_effect_get_param_by_name(effect, "image");
	gs_effect_set_texture_srgb(param, tex);
	param = gs_effect_get_param_by_name(effect, "multiplier");
	gs_effect_set_float(param, multiplier);

	gs_blend_state_push();
	gs_blend_function_separate(src_c, dest_c, src_a, dest_a);

	while (gs_effect_loop(effect, tech_name))
		gs_draw_sprite(tex, 0, 0, 0);

	gs_blend_state_pop();

	gs_enable_framebuffer_srgb(previous);
}

void obs_render_main_texture(void)
{
	obs_render_canvas_texture_internal(obs->data.main_canvas, GS_BLEND_ONE, GS_BLEND_INVSRCALPHA, GS_BLEND_ONE,
					   GS_BLEND_INVSRCALPHA);
}

void obs_render_main_texture_src_color_only(void)
{
	obs_render_canvas_texture_internal(obs->data.main_canvas, GS_BLEND_ONE, GS_BLEND_ZERO, GS_BLEND_ONE,
					   GS_BLEND_INVSRCALPHA);
}

void obs_render_canvas_texture(obs_canvas_t *canvas)
{
	obs_render_canvas_texture_internal(canvas, GS_BLEND_ONE, GS_BLEND_INVSRCALPHA, GS_BLEND_ONE,
					   GS_BLEND_INVSRCALPHA);
}

void obs_render_canvas_texture_src_color_only(obs_canvas_t *canvas)
{
	obs_render_canvas_texture_internal(canvas, GS_BLEND_ONE, GS_BLEND_ZERO, GS_BLEND_ONE, GS_BLEND_INVSRCALPHA);
}
