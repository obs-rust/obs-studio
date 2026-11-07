#include "libobs/build/libobs.pch.h"

video_t *obs_canvas_get_video(const obs_canvas_t *canvas)
{
	return canvas->mix ? canvas->mix->video : NULL;
}
