/* Test-only oracle: the original libobs/obs-source-conversion-plan.c.h,
 * unmodified, with its function renamed to oracle_* so it can link next to
 * the Rust shim. */
#include <stddef.h>
#include <stdint.h>

#define obs_source_conversion_plan_fill oracle_obs_source_conversion_plan_fill

#include "obs-source-conversion-plan.c.h"

#undef obs_source_conversion_plan_fill

/* The layout of struct obs_source_conversion_plan, in the order of
 * rust/obs-c-oracle/src/source_conversion_plan.rs; SIZE_MAX past the end. */
size_t oracle_source_conversion_plan_layout(size_t which);

#define PLAN_OFFSET(m) offsetof(struct obs_source_conversion_plan, m)

size_t oracle_source_conversion_plan_layout(size_t which)
{
	const size_t values[] = {
		sizeof(struct obs_source_conversion_plan),
		_Alignof(struct obs_source_conversion_plan),
		PLAN_OFFSET(convert_type),
		PLAN_OFFSET(channel_count),
		PLAN_OFFSET(width),
		PLAN_OFFSET(height),
		PLAN_OFFSET(format),
		PLAN_OFFSET(technique),
		PLAN_OFFSET(linear),
		OBS_SOURCE_CONVERSION_PLAN_PLANES,
		sizeof(enum obs_source_convert_type),
		sizeof(enum video_format),
		sizeof(enum gs_color_format),
		sizeof(int),
		sizeof(bool),
	};
	return which < sizeof(values) / sizeof(values[0]) ? values[which] : SIZE_MAX;
}

#undef PLAN_OFFSET

/* The enum values the plan uses, in the order of
 * rust/obs-c-oracle/src/source_conversion_plan.rs; INT64_MIN past the end. */
int64_t oracle_source_conversion_plan_value(size_t which);

int64_t oracle_source_conversion_plan_value(size_t which)
{
	const int64_t values[] = {
		/* enum video_format */
		VIDEO_FORMAT_NONE,
		VIDEO_FORMAT_I420,
		VIDEO_FORMAT_NV12,
		VIDEO_FORMAT_YVYU,
		VIDEO_FORMAT_YUY2,
		VIDEO_FORMAT_UYVY,
		VIDEO_FORMAT_RGBA,
		VIDEO_FORMAT_BGRA,
		VIDEO_FORMAT_BGRX,
		VIDEO_FORMAT_Y800,
		VIDEO_FORMAT_I444,
		VIDEO_FORMAT_BGR3,
		VIDEO_FORMAT_I422,
		VIDEO_FORMAT_I40A,
		VIDEO_FORMAT_I42A,
		VIDEO_FORMAT_YUVA,
		VIDEO_FORMAT_AYUV,
		VIDEO_FORMAT_I010,
		VIDEO_FORMAT_P010,
		VIDEO_FORMAT_I210,
		VIDEO_FORMAT_I412,
		VIDEO_FORMAT_YA2L,
		VIDEO_FORMAT_P216,
		VIDEO_FORMAT_P416,
		VIDEO_FORMAT_V210,
		VIDEO_FORMAT_R10L,
		/* enum video_trc */
		VIDEO_TRC_DEFAULT,
		VIDEO_TRC_SRGB,
		VIDEO_TRC_PQ,
		VIDEO_TRC_HLG,
		/* enum gs_color_format */
		GS_UNKNOWN,
		GS_R8,
		GS_RGBA,
		GS_BGRX,
		GS_BGRA,
		GS_R10G10B10A2,
		GS_R16,
		GS_RGBA16F,
		GS_R8G8,
		GS_BGRA_UNORM,
		GS_RG16,
		/* enum obs_source_convert_type */
		OBS_SOURCE_CONVERT_NONE,
		OBS_SOURCE_CONVERT_NV12,
		OBS_SOURCE_CONVERT_420,
		OBS_SOURCE_CONVERT_420_PQ,
		OBS_SOURCE_CONVERT_420_A,
		OBS_SOURCE_CONVERT_422,
		OBS_SOURCE_CONVERT_422P10LE,
		OBS_SOURCE_CONVERT_422_A,
		OBS_SOURCE_CONVERT_422_PACK,
		OBS_SOURCE_CONVERT_444,
		OBS_SOURCE_CONVERT_444P12LE,
		OBS_SOURCE_CONVERT_444_A,
		OBS_SOURCE_CONVERT_444P12LE_A,
		OBS_SOURCE_CONVERT_444_A_PACK,
		OBS_SOURCE_CONVERT_800,
		OBS_SOURCE_CONVERT_RGB_LIMITED,
		OBS_SOURCE_CONVERT_BGR3,
		OBS_SOURCE_CONVERT_I010,
		OBS_SOURCE_CONVERT_P010,
		OBS_SOURCE_CONVERT_V210,
		OBS_SOURCE_CONVERT_R10L,
	};
	return which < sizeof(values) / sizeof(values[0]) ? values[which] : INT64_MIN;
}
