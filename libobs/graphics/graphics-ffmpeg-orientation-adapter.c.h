#include "libobs/build/libobs-media.pch.h"

static int ffmpeg_image_get_orientation(const AVFrame *frame)
{
	return gs_ffmpeg_image_get_orientation(frame);
}
