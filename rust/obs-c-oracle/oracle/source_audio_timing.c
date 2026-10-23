/* Test-only oracle: the original libobs/obs-source-audio-timing.c.h,
 * unmodified, with its three functions renamed to oracle_* so they can link
 * next to the Rust shims. */
#include <stddef.h>

#define obs_source_audio_timing_pre_lock oracle_obs_source_audio_timing_pre_lock
#define obs_source_audio_timing_under_lock oracle_obs_source_audio_timing_under_lock
#define obs_source_audio_timing_buffer_start oracle_obs_source_audio_timing_buffer_start

/* The C adds the int64_t sync offset to a uint64_t timestamp, which converts
 * it, as source_output_audio_data() always did. The oracle builds with
 * -Wsign-conversion and libobs does not. */
#if defined(__clang__) || defined(__GNUC__)
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wsign-conversion"
#endif

#include "obs-source-audio-timing.c.h"

#if defined(__clang__) || defined(__GNUC__)
#pragma GCC diagnostic pop
#endif

#undef obs_source_audio_timing_buffer_start
#undef obs_source_audio_timing_under_lock
#undef obs_source_audio_timing_pre_lock

/* The layouts and values the C uses, in the order of
 * rust/obs-c-oracle/src/source_audio_timing.rs. */
enum {
	ORACLE_SOURCE_AUDIO_TIMING_LAYOUT_COUNT = 39,
};

size_t oracle_source_audio_timing_layout(size_t which);

size_t oracle_source_audio_timing_layout(size_t which)
{
	const size_t values[ORACLE_SOURCE_AUDIO_TIMING_LAYOUT_COUNT] = {
		sizeof(struct obs_source_audio_timing_pre),
		_Alignof(struct obs_source_audio_timing_pre),
		offsetof(struct obs_source_audio_timing_pre, timestamp),
		offsetof(struct obs_source_audio_timing_pre, os_time),
		offsetof(struct obs_source_audio_timing_pre, duration),
		offsetof(struct obs_source_audio_timing_pre, next_audio_ts_min),
		offsetof(struct obs_source_audio_timing_pre, timing_adjust),
		offsetof(struct obs_source_audio_timing_pre, diff),
		offsetof(struct obs_source_audio_timing_pre, drift_error),
		offsetof(struct obs_source_audio_timing_pre, action),
		offsetof(struct obs_source_audio_timing_pre, timing_set),
		offsetof(struct obs_source_audio_timing_pre, async_unbuffered),
		offsetof(struct obs_source_audio_timing_pre, async_decoupled),
		offsetof(struct obs_source_audio_timing_pre, direct),
		offsetof(struct obs_source_audio_timing_pre, reanchor),
		offsetof(struct obs_source_audio_timing_pre, continuous),
		sizeof(struct obs_source_audio_timing_under),
		_Alignof(struct obs_source_audio_timing_under),
		offsetof(struct obs_source_audio_timing_under, timestamp),
		offsetof(struct obs_source_audio_timing_under, packet_timing_adjust),
		offsetof(struct obs_source_audio_timing_under, data_timestamp),
		offsetof(struct obs_source_audio_timing_under, os_time),
		offsetof(struct obs_source_audio_timing_under, next_audio_ts_min),
		offsetof(struct obs_source_audio_timing_under, resample_offset),
		offsetof(struct obs_source_audio_timing_under, sync_offset),
		offsetof(struct obs_source_audio_timing_under, timing_adjust),
		offsetof(struct obs_source_audio_timing_under, next_audio_sys_ts_min),
		offsetof(struct obs_source_audio_timing_under, last_sync_offset),
		offsetof(struct obs_source_audio_timing_under, continuous),
		offsetof(struct obs_source_audio_timing_under, push_back),
		offsetof(struct obs_source_audio_timing_under, reset_timing),
		sizeof(enum obs_source_audio_ts_action),
		OBS_SOURCE_AUDIO_TS_KEEP,
		OBS_SOURCE_AUDIO_TS_RESET,
		OBS_SOURCE_AUDIO_TS_JUMP,
		OBS_SOURCE_AUDIO_TS_SMOOTH,
		OBS_SOURCE_AUDIO_TS_EXCEEDED,
		(size_t)OBS_SOURCE_AUDIO_TS_SMOOTHING_THRESHOLD,
		(size_t)OBS_SOURCE_AUDIO_MAX_TS_VAR,
	};
	return which < ORACLE_SOURCE_AUDIO_TIMING_LAYOUT_COUNT ? values[which] : 0;
}
