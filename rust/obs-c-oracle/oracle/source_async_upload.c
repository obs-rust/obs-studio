/* Test-only oracle: the original libobs/obs-source-async-upload.c.h,
 * unmodified, with its fourteen functions renamed to oracle_* so they can
 * link next to the Rust shims. */
#include <stddef.h>
#include <stdint.h>

#define obs_source_async_upload_texture_changed oracle_obs_source_async_upload_texture_changed
#define obs_source_async_upload_tries_gpu oracle_obs_source_async_upload_tries_gpu
#define obs_source_async_upload_size_uniforms oracle_obs_source_async_upload_size_uniforms
#define obs_source_async_upload_hlg_peak oracle_obs_source_async_upload_hlg_peak
#define obs_source_async_upload_maximum_nits oracle_obs_source_async_upload_maximum_nits
#define obs_source_async_upload_over_sdr_white oracle_obs_source_async_upload_over_sdr_white
#define obs_source_async_upload_hdr_lw oracle_obs_source_async_upload_hdr_lw
#define obs_source_async_upload_sets_range oracle_obs_source_async_upload_sets_range
#define obs_source_async_upload_linear_alpha oracle_obs_source_async_upload_linear_alpha
#define obs_source_async_upload_uses_texrender oracle_obs_source_async_upload_uses_texrender
#define obs_source_async_upload_plain oracle_obs_source_async_upload_plain
#define obs_source_async_upload_bgr_swap oracle_obs_source_async_upload_bgr_swap
#define obs_source_async_upload_reanchor_reads_unbuffered oracle_obs_source_async_upload_reanchor_reads_unbuffered
#define obs_source_async_upload_reanchor oracle_obs_source_async_upload_reanchor

#include "obs-source-async-upload.c.h"

#undef obs_source_async_upload_reanchor
#undef obs_source_async_upload_reanchor_reads_unbuffered
#undef obs_source_async_upload_bgr_swap
#undef obs_source_async_upload_plain
#undef obs_source_async_upload_uses_texrender
#undef obs_source_async_upload_linear_alpha
#undef obs_source_async_upload_sets_range
#undef obs_source_async_upload_hdr_lw
#undef obs_source_async_upload_over_sdr_white
#undef obs_source_async_upload_maximum_nits
#undef obs_source_async_upload_hlg_peak
#undef obs_source_async_upload_size_uniforms
#undef obs_source_async_upload_tries_gpu
#undef obs_source_async_upload_texture_changed

/* The layouts and values the C uses, in the order of
 * rust/obs-c-oracle/src/source_async_upload.rs; INT64_MIN past the end. */
int64_t oracle_source_async_upload_value(size_t which);

int64_t oracle_source_async_upload_value(size_t which)
{
	const int64_t values[] = {
		sizeof(struct obs_source_async_upload_format),
		_Alignof(struct obs_source_async_upload_format),
		offsetof(struct obs_source_async_upload_format, width),
		offsetof(struct obs_source_async_upload_format, height),
		offsetof(struct obs_source_async_upload_format, format),
		offsetof(struct obs_source_async_upload_format, full_range),
		offsetof(struct obs_source_async_upload_format, trc),
		sizeof(struct obs_source_async_upload_size),
		_Alignof(struct obs_source_async_upload_size),
		offsetof(struct obs_source_async_upload_size, width),
		offsetof(struct obs_source_async_upload_size, height),
		offsetof(struct obs_source_async_upload_size, width_d2),
		offsetof(struct obs_source_async_upload_size, height_d2),
		offsetof(struct obs_source_async_upload_size, width_x2_i),
		offsetof(struct obs_source_async_upload_size, height_x2_i),
		GS_UNKNOWN,
		GS_BGRX,
		GS_BGRA,
		VIDEO_FORMAT_BGRA,
		VIDEO_FORMAT_BGRX,
		VIDEO_TRC_HLG,
		OBS_SOURCE_FRAME_LINEAR_ALPHA,
		OBS_SOURCE_CONVERT_NONE,
		sizeof(enum gs_color_format),
		sizeof(enum video_format),
		sizeof(bool),
		sizeof(float),
	};
	return which < sizeof(values) / sizeof(values[0]) ? values[which] : INT64_MIN;
}
