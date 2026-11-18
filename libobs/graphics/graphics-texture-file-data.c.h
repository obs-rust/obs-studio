#include "libobs/build/libobs-media.pch.h"

uint8_t *gs_create_texture_file_data(const char *file, enum gs_color_format *format, uint32_t *cx_out, uint32_t *cy_out)
{
	struct ffmpeg_image image;
	uint8_t *data = NULL;

	if (ffmpeg_image_init(&image, file, gs_image_max_texture_size)) {
		struct gs_image_decode_context decode = {&image, ffmpeg_image_too_large, ffmpeg_image_reformat_frame};
		data = gs_ffmpeg_image_decode(&decode, (int)GS_IMAGE_ALPHA_STRAIGHT);
		if (data) {
			*format = (enum gs_color_format)gs_ffmpeg_image_color_format(image.format);
			*cx_out = (uint32_t)image.cx;
			*cy_out = (uint32_t)image.cy;
		}

		ffmpeg_image_free(&image);
	}

	return data;
}
