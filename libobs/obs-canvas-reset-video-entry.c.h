#include "libobs/build/libobs.pch.h"

bool obs_canvas_reset_video(obs_canvas_t *canvas, struct obs_video_info *ovi)
{
	if (!ovi || !obs_canvas_rules_resets(canvas->flags) || obs_video_active())
		return false;

	return obs_canvas_reset_video_internal(canvas, ovi);
}
