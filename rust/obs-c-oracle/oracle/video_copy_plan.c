/* Test-only oracle: the original libobs/obs-video-copy-plan.c.h, unmodified,
 * with its five functions renamed to oracle_* so they can link next to the
 * Rust shims, plus the constants and the layouts of its structs as the C sees
 * them. */
#include <stddef.h>
#include <stdint.h>

#define obs_video_gpu_copy_plan oracle_obs_video_gpu_copy_plan
#define obs_video_rgbx_copy_plan oracle_obs_video_rgbx_copy_plan
#define obs_video_scale_resolution_close oracle_obs_video_scale_resolution_close
#define obs_video_scale_effect_choice oracle_obs_video_scale_effect_choice
#define obs_video_mix_reuse_match oracle_obs_video_mix_reuse_match

#include "obs-video-copy-plan.c.h"

#undef obs_video_mix_reuse_match
#undef obs_video_scale_effect_choice
#undef obs_video_scale_resolution_close
#undef obs_video_rgbx_copy_plan
#undef obs_video_gpu_copy_plan

/* In the order of rust/obs-c-oracle/src/video_copy_plan.rs. */
enum {
	ORACLE_VIDEO_COPY_PLAN_CONSTANT_COUNT = 16,
	ORACLE_VIDEO_COPY_PLAN_LAYOUT_COUNT = 31,
};

long long oracle_video_copy_plan_constant(size_t which);
size_t oracle_video_copy_plan_layout(size_t which);

long long oracle_video_copy_plan_constant(size_t which)
{
	const long long values[ORACLE_VIDEO_COPY_PLAN_CONSTANT_COUNT] = {
		OBS_VIDEO_COPY_MAX,
		OBS_VIDEO_COPY_SCALE_DISABLE,
		OBS_VIDEO_COPY_SCALE_POINT,
		OBS_VIDEO_COPY_SCALE_BICUBIC,
		OBS_VIDEO_COPY_SCALE_BILINEAR,
		OBS_VIDEO_COPY_SCALE_LANCZOS,
		OBS_VIDEO_COPY_SCALE_AREA,
		OBS_VIDEO_SCALE_EFFECT_DEFAULT,
		OBS_VIDEO_SCALE_EFFECT_BILINEAR_LOWRES,
		OBS_VIDEO_SCALE_EFFECT_BICUBIC,
		OBS_VIDEO_SCALE_EFFECT_LANCZOS,
		OBS_VIDEO_SCALE_EFFECT_AREA,
		OBS_VIDEO_SCALE_LOADED(OBS_VIDEO_SCALE_EFFECT_DEFAULT),
		OBS_VIDEO_SCALE_LOADED(OBS_VIDEO_SCALE_EFFECT_AREA),
		MAX_AV_PLANES,
		VIDEO_FORMAT_R10L,
	};
	return which < ORACLE_VIDEO_COPY_PLAN_CONSTANT_COUNT ? values[which] : -1;
}

size_t oracle_video_copy_plan_layout(size_t which)
{
	const size_t values[ORACLE_VIDEO_COPY_PLAN_LAYOUT_COUNT] = {
		sizeof(struct obs_video_copy),
		_Alignof(struct obs_video_copy),
		offsetof(struct obs_video_copy, src_after_plane0),
		offsetof(struct obs_video_copy, whole),
		offsetof(struct obs_video_copy, bytes),
		offsetof(struct obs_video_copy, rows),
		offsetof(struct obs_video_copy, src_stride),
		offsetof(struct obs_video_copy, dst_stride),
		sizeof(struct obs_video_copy_plan),
		_Alignof(struct obs_video_copy_plan),
		offsetof(struct obs_video_copy_plan, count),
		offsetof(struct obs_video_copy_plan, copies),
		sizeof(struct obs_video_scale_input),
		_Alignof(struct obs_video_scale_input),
		offsetof(struct obs_video_scale_input, base_width),
		offsetof(struct obs_video_scale_input, base_height),
		offsetof(struct obs_video_scale_input, target_width),
		offsetof(struct obs_video_scale_input, target_height),
		offsetof(struct obs_video_scale_input, output_width),
		offsetof(struct obs_video_scale_input, output_height),
		offsetof(struct obs_video_scale_input, scale_type),
		offsetof(struct obs_video_scale_input, loaded),
		sizeof(struct obs_video_mix_reuse_key),
		_Alignof(struct obs_video_mix_reuse_key),
		offsetof(struct obs_video_mix_reuse_key, view),
		offsetof(struct obs_video_mix_reuse_key, render_space),
		offsetof(struct obs_video_mix_reuse_key, base_width),
		offsetof(struct obs_video_mix_reuse_key, base_height),
		offsetof(struct obs_video_mix_reuse_key, rendered),
		sizeof(enum obs_video_scale_effect),
		sizeof(enum video_format),
	};
	return which < ORACLE_VIDEO_COPY_PLAN_LAYOUT_COUNT ? values[which] : (size_t)-1;
}
