#include "libobs/build/libobs.pch.h"

gs_texture_t *obs_get_main_texture(void)
{
	/* The main canvas has no mix until the first obs_reset_video() (#2253). */
	const obs_canvas_t *canvas = obs->data.main_canvas;
	struct obs_core_video_mix *video = canvas ? canvas->mix : NULL;

	if (!video || !os_atomic_load_bool(&video->texture_rendered))
		return NULL;

	return video->render_texture;
}
