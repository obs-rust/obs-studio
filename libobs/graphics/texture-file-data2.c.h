#include "libobs/build/libobs-media.pch.h"

uint8_t *gs_create_texture_file_data2(const char *file, enum gs_image_alpha_mode alpha_mode,
				      enum gs_color_format *format, uint32_t *cx_out, uint32_t *cy_out)
{
	enum gs_color_space unused;
	return gs_create_texture_file_data3(file, alpha_mode, format, cx_out, cy_out, &unused);
}
