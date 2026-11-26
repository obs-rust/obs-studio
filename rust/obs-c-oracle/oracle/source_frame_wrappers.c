/* Tier 3: the unchanged extracted async-frame entry points
 * (libobs/obs-source-frame-wrappers.c.h) against the Rust exports, over a
 * real private source layout.
 *
 * The production adapters the Rust exports call
 * (libobs/obs-source-frame-wrappers-fields.c.h) are compiled here, once for
 * every test binary. Both sides run with destroying() and the three workers
 * routed through recorders: destroying() reads the real field, and each
 * worker records which one it is, the frame pointer it got and a copy of
 * the frame, so the parity test compares the exact frame each worker
 * receives, field by field. */
#define _NO_CRT_STDIO_INLINE
#include <stddef.h>
#include <string.h>
#include "libobs/build/libobs.pch.h"

enum {
	FW_DESTROYING = 1,
	FW_OUTPUT = 2,
	FW_PRELOAD = 3,
	FW_SET = 4,
};

#define FW_MAX_EVENTS 8

struct fw_event {
	int kind;
	const void *source;
	const struct obs_source_frame *frame;
	struct obs_source_frame copy;
};

static struct fw_event fw_events[FW_MAX_EVENTS];
static size_t fw_event_count;

static void fw_record(int kind, const void *source, const struct obs_source_frame *frame)
{
	if (fw_event_count < FW_MAX_EVENTS) {
		struct fw_event *event = &fw_events[fw_event_count];
		memset(event, 0, sizeof(*event));
		event->kind = kind;
		event->source = source;
		event->frame = frame;
		if (frame)
			event->copy = *frame;
	}
	fw_event_count++;
}

static bool fw_destroying(const struct obs_source *source)
{
	fw_record(FW_DESTROYING, source, NULL);
	return os_atomic_load_long(&source->destroying);
}

static void fw_output(obs_source_t *source, const struct obs_source_frame *frame)
{
	fw_record(FW_OUTPUT, source, frame);
}

static void fw_preload(obs_source_t *source, const struct obs_source_frame *frame)
{
	fw_record(FW_PRELOAD, source, frame);
}

static void fw_set(obs_source_t *source, const struct obs_source_frame *frame)
{
	fw_record(FW_SET, source, frame);
}

#define destroying fw_destroying
#define obs_source_output_video_internal fw_output
#define obs_source_preload_video_internal fw_preload
#define obs_source_set_video_frame_internal fw_set

#define obs_source_output_video oracle_obs_source_output_video
#define obs_source_output_video2 oracle_obs_source_output_video2
#define obs_source_preload_video oracle_obs_source_preload_video
#define obs_source_preload_video2 oracle_obs_source_preload_video2
#define obs_source_set_video_frame oracle_obs_source_set_video_frame
#define obs_source_set_video_frame2 oracle_obs_source_set_video_frame2
void oracle_obs_source_output_video(obs_source_t *source, const struct obs_source_frame *frame);
void oracle_obs_source_output_video2(obs_source_t *source, const struct obs_source_frame2 *frame);
void oracle_obs_source_preload_video(obs_source_t *source, const struct obs_source_frame *frame);
void oracle_obs_source_preload_video2(obs_source_t *source, const struct obs_source_frame2 *frame);
void oracle_obs_source_set_video_frame(obs_source_t *source, const struct obs_source_frame *frame);
void oracle_obs_source_set_video_frame2(obs_source_t *source, const struct obs_source_frame2 *frame);
#include "source-frame-wrappers.inc"
#undef obs_source_output_video
#undef obs_source_output_video2
#undef obs_source_preload_video
#undef obs_source_preload_video2
#undef obs_source_set_video_frame
#undef obs_source_set_video_frame2

#include "source-frame-wrappers-fields.inc"

#undef destroying
#undef obs_source_output_video_internal
#undef obs_source_preload_video_internal
#undef obs_source_set_video_frame_internal

/* The calls recorded since the last read, oldest first; returns the count
 * (more than max when some were dropped) and clears the log. Each frame
 * copy goes to copies[i] when the worker got a frame. */
size_t oracle_fw_events(int *kinds, const void **sources, const void **frames, struct obs_source_frame *copies,
			size_t max);
size_t oracle_fw_events(int *kinds, const void **sources, const void **frames, struct obs_source_frame *copies,
			size_t max)
{
	size_t count = fw_event_count;
	for (size_t i = 0; i < count && i < max && i < FW_MAX_EVENTS; i++) {
		kinds[i] = fw_events[i].kind;
		sources[i] = fw_events[i].source;
		frames[i] = fw_events[i].frame;
		copies[i] = fw_events[i].copy;
	}
	fw_event_count = 0;
	return count;
}

void *oracle_fw_source_new(long destroying);
void *oracle_fw_source_new(long destroying)
{
	struct obs_source *source = bzalloc(sizeof(*source));
	source->destroying = destroying;
	return source;
}

void oracle_fw_source_free(struct obs_source *source);
void oracle_fw_source_free(struct obs_source *source)
{
	bfree(source);
}

/* The layouts the Rust mirrors must match: sizes, then each field's
 * offset, in declaration order. */
size_t oracle_fw_frame_layout(size_t *out, size_t max);
size_t oracle_fw_frame_layout(size_t *out, size_t max)
{
	const size_t layout[] = {
		sizeof(struct obs_source_frame),
		offsetof(struct obs_source_frame, data),
		offsetof(struct obs_source_frame, linesize),
		offsetof(struct obs_source_frame, width),
		offsetof(struct obs_source_frame, height),
		offsetof(struct obs_source_frame, timestamp),
		offsetof(struct obs_source_frame, format),
		offsetof(struct obs_source_frame, color_matrix),
		offsetof(struct obs_source_frame, full_range),
		offsetof(struct obs_source_frame, max_luminance),
		offsetof(struct obs_source_frame, color_range_min),
		offsetof(struct obs_source_frame, color_range_max),
		offsetof(struct obs_source_frame, flip),
		offsetof(struct obs_source_frame, flags),
		offsetof(struct obs_source_frame, trc),
		offsetof(struct obs_source_frame, refs),
		offsetof(struct obs_source_frame, prev_frame),
		sizeof(struct obs_source_frame2),
		offsetof(struct obs_source_frame2, data),
		offsetof(struct obs_source_frame2, linesize),
		offsetof(struct obs_source_frame2, width),
		offsetof(struct obs_source_frame2, height),
		offsetof(struct obs_source_frame2, timestamp),
		offsetof(struct obs_source_frame2, format),
		offsetof(struct obs_source_frame2, range),
		offsetof(struct obs_source_frame2, color_matrix),
		offsetof(struct obs_source_frame2, color_range_min),
		offsetof(struct obs_source_frame2, color_range_max),
		offsetof(struct obs_source_frame2, flip),
		offsetof(struct obs_source_frame2, flags),
		offsetof(struct obs_source_frame2, trc),
	};
	const size_t count = sizeof(layout) / sizeof(layout[0]);
	for (size_t i = 0; i < count && i < max; i++)
		out[i] = layout[i];
	return count;
}
