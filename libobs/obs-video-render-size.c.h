/* Private bridge: graphics calls stay bound to the caller's graphics context. */
#include "libobs/build/libobs.pch.h"

void obs_video_set_render_size(uint32_t width, uint32_t height, void (*depth)(bool), void (*cull)(enum gs_cull_mode),
			       void (*ortho)(float, float, float, float, float, float),
			       void (*viewport)(int, int, int, int));

static inline void set_render_size(uint32_t width, uint32_t height)
{
	obs_video_set_render_size(width, height, gs_enable_depth_test, gs_set_cull_mode, gs_ortho, gs_set_viewport);
}
