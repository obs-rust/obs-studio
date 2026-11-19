#include "libobs/build/libobs.pch.h"

/* The original staging cleanup, for the C reference and the Tier 3 oracle. Its
 * body is the C's, verbatim, over the mix and the unmap it is given. */
#define gs_stagesurface_unmap unmap_last_surface_unmap
static inline void unmap_last_surface_reference(struct obs_core_video_mix *video,
						void (*gs_stagesurface_unmap)(gs_stagesurf_t *))
{
	for (int c = 0; c < NUM_CHANNELS; ++c) {
		if (video->mapped_surfaces[c]) {
			gs_stagesurface_unmap(video->mapped_surfaces[c]);
			video->mapped_surfaces[c] = NULL;
		}
	}
}
#undef gs_stagesurface_unmap

void obs_video_unmap_last_surface(struct obs_core_video_mix *video, size_t channels,
				  gs_stagesurf_t *(*mapped)(struct obs_core_video_mix *video, size_t channel),
				  void (*clear)(struct obs_core_video_mix *video, size_t channel),
				  void (*unmap)(gs_stagesurf_t *surface))
{
	UNUSED_PARAMETER(channels);
	UNUSED_PARAMETER(mapped);
	UNUSED_PARAMETER(clear);
	unmap_last_surface_reference(video, unmap);
}
