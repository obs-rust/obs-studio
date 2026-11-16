/* Original helper, with graphics callbacks supplied by each parity invocation. */
#include <stdbool.h>
#include <stdint.h>
#include "graphics/graphics.h"

#define set_render_size(width, height)                                                                                  \
	render_size_oracle(width, height, void (*depth)(bool), void (*cull)(enum gs_cull_mode),                              \
	                   void (*ortho)(float, float, float, float, float, float),                                       \
	                   void (*viewport)(int, int, int, int))
#define gs_enable_depth_test depth
#define gs_set_cull_mode cull
#define gs_ortho ortho
#define gs_set_viewport viewport
#include "obs-video-render-size.inc"
#undef gs_set_viewport
#undef gs_ortho
#undef gs_set_cull_mode
#undef gs_enable_depth_test
#undef set_render_size

void oracle_obs_video_set_render_size(uint32_t width, uint32_t height, void (*depth)(bool),
				      void (*cull)(enum gs_cull_mode),
				      void (*ortho)(float, float, float, float, float, float),
				      void (*viewport)(int, int, int, int));
void oracle_obs_video_set_render_size(uint32_t width, uint32_t height, void (*depth)(bool),
				      void (*cull)(enum gs_cull_mode),
				      void (*ortho)(float, float, float, float, float, float),
				      void (*viewport)(int, int, int, int))
{
	render_size_oracle(width, height, depth, cull, ortho, viewport);
}
