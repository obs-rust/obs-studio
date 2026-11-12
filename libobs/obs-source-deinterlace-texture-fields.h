/* Private bridge: borrowed addresses of the allocation inputs and the texture
 * destinations. Rust reads each field where the C reads it, so nothing is
 * read before an allocation that comes before its read in the C. */
#pragma once

struct obs_source_deinterlace_texture_fields {
	const enum video_format *format;
	const uint8_t *trc;
	const bool *gpu;
	const int *channels;
	const uint32_t *width, *height;
	const uint32_t *widths, *heights;
	const enum gs_color_format *formats;
	gs_texture_t **textures;
	gs_texrender_t **render;
};

struct obs_source_deinterlace_texture_fields obs_source_deinterlace_texture_fields(obs_source_t *source);
