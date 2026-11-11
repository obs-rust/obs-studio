#pragma once

#include <stdbool.h>
#include <stddef.h>

struct obs_core;
struct video_output;

/* The graphics thread's stop check (stop_requested() in
 * obs-video-stop-requested.c), a private adapter, not a new public API. It is
 * Rust in hybrid libobs (obs-core::video_stop_requested) and the original C in
 * the C reference (obs-video-stop-requested-reference.c). Hidden, not EXPORT.
 *
 * True when every mix's output has stopped, and for no mixes. It takes the
 * mixes mutex once around the whole walk, reads the mix count once under it,
 * and asks every mix in order with no early exit. The Rust reaches the mix
 * list only through the accessors, so the mutex and the array stay C-owned;
 * the C reference reads core directly and ignores them. */
bool obs_video_stop_requested(struct obs_core *core, void (*lock)(struct obs_core *core),
			      void (*unlock)(struct obs_core *core), size_t (*count)(struct obs_core *core),
			      struct video_output *(*video)(struct obs_core *core, size_t index),
			      bool (*stopped)(struct video_output *video));
