/* Test-only oracle: the original libobs/obs-video-output-step.c.h,
 * unmodified, with its ten functions renamed to oracle_* so they can link
 * next to the Rust shims, plus the constants and the layouts of its structs
 * as the C sees them. Five struct tags share their function's name, so the
 * renames rename them too: the layout table names the renamed tags. */
#include <stddef.h>
#include <stdint.h>

#define obs_video_step_activation oracle_obs_video_step_activation
#define obs_video_step_stage oracle_obs_video_step_stage
#define obs_video_step_queue oracle_obs_video_step_queue
#define obs_video_step_queue_copy_uv oracle_obs_video_step_queue_copy_uv
#define obs_video_step_queue_count oracle_obs_video_step_queue_count
#define obs_video_step_render_targets oracle_obs_video_step_render_targets
#define obs_video_step_render oracle_obs_video_step_render
#define obs_video_step_ring oracle_obs_video_step_ring
#define obs_video_step_download oracle_obs_video_step_download
#define obs_video_step_output oracle_obs_video_step_output

#include "obs-video-output-step.c.h"

#undef obs_video_step_output
#undef obs_video_step_download
#undef obs_video_step_ring
#undef obs_video_step_render
#undef obs_video_step_render_targets
#undef obs_video_step_queue_count
#undef obs_video_step_queue_copy_uv
#undef obs_video_step_queue
#undef obs_video_step_stage
#undef obs_video_step_activation

/* In the order of rust/obs-c-oracle/src/video_output_step.rs. */
enum {
	ORACLE_VIDEO_OUTPUT_STEP_CONSTANT_COUNT = 5,
	ORACLE_VIDEO_OUTPUT_STEP_LAYOUT_COUNT = 35,
};

long long oracle_video_output_step_constant(size_t which);
size_t oracle_video_output_step_layout(size_t which);

long long oracle_video_output_step_constant(size_t which)
{
	const long long values[ORACLE_VIDEO_OUTPUT_STEP_CONSTANT_COUNT] = {
		OBS_VIDEO_STEP_TEXTURES,   OBS_VIDEO_STEP_CHANNELS,   OBS_VIDEO_STEP_QUEUE_DUPLICATE,
		OBS_VIDEO_STEP_QUEUE_COPY, OBS_VIDEO_STEP_QUEUE_SWAP,
	};
	return which < ORACLE_VIDEO_OUTPUT_STEP_CONSTANT_COUNT ? values[which] : -1;
}

size_t oracle_video_output_step_layout(size_t which)
{
	const size_t values[ORACLE_VIDEO_OUTPUT_STEP_LAYOUT_COUNT] = {
		sizeof(struct obs_video_step_activation_input),
		_Alignof(struct obs_video_step_activation_input),
		offsetof(struct obs_video_step_activation_input, raw_was_active),
		offsetof(struct obs_video_step_activation_input, gpu_was_active),
		offsetof(struct obs_video_step_activation_input, was_active),
		offsetof(struct obs_video_step_activation_input, raw_active),
		offsetof(struct obs_video_step_activation_input, gpu_encoder_active),
		sizeof(struct oracle_obs_video_step_activation),
		_Alignof(struct oracle_obs_video_step_activation),
		offsetof(struct oracle_obs_video_step_activation, clear_base),
		offsetof(struct oracle_obs_video_step_activation, clear_raw),
		offsetof(struct oracle_obs_video_step_activation, clear_gpu),
		offsetof(struct oracle_obs_video_step_activation, raw_active),
		offsetof(struct oracle_obs_video_step_activation, gpu_active),
		offsetof(struct oracle_obs_video_step_activation, active),
		sizeof(struct oracle_obs_video_step_stage),
		_Alignof(struct oracle_obs_video_step_stage),
		offsetof(struct oracle_obs_video_step_stage, staged),
		offsetof(struct oracle_obs_video_step_stage, from_output),
		offsetof(struct oracle_obs_video_step_stage, channels),
		sizeof(struct oracle_obs_video_step_render_targets),
		_Alignof(struct oracle_obs_video_step_render_targets),
		offsetof(struct oracle_obs_video_step_render_targets, outputs),
		offsetof(struct oracle_obs_video_step_render_targets, encode),
		sizeof(struct oracle_obs_video_step_render),
		_Alignof(struct oracle_obs_video_step_render),
		offsetof(struct oracle_obs_video_step_render, convert),
		offsetof(struct oracle_obs_video_step_render, encode),
		offsetof(struct oracle_obs_video_step_render, stage),
		sizeof(struct oracle_obs_video_step_ring),
		_Alignof(struct oracle_obs_video_step_ring),
		offsetof(struct oracle_obs_video_step_ring, prev),
		offsetof(struct oracle_obs_video_step_ring, next),
		sizeof(enum obs_video_step_queue_action),
		sizeof(long),
	};
	return which < ORACLE_VIDEO_OUTPUT_STEP_LAYOUT_COUNT ? values[which] : (size_t)-1;
}
