#include "libobs/build/libobs.pch.h"

/* The C reference and oracle share the original helper verbatim. Bind graphics
 * calls to per-invocation callbacks, with no globals or substitute behavior. */
void obs_video_set_render_size(uint32_t width, uint32_t height, void (*depth)(bool), void (*cull)(enum gs_cull_mode),
			       void (*ortho)(float, float, float, float, float, float),
			       void (*viewport)(int, int, int, int));

#define set_render_size(width, height)                                                                                  \
	render_size_reference(width, height, void (*depth)(bool), void (*cull)(enum gs_cull_mode),                           \
	                      void (*ortho)(float, float, float, float, float, float),                                    \
	                      void (*viewport)(int, int, int, int))
#define gs_enable_depth_test depth
#define gs_set_cull_mode cull
#define gs_ortho ortho
#define gs_set_viewport viewport
#include "rust/obs-c-oracle/legacy/obs-video-render-size.inc"
#undef gs_set_viewport
#undef gs_ortho
#undef gs_set_cull_mode
#undef gs_enable_depth_test
#undef set_render_size

void obs_video_set_render_size(uint32_t width, uint32_t height, void (*depth)(bool), void (*cull)(enum gs_cull_mode),
			       void (*ortho)(float, float, float, float, float, float),
			       void (*viewport)(int, int, int, int))
{
	render_size_reference(width, height, depth, cull, ortho, viewport);
}
