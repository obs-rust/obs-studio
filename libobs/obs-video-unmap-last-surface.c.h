#include "libobs/build/libobs.pch.h"

/* Private bridge to obs_video_unmap_last_surface()
 * (obs-video-unmap-last-surface.h): these accessors give it the mix's mapped
 * staging surfaces, which stay C-owned. */
static gs_stagesurf_t *unmap_last_surface_mapped(struct obs_core_video_mix *video, size_t channel)
{
	return video->mapped_surfaces[channel];
}

static void unmap_last_surface_clear(struct obs_core_video_mix *video, size_t channel)
{
	video->mapped_surfaces[channel] = NULL;
}

static inline void unmap_last_surface(struct obs_core_video_mix *video)
{
	obs_video_unmap_last_surface(video, NUM_CHANNELS, unmap_last_surface_mapped, unmap_last_surface_clear,
				     gs_stagesurface_unmap);
}
