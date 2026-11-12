/* Private bridge: allocation inputs and borrowed texture destinations. */
#pragma once

struct obs_source_deinterlace_texture_fields {
	int format;
	uint8_t trc;
	bool gpu;
	int channels;
	uint32_t width, height;
	uint32_t widths[MAX_AV_PLANES], heights[MAX_AV_PLANES];
	int formats[MAX_AV_PLANES];
	gs_texture_t **textures;
	gs_texrender_t **render;
};

struct obs_source_deinterlace_texture_fields obs_source_deinterlace_texture_fields(obs_source_t *source);
