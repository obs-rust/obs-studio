/* Test-only oracle: the original libobs/obs-encoder-gpu-rescale.c.h,
 * unmodified, with its functions renamed to oracle_* so they can link next to
 * the Rust shims, plus the constants and the layouts of its structs as the C
 * sees them. */
#include <stddef.h>

#define obs_encoder_gpu_rescale_wanted oracle_obs_encoder_gpu_rescale_wanted
#define obs_encoder_gpu_rescale_target oracle_obs_encoder_gpu_rescale_target
#define obs_encoder_gpu_rescale_matches oracle_obs_encoder_gpu_rescale_matches
#define obs_encoder_gpu_rescale_release_step oracle_obs_encoder_gpu_rescale_release_step
#define obs_encoder_gpu_rescale_release_refs oracle_obs_encoder_gpu_rescale_release_refs

#include "obs-encoder-gpu-rescale.c.h"

#undef obs_encoder_gpu_rescale_release_refs
#undef obs_encoder_gpu_rescale_release_step
#undef obs_encoder_gpu_rescale_matches
#undef obs_encoder_gpu_rescale_target
#undef obs_encoder_gpu_rescale_wanted

/* In the order of rust/obs-c-oracle/src/encoder_gpu_rescale.rs. */
enum {
	ORACLE_ENCODER_GPU_RESCALE_CONSTANT_COUNT = 7,
	ORACLE_ENCODER_GPU_RESCALE_LAYOUT_COUNT = 16,
};

long long oracle_encoder_gpu_rescale_constant(size_t which);
size_t oracle_encoder_gpu_rescale_layout(size_t which);

long long oracle_encoder_gpu_rescale_constant(size_t which)
{
	const long long values[ORACLE_ENCODER_GPU_RESCALE_CONSTANT_COUNT] = {
		OBS_ENCODER_GPU_RESCALE_SCALE_DISABLE, OBS_ENCODER_GPU_RESCALE_FORMAT_NONE,
		OBS_ENCODER_GPU_RESCALE_CS_DEFAULT,    OBS_ENCODER_GPU_RESCALE_RANGE_DEFAULT,
		OBS_ENCODER_GPU_RESCALE_RELEASE_SKIP,  OBS_ENCODER_GPU_RESCALE_RELEASE_STOP,
		OBS_ENCODER_GPU_RESCALE_RELEASE_DROP,
	};
	return which < ORACLE_ENCODER_GPU_RESCALE_CONSTANT_COUNT ? values[which] : -1;
}

size_t oracle_encoder_gpu_rescale_layout(size_t which)
{
	const size_t values[ORACLE_ENCODER_GPU_RESCALE_LAYOUT_COUNT] = {
		sizeof(struct obs_encoder_gpu_rescale_request),
		_Alignof(struct obs_encoder_gpu_rescale_request),
		offsetof(struct obs_encoder_gpu_rescale_request, scale_type),
		offsetof(struct obs_encoder_gpu_rescale_request, scaled_width),
		offsetof(struct obs_encoder_gpu_rescale_request, scaled_height),
		offsetof(struct obs_encoder_gpu_rescale_request, format),
		offsetof(struct obs_encoder_gpu_rescale_request, colorspace),
		offsetof(struct obs_encoder_gpu_rescale_request, range),
		sizeof(struct obs_encoder_gpu_rescale_output),
		_Alignof(struct obs_encoder_gpu_rescale_output),
		offsetof(struct obs_encoder_gpu_rescale_output, width),
		offsetof(struct obs_encoder_gpu_rescale_output, height),
		offsetof(struct obs_encoder_gpu_rescale_output, format),
		offsetof(struct obs_encoder_gpu_rescale_output, colorspace),
		offsetof(struct obs_encoder_gpu_rescale_output, range),
		sizeof(enum obs_encoder_gpu_rescale_release),
	};
	return which < ORACLE_ENCODER_GPU_RESCALE_LAYOUT_COUNT ? values[which] : (size_t)-1;
}
