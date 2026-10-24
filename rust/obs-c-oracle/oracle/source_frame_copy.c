/* Test-only oracle: the original libobs/obs-source-frame-copy.c.h,
 * unmodified, with its six functions renamed to oracle_* so they can link
 * next to the Rust shims. The rename also renames the struct of the same
 * name as the plan function (struct obs_source_frame_copy_plan), in the
 * header and here alike. */
#include <limits.h>
#include <stddef.h>
#include <stdint.h>

#define obs_source_frame_copy_plan oracle_obs_source_frame_copy_plan
#define obs_source_frame_copy_cache_step oracle_obs_source_frame_copy_cache_step
#define obs_source_frame_copy_cache_clean oracle_obs_source_frame_copy_cache_clean
#define obs_source_frame_copy_full_range oracle_obs_source_frame_copy_full_range
#define obs_source_frame_copy_full_range2 oracle_obs_source_frame_copy_full_range2
#define obs_source_frame_copy_preload_changed oracle_obs_source_frame_copy_preload_changed

#include "obs-source-frame-copy.c.h"

/* The layouts and values the C uses, in the order of
 * rust/obs-c-oracle/src/source_frame_copy.rs; SIZE_MAX past the end. */
size_t oracle_source_frame_copy_layout(size_t which);

#define OFF(type, member) offsetof(struct type, member)

size_t oracle_source_frame_copy_layout(size_t which)
{
	const size_t values[] = {
		sizeof(struct obs_source_frame_copy_lines),
		_Alignof(struct obs_source_frame_copy_lines),
		OFF(obs_source_frame_copy_lines, linesize),
		sizeof(struct obs_source_frame_plane_copy),
		_Alignof(struct obs_source_frame_plane_copy),
		OFF(obs_source_frame_plane_copy, whole),
		OFF(obs_source_frame_plane_copy, bytes),
		OFF(obs_source_frame_plane_copy, rows),
		OFF(obs_source_frame_plane_copy, src_stride),
		OFF(obs_source_frame_plane_copy, dst_stride),
		/* the macro above renamed this struct's tag */
		sizeof(struct obs_source_frame_copy_plan),
		_Alignof(struct obs_source_frame_copy_plan),
		OFF(obs_source_frame_copy_plan, color_range),
		OFF(obs_source_frame_copy_plan, count),
		OFF(obs_source_frame_copy_plan, planes),
		sizeof(struct obs_source_frame_copy_key),
		_Alignof(struct obs_source_frame_copy_key),
		OFF(obs_source_frame_copy_key, width),
		OFF(obs_source_frame_copy_key, height),
		OFF(obs_source_frame_copy_key, convert_type),
		sizeof(struct obs_source_frame_cache_age),
		_Alignof(struct obs_source_frame_cache_age),
		OFF(obs_source_frame_cache_age, counted),
		OFF(obs_source_frame_cache_age, unused_count),
		OFF(obs_source_frame_cache_age, drop),
		sizeof(struct obs_source_frame_copy_shape),
		_Alignof(struct obs_source_frame_copy_shape),
		OFF(obs_source_frame_copy_shape, format),
		OFF(obs_source_frame_copy_shape, width),
		OFF(obs_source_frame_copy_shape, height),
		sizeof(enum obs_source_frame_cache_step),
		OBS_SOURCE_FRAME_CACHE_KEEP,
		OBS_SOURCE_FRAME_CACHE_RESET,
		OBS_SOURCE_FRAME_CACHE_OVERFLOW,
		OBS_SOURCE_FRAME_COPY_PLANES,
		OBS_SOURCE_FRAME_COPY_MAX_ASYNC_FRAMES,
		OBS_SOURCE_FRAME_COPY_MAX_UNUSED,
		sizeof(long),
		sizeof(size_t),
		sizeof(int),
		sizeof(bool),
		sizeof(enum video_format),
		sizeof(enum video_range_type),
		VIDEO_RANGE_DEFAULT,
		VIDEO_RANGE_PARTIAL,
		VIDEO_RANGE_FULL,
	};
	return which < sizeof(values) / sizeof(values[0]) ? values[which] : SIZE_MAX;
}

/* Every enum video_format value, in enum order (VIDEO_FORMAT_NONE to
 * VIDEO_FORMAT_R10L); INT_MIN past the end. */
int oracle_source_frame_copy_format(size_t which);

int oracle_source_frame_copy_format(size_t which)
{
	const int values[] = {
		VIDEO_FORMAT_NONE, VIDEO_FORMAT_I420, VIDEO_FORMAT_NV12, VIDEO_FORMAT_YVYU, VIDEO_FORMAT_YUY2,
		VIDEO_FORMAT_UYVY, VIDEO_FORMAT_RGBA, VIDEO_FORMAT_BGRA, VIDEO_FORMAT_BGRX, VIDEO_FORMAT_Y800,
		VIDEO_FORMAT_I444, VIDEO_FORMAT_BGR3, VIDEO_FORMAT_I422, VIDEO_FORMAT_I40A, VIDEO_FORMAT_I42A,
		VIDEO_FORMAT_YUVA, VIDEO_FORMAT_AYUV, VIDEO_FORMAT_I010, VIDEO_FORMAT_P010, VIDEO_FORMAT_I210,
		VIDEO_FORMAT_I412, VIDEO_FORMAT_YA2L, VIDEO_FORMAT_P216, VIDEO_FORMAT_P416, VIDEO_FORMAT_V210,
		VIDEO_FORMAT_R10L,
	};
	return which < sizeof(values) / sizeof(values[0]) ? values[which] : INT_MIN;
}

#undef OFF
#undef obs_source_frame_copy_preload_changed
#undef obs_source_frame_copy_full_range2
#undef obs_source_frame_copy_full_range
#undef obs_source_frame_copy_cache_clean
#undef obs_source_frame_copy_cache_step
#undef obs_source_frame_copy_plan
