/* Tier 3: the unchanged extracted obs_source_frame_copy() (copy_frame_data()
 * and its helpers) against the Rust export. Both ask the same
 * obs_source_frame_copy_plan() (the Rust one; its own parity is
 * source_frame_copy.c), so this compares running the plan: the fields
 * written and the bytes copied. */
#define _NO_CRT_STDIO_INLINE
#include <stddef.h>
#include <stdint.h>
#define obs_source_frame_copy oracle_obs_source_frame_copy
#include "libobs/build/libobs.pch.h"
#include "source-frame-copy-data.inc"
#undef obs_source_frame_copy

/* The layout of struct obs_source_frame, in the order of
 * rust/obs-source/tests/source/frame_copy_data_parity.rs; SIZE_MAX past
 * the end. */
size_t oracle_source_frame_copy_data_layout(size_t which);

#define OFF(member) offsetof(struct obs_source_frame, member)

size_t oracle_source_frame_copy_data_layout(size_t which)
{
	const size_t values[] = {
		sizeof(struct obs_source_frame),
		_Alignof(struct obs_source_frame),
		OFF(data),
		OFF(linesize),
		OFF(width),
		OFF(height),
		OFF(timestamp),
		OFF(format),
		OFF(color_matrix),
		OFF(full_range),
		OFF(max_luminance),
		OFF(color_range_min),
		OFF(color_range_max),
		OFF(flip),
		OFF(flags),
		OFF(trc),
		OFF(refs),
		OFF(prev_frame),
		sizeof(enum video_format),
		MAX_AV_PLANES,
	};
	return which < sizeof(values) / sizeof(values[0]) ? values[which] : SIZE_MAX;
}
