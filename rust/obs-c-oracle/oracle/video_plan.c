/* Test-only oracle: the original libobs/obs-video-plan.c.h, unmodified, with
 * its five functions renamed to oracle_* so they can link next to the Rust
 * shims. The rename also renames the struct tag obs_video_output_plan, in
 * obs-internal.h and here alike, which leaves its layout as it is. */
#include <stddef.h>
#include <stdint.h>

#define obs_video_size_valid oracle_obs_video_size_valid
#define obs_video_align_output_size oracle_obs_video_align_output_size
#define obs_video_output_size_valid oracle_obs_video_output_size_valid
#define obs_video_scale_type_name oracle_obs_video_scale_type_name
#define obs_video_output_plan oracle_obs_video_output_plan

#include "obs-video-plan.c.h"

/* The layout of struct obs_video_plane and struct obs_video_output_plan, in
 * the order of rust/obs-c-oracle/src/video_plan.rs; SIZE_MAX past the end. */
size_t oracle_video_plan_layout(size_t which);

#define PLAN_OFFSET(m) offsetof(struct obs_video_output_plan, m)
#define PLANE_OFFSET(m) offsetof(struct obs_video_plane, m)

size_t oracle_video_plan_layout(size_t which)
{
	const size_t values[] = {
		sizeof(struct obs_video_plane),
		_Alignof(struct obs_video_plane),
		PLANE_OFFSET(width),
		PLANE_OFFSET(height),
		PLANE_OFFSET(format),
		sizeof(struct obs_video_output_plan),
		_Alignof(struct obs_video_output_plan),
		PLAN_OFFSET(conversion_needed),
		PLAN_OFFSET(conversion_techs),
		PLAN_OFFSET(conversion_width_i),
		PLAN_OFFSET(conversion_height_i),
		PLAN_OFFSET(planes),
		PLAN_OFFSET(texture_format),
		PLAN_OFFSET(render_space),
		NUM_CHANNELS,
		sizeof(enum gs_color_format),
		sizeof(enum gs_color_space),
		sizeof(enum video_format),
		sizeof(enum video_colorspace),
		sizeof(enum obs_scale_type),
	};
	return which < sizeof(values) / sizeof(values[0]) ? values[which] : SIZE_MAX;
}

#undef PLANE_OFFSET
#undef PLAN_OFFSET

/* The enum values the plan uses, in the order of
 * rust/obs-c-oracle/src/video_plan.rs; INT64_MIN past the end. */
int64_t oracle_video_plan_value(size_t which);

int64_t oracle_video_plan_value(size_t which)
{
	const int64_t values[] = {
		/* enum gs_color_format */
		GS_UNKNOWN,
		GS_A8,
		GS_R8,
		GS_RGBA,
		GS_BGRX,
		GS_BGRA,
		GS_R10G10B10A2,
		GS_RGBA16,
		GS_R16,
		GS_RGBA16F,
		GS_RGBA32F,
		GS_RG16F,
		GS_RG32F,
		GS_R16F,
		GS_R32F,
		GS_DXT1,
		GS_DXT3,
		GS_DXT5,
		GS_R8G8,
		GS_RGBA_UNORM,
		GS_BGRX_UNORM,
		GS_BGRA_UNORM,
		GS_RG16,
		/* enum gs_color_space */
		GS_CS_SRGB,
		GS_CS_SRGB_16F,
		GS_CS_709_EXTENDED,
		GS_CS_709_SCRGB,
		/* enum obs_scale_type */
		OBS_SCALE_DISABLE,
		OBS_SCALE_POINT,
		OBS_SCALE_BICUBIC,
		OBS_SCALE_BILINEAR,
		OBS_SCALE_LANCZOS,
		OBS_SCALE_AREA,
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
		/* enum video_colorspace */
		VIDEO_CS_DEFAULT,
		VIDEO_CS_601,
		VIDEO_CS_709,
		VIDEO_CS_SRGB,
		VIDEO_CS_2100_PQ,
		VIDEO_CS_2100_HLG,
	};
	return which < sizeof(values) / sizeof(values[0]) ? values[which] : INT64_MIN;
}
