#include "libobs/build/libobs.pch.h"

bool obs_get_video_info(struct obs_video_info *ovi)
{
	/* no core (before obs_startup): no video (#1509) */
	if (!obs || !obs->video.graphics || !obs->data.main_canvas->mix)
		return false;

	*ovi = obs->data.main_canvas->mix->ovi;
	return true;
}
