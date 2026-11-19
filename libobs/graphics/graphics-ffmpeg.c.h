#include "libobs/build/libobs-media.pch.h"

void gs_init_image_deps(void);
void gs_free_image_deps(void);

void gs_init_image_deps(void) {}

void gs_free_image_deps(void) {}

uint8_t *gs_create_texture_file_data3(const char *file, enum gs_image_alpha_mode alpha_mode,
				      enum gs_color_format *format, uint32_t *cx_out, uint32_t *cy_out,
				      enum gs_color_space *space)
{
	struct ffmpeg_image image = {0};
	uint8_t *data = NULL;

	if (ffmpeg_image_init(&image, file, gs_image_max_texture_size)) {
		struct gs_image_decode_context decode = {&image, ffmpeg_image_too_large, ffmpeg_image_reformat_frame};
		data = gs_ffmpeg_image_decode(&decode, (int)alpha_mode);
		if (data) {
			*format = (enum gs_color_format)gs_ffmpeg_image_color_format(image.format);
			*cx_out = (uint32_t)image.cx;
			*cy_out = (uint32_t)image.cy;
			*space = GS_CS_SRGB;
		}

		ffmpeg_image_free(&image);
	}

	if (data == NULL && !image.too_large)
		data = gs_image_host_fallback_load(file, format, cx_out, cy_out, space);

	return data;
}
