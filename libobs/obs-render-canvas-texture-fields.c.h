#include "libobs/build/libobs.pch.h"

/* Private adapters for the Rust canvas texture draw entry points
 * (obs-render-canvas-texture.c.h, #5543): the current main-canvas field, a
 * canvas's mix, and the mix's texture_rendered flag (an atomic load),
 * render_space and render_texture, each one borrowed read as the original
 * did, with no reference or lock taken. The layouts stay in C. The canvas
 * variants never read the core, so the main canvas is its own adapter. */
const obs_canvas_t *obs_render_canvas_texture_main_canvas(void);
const obs_canvas_t *obs_render_canvas_texture_main_canvas(void)
{
	return obs->data.main_canvas;
}

const struct obs_core_video_mix *obs_render_canvas_texture_mix(const obs_canvas_t *canvas);
const struct obs_core_video_mix *obs_render_canvas_texture_mix(const obs_canvas_t *canvas)
{
	return canvas->mix;
}

bool obs_render_canvas_texture_rendered(const struct obs_core_video_mix *mix);
bool obs_render_canvas_texture_rendered(const struct obs_core_video_mix *mix)
{
	return os_atomic_load_bool(&mix->texture_rendered);
}

int obs_render_canvas_texture_space(const struct obs_core_video_mix *mix);
int obs_render_canvas_texture_space(const struct obs_core_video_mix *mix)
{
	return (int)mix->render_space;
}

gs_texture_t *obs_render_canvas_texture_texture(const struct obs_core_video_mix *mix);
gs_texture_t *obs_render_canvas_texture_texture(const struct obs_core_video_mix *mix)
{
	return mix->render_texture;
}
