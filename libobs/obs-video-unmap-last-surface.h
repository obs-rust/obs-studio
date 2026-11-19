#pragma once

#include <stddef.h>

struct obs_core_video_mix;
struct gs_stage_surface;

/* The staging cleanup at the start of each output stage (unmap_last_surface()
 * in obs-video-unmap-last-surface.c), a private adapter, not a new public API.
 * It is Rust in hybrid libobs (obs-core::video_unmap_last_surface) and the
 * original C in the C reference (obs-video-unmap-last-surface-reference.c).
 * Hidden, not EXPORT.
 *
 * For each of the mix's channels slots in ascending order, a mapped staging
 * surface is passed to unmap and only then is its slot reset to NULL; a NULL
 * slot makes no call. The Rust reaches the slots only through the accessors,
 * so mapped_surfaces stays C-owned; the C reference reads video directly and
 * ignores them. Called on the graphics thread, which owns the context unmap
 * needs. */
void obs_video_unmap_last_surface(struct obs_core_video_mix *video, size_t channels,
				  struct gs_stage_surface *(*mapped)(struct obs_core_video_mix *video, size_t channel),
				  void (*clear)(struct obs_core_video_mix *video, size_t channel),
				  void (*unmap)(struct gs_stage_surface *surface));
