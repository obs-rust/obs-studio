#include "libobs/build/libobs-ported-graphics-ffmpeg-decode.pch.h"

void *gs_ffmpeg_image_decode(void *info, int alpha_mode)
{
	struct gs_image_decode_context *context = info;
	return ffmpeg_image_decode(context->image, (enum gs_image_alpha_mode)alpha_mode);
}
