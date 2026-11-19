/* The original staging cleanup (the C reference) and the production bridge's
 * slot accessors, over the real private mix layout. */
#define obs_video_unmap_last_surface oracle_obs_video_unmap_last_surface
#include "libobs/obs-video-unmap-last-surface-reference.c.h"
#include "libobs/obs-video-unmap-last-surface.c.h"
#undef obs_video_unmap_last_surface
#include <stdlib.h>

void *oracle_unmap_last_surface_new(void);
void oracle_unmap_last_surface_free(void *video);
size_t oracle_unmap_last_surface_channels(void);
void *oracle_unmap_last_surface_slot(void *video, size_t channel);
void oracle_unmap_last_surface_set(void *video, size_t channel, void *surface);
void oracle_unmap_last_surface_original(void *video, void (*unmap)(gs_stagesurf_t *));
void *oracle_unmap_last_surface_mapped(void *video, size_t channel);
void oracle_unmap_last_surface_clear(void *video, size_t channel);

/* A mix with every slot NULL; its surfaces are tokens, never dereferenced. */
void *oracle_unmap_last_surface_new(void)
{
	return calloc(1, sizeof(struct obs_core_video_mix));
}

void oracle_unmap_last_surface_free(void *video)
{
	free(video);
}

size_t oracle_unmap_last_surface_channels(void)
{
	return NUM_CHANNELS;
}

/* The slot as stored, read without the bridge. */
void *oracle_unmap_last_surface_slot(void *opaque, size_t channel)
{
	struct obs_core_video_mix *video = opaque;
	return video->mapped_surfaces[channel];
}

void oracle_unmap_last_surface_set(void *opaque, size_t channel, void *surface)
{
	struct obs_core_video_mix *video = opaque;
	video->mapped_surfaces[channel] = surface;
}

void oracle_unmap_last_surface_original(void *video, void (*unmap)(gs_stagesurf_t *))
{
	oracle_obs_video_unmap_last_surface(video, 0, NULL, NULL, unmap);
}

void *oracle_unmap_last_surface_mapped(void *video, size_t channel)
{
	return unmap_last_surface_mapped(video, channel);
}

void oracle_unmap_last_surface_clear(void *video, size_t channel)
{
	unmap_last_surface_clear(video, channel);
}
