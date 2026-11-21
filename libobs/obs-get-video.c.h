#include "libobs/build/libobs.pch.h"

video_t *obs_get_video(void)
{
	/* The main canvas has no mix until the first obs_reset_video(): no
	 * video yet, as obs_canvas_get_video() answers for any canvas (#2237). */
	const obs_canvas_t *canvas = obs->data.main_canvas;
	return canvas && canvas->mix ? canvas->mix->video : NULL;
}
