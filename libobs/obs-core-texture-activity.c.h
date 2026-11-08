#include "libobs/build/libobs.pch.h"

/* The main canvas has no mix until the first obs_reset_video() (#2253), and
 * there is no canvas at all with no core (#2566): no video, so no texture
 * conversion either. */
bool obs_nv12_tex_active(void)
{
	if (!obs)
		return false;

	const obs_canvas_t *canvas = obs->data.main_canvas;
	const struct obs_core_video_mix *video = canvas ? canvas->mix : NULL;
	return video && video->using_nv12_tex;
}

bool obs_p010_tex_active(void)
{
	if (!obs)
		return false;

	const obs_canvas_t *canvas = obs->data.main_canvas;
	const struct obs_core_video_mix *video = canvas ? canvas->mix : NULL;
	return video && video->using_p010_tex;
}
