/* Test-only oracle for libobs/media-io/video-io.c. The exported names are
 * renamed oracle_* around the include so they do not collide with the Rust
 * shims. profile_store_name and obs_get_profiler_name_store stay public
 * after the include: weak on Unix, where the audio oracle's strong copies
 * win, and strong on Windows, where the audio oracle is not built.
 *
 * video_scaler_* is one stub for this file and for the Rust shim. libobs
 * still calls the real FFmpeg scaler; this stand-in is cargo-test only.
 */
#if !defined(_WIN32)
#define _GNU_SOURCE
#endif

#include <stdarg.h>
#include <stddef.h>
#include <stdint.h>

#define profile_start oracle_vio_profile_start
#define profile_end oracle_vio_profile_end
#define profile_reenable_thread oracle_vio_profile_reenable_thread
#define profile_store_name oracle_vio_profile_store_name
#define obs_get_profiler_name_store oracle_vio_obs_get_profiler_name_store

#define video_output_open oracle_video_output_open
#define video_output_close oracle_video_output_close
#define video_output_connect oracle_video_output_connect
#define video_output_connect2 oracle_video_output_connect2
#define video_output_disconnect oracle_video_output_disconnect
#define video_output_disconnect2 oracle_video_output_disconnect2
#define video_output_align_frame_rate_divisor oracle_video_output_align_frame_rate_divisor
#define video_output_active oracle_video_output_active
#define video_output_get_info oracle_video_output_get_info
#define video_output_lock_frame oracle_video_output_lock_frame
#define video_output_unlock_frame oracle_video_output_unlock_frame
#define video_output_get_frame_time oracle_video_output_get_frame_time
#define video_output_stop oracle_video_output_stop
#define video_output_stopped oracle_video_output_stopped
#define video_output_get_format oracle_video_output_get_format
#define video_output_get_width oracle_video_output_get_width
#define video_output_get_height oracle_video_output_get_height
#define video_output_get_frame_rate oracle_video_output_get_frame_rate
#define video_output_get_skipped_frames oracle_video_output_get_skipped_frames
#define video_output_get_total_frames oracle_video_output_get_total_frames
#define video_output_inc_texture_encoders oracle_video_output_inc_texture_encoders
#define video_output_dec_texture_encoders oracle_video_output_dec_texture_encoders
#define video_output_inc_texture_frames oracle_video_output_inc_texture_frames
#define video_output_inc_texture_skipped_frames oracle_video_output_inc_texture_skipped_frames
#define video_output_create_with_frame_rate_divisor oracle_video_output_create_with_frame_rate_divisor
#define video_output_free_frame_rate_divisor oracle_video_output_free_frame_rate_divisor

#include "media-io/video-io.c.h"

#undef profile_start
#undef profile_end
#undef profile_reenable_thread
#undef profile_store_name
#undef obs_get_profiler_name_store

void oracle_vio_profile_start(const char *name)
{
	(void)name;
}

void oracle_vio_profile_end(const char *name)
{
	(void)name;
}

void oracle_vio_profile_reenable_thread(void) {}

const char *oracle_vio_profile_store_name(profiler_name_store_t *store, const char *format, ...)
{
	(void)store;
	(void)format;
	return NULL;
}

profiler_name_store_t *oracle_vio_obs_get_profiler_name_store(void)
{
	return NULL;
}

/* Strong on every Windows target: the audio oracle (the Unix copy) is not
 * built there, and MinGW drops weak-only definitions from static archives
 * (#291). */
#if defined(_WIN32)
#define VIO_STUB
#else
#define VIO_STUB __attribute__((weak))
#endif

VIO_STUB
/* Test-only exports called from Rust over FFI. */
profiler_name_store_t *obs_get_profiler_name_store(void);
const char *profile_store_name(profiler_name_store_t *store, const char *format, ...);

profiler_name_store_t *obs_get_profiler_name_store(void)
{
	return NULL;
}

VIO_STUB
const char *profile_store_name(profiler_name_store_t *store, const char *format, ...)
{
	(void)store;
	(void)format;
	return NULL;
}

struct video_scaler {
	int live;
};

static bool vio_scaler_format_ok(enum video_format format)
{
	switch (format) {
	case VIDEO_FORMAT_I420:
	case VIDEO_FORMAT_NV12:
	case VIDEO_FORMAT_YUY2:
	case VIDEO_FORMAT_UYVY:
	case VIDEO_FORMAT_YVYU:
	case VIDEO_FORMAT_RGBA:
	case VIDEO_FORMAT_BGRA:
	case VIDEO_FORMAT_BGRX:
	case VIDEO_FORMAT_Y800:
	case VIDEO_FORMAT_I444:
	case VIDEO_FORMAT_I412:
	case VIDEO_FORMAT_BGR3:
	case VIDEO_FORMAT_I422:
	case VIDEO_FORMAT_I210:
	case VIDEO_FORMAT_I40A:
	case VIDEO_FORMAT_I42A:
	case VIDEO_FORMAT_YUVA:
	case VIDEO_FORMAT_YA2L:
	case VIDEO_FORMAT_I010:
	case VIDEO_FORMAT_P010:
	case VIDEO_FORMAT_P216:
	case VIDEO_FORMAT_P416:
		return true;
	default:
		return false;
	}
}

int video_scaler_create(video_scaler_t **scaler, const struct video_scale_info *dst, const struct video_scale_info *src,
			enum video_scale_type type)
{
	(void)type;
	if (!scaler)
		return VIDEO_SCALER_FAILED;
	*scaler = NULL;
	if (!dst || !src || !vio_scaler_format_ok(dst->format) || !vio_scaler_format_ok(src->format))
		return VIDEO_SCALER_BAD_CONVERSION;
	*scaler = bzalloc(sizeof(struct video_scaler));
	if (!*scaler)
		return VIDEO_SCALER_FAILED;
	return VIDEO_SCALER_SUCCESS;
}

void video_scaler_destroy(video_scaler_t *scaler)
{
	bfree(scaler);
}

bool video_scaler_scale(video_scaler_t *scaler, uint8_t *output[], const uint32_t out_linesize[],
			const uint8_t *const input[], const uint32_t in_linesize[])
{
	(void)out_linesize;
	(void)in_linesize;
	if (!scaler)
		return false;
	if (output && output[0] && input && input[0])
		output[0][0] = input[0][0];
	return true;
}
