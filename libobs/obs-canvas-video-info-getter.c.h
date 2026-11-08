#include "libobs/build/libobs.pch.h"

bool obs_canvas_get_video_info(const obs_canvas_t *canvas, struct obs_video_info *ovi)
{
	if (!obs->video.graphics || !canvas->mix)
		return false;

	*ovi = canvas->mix->ovi;
	return true;
}
