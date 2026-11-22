#include "libobs/build/libobs.pch.h"

/* Private adapters for the Rust obs_get_main_texture(): the current
 * main-canvas field, its mix, the mix's texture_rendered flag (an atomic
 * load) and its render_texture, each one borrowed read as the original did,
 * with no reference or lock taken. The layouts stay in C. */
struct obs_get_main_texture_fields {
	const obs_canvas_t *canvas;
	const struct obs_core_video_mix *(*mix)(const obs_canvas_t *canvas);
	bool (*rendered)(const struct obs_core_video_mix *mix);
	gs_texture_t *(*texture)(const struct obs_core_video_mix *mix);
};

static const struct obs_core_video_mix *obs_get_main_texture_canvas_mix(const obs_canvas_t *canvas)
{
	return canvas->mix;
}

static bool obs_get_main_texture_mix_rendered(const struct obs_core_video_mix *mix)
{
	return os_atomic_load_bool(&mix->texture_rendered);
}

static gs_texture_t *obs_get_main_texture_mix_texture(const struct obs_core_video_mix *mix)
{
	return mix->render_texture;
}

struct obs_get_main_texture_fields obs_get_main_texture_fields(void);
struct obs_get_main_texture_fields obs_get_main_texture_fields(void)
{
	return (struct obs_get_main_texture_fields){obs->data.main_canvas, obs_get_main_texture_canvas_mix,
						    obs_get_main_texture_mix_rendered,
						    obs_get_main_texture_mix_texture};
}
