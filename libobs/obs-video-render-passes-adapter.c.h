#include "libobs/build/libobs.pch.h"

/* Only synchronous graphics calls and private layout access live here.
 * The Rust core owns their order and the pass loops. */
static inline void set_render_size(uint32_t width, uint32_t height);

extern void obs_video_draw_mix_texture(size_t index, uintptr_t (*call)(uint32_t, uintptr_t, uintptr_t));
extern void obs_video_render_convert_plane(gs_effect_t *effect, gs_texture_t *target, const char *name,
					   uintptr_t (*call)(uint32_t, uintptr_t, uintptr_t));

static uintptr_t video_render_passes_call(uint32_t operation, uintptr_t a, uintptr_t b)
{
	switch (operation) {
	case 0:
		return (uintptr_t)obs->video.mixes.array[a]->render_texture;
	case 1:
		return (uintptr_t)obs_get_base_effect(OBS_EFFECT_DEFAULT);
	case 2:
		return (uintptr_t)gs_effect_get_param_by_name((gs_effect_t *)a, "image");
	case 3:
		gs_effect_set_texture_srgb((gs_eparam_t *)a, (gs_texture_t *)b);
		break;
	case 4:
		gs_blend_state_push();
		break;
	case 5:
		gs_enable_blending(false);
		break;
	case 6:
		gs_enable_framebuffer_srgb(true);
		break;
	case 7:
		return gs_effect_loop((gs_effect_t *)a, "Draw");
	case 8:
		gs_draw_sprite((gs_texture_t *)a, 0, 0, 0);
		break;
	case 9:
		gs_enable_framebuffer_srgb(false);
		break;
	case 10:
		gs_blend_state_pop();
		break;
	case 11:
		return (uintptr_t)gs_effect_get_technique((gs_effect_t *)a, (const char *)b);
	case 12:
		return gs_texture_get_width((gs_texture_t *)a);
	case 13:
		return gs_texture_get_height((gs_texture_t *)a);
	case 14:
		gs_set_render_target((gs_texture_t *)a, NULL);
		break;
	case 15:
		set_render_size((uint32_t)a, (uint32_t)b);
		break;
	case 16:
		return gs_technique_begin((gs_technique_t *)a);
	case 17:
		return gs_technique_begin_pass((gs_technique_t *)a, b);
	case 18:
		gs_draw(GS_TRIS, 0, 3);
		break;
	case 19:
		gs_technique_end_pass((gs_technique_t *)a);
		break;
	case 20:
		gs_technique_end((gs_technique_t *)a);
		break;
	}
	return 0;
}

static inline void draw_mix_texture(const size_t mix_idx)
{
	obs_video_draw_mix_texture(mix_idx, video_render_passes_call);
}

static void render_convert_plane(gs_effect_t *effect, gs_texture_t *target, const char *tech_name)
{
	obs_video_render_convert_plane(effect, target, tech_name, video_render_passes_call);
}
