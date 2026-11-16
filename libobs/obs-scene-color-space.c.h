#include "libobs/build/libobs.pch.h"

enum gs_color_space scene_video_get_color_space(void *data, size_t count, const enum gs_color_space *preferred_spaces);
enum gs_color_space scene_video_get_color_space(void *data, size_t count, const enum gs_color_space *preferred_spaces)
{
	UNUSED_PARAMETER(data);
	UNUSED_PARAMETER(count);
	UNUSED_PARAMETER(preferred_spaces);

	enum gs_color_space space = GS_CS_SRGB;
	struct obs_video_info ovi;
	if (obs_get_video_info(&ovi)) {
		if (ovi.colorspace == VIDEO_CS_2100_PQ || ovi.colorspace == VIDEO_CS_2100_HLG)
			space = GS_CS_709_EXTENDED;
	}

	return space;
}
