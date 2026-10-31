/* Test-only oracle: the original libobs/media-io/video-scaler-plan.c.h,
 * unmodified, with its five functions renamed to oracle_* so they can link
 * next to the Rust shims, plus its constants and the enum values it maps as
 * the C sees them. It calls nothing outside the file. */
#include <stddef.h>
#include <stdint.h>

#define video_scaler_sws_flags oracle_video_scaler_sws_flags
#define video_scaler_sws_colorspace oracle_video_scaler_sws_colorspace
#define video_scaler_sws_range oracle_video_scaler_sws_range
#define video_scaler_plane_heights oracle_video_scaler_plane_heights
#define video_scaler_plane_copy oracle_video_scaler_plane_copy

#include "media-io/video-scaler-plan.c.h"

/* The integer values the C uses, in the order of
 * rust/obs-c-oracle/src/video_scaler_plan.rs. */
enum {
	ORACLE_VIDEO_SCALER_PLAN_INT_COUNT = 22,
};

int64_t oracle_video_scaler_plan_int(size_t which);

int64_t oracle_video_scaler_plan_int(size_t which)
{
	const int64_t values[ORACLE_VIDEO_SCALER_PLAN_INT_COUNT] = {
		VIDEO_SCALER_SWS_FAST_BILINEAR,
		VIDEO_SCALER_SWS_BILINEAR,
		VIDEO_SCALER_SWS_BICUBIC,
		VIDEO_SCALER_SWS_POINT,
		VIDEO_SCALER_SWS_AREA,
		VIDEO_SCALER_SWS_CS_ITU709,
		VIDEO_SCALER_SWS_CS_ITU601,
		VIDEO_SCALER_SWS_CS_BT2020,
		VIDEO_SCALE_DEFAULT,
		VIDEO_SCALE_POINT,
		VIDEO_SCALE_FAST_BILINEAR,
		VIDEO_SCALE_BILINEAR,
		VIDEO_SCALE_BICUBIC,
		VIDEO_CS_DEFAULT,
		VIDEO_CS_601,
		VIDEO_CS_709,
		VIDEO_CS_SRGB,
		VIDEO_CS_2100_PQ,
		VIDEO_CS_2100_HLG,
		VIDEO_RANGE_DEFAULT,
		VIDEO_RANGE_PARTIAL,
		VIDEO_RANGE_FULL,
	};
	return which < ORACLE_VIDEO_SCALER_PLAN_INT_COUNT ? values[which] : -1;
}
