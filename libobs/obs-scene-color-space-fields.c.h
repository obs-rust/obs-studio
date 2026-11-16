#include "libobs/build/libobs.pch.h"

bool obs_scene_video_color_space_query(int *colorspace);
bool obs_scene_video_color_space_query(int *colorspace)
{
	struct obs_video_info info;
	if (!obs_get_video_info(&info))
		return false;
	*colorspace = (int)info.colorspace;
	return true;
}
