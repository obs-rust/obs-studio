/* Test-only oracle: the original libobs/obs-encoder-gpu-gates.c.h,
 * unmodified, with its functions renamed to oracle_* so they can link next
 * to the Rust shims, plus its constants as the C sees them. Nothing in it
 * divides, so no input traps on any host. obs_encoder_gpu_gates_warm_next()
 * and _count_next() subtract 1 from an int: callers keep it above INT_MIN,
 * where the C is defined. */
#include <stddef.h>

#define obs_encoder_gpu_gates_warming oracle_obs_encoder_gpu_gates_warming
#define obs_encoder_gpu_gates_warm_next oracle_obs_encoder_gpu_gates_warm_next
#define obs_encoder_gpu_gates_divisor_inc oracle_obs_encoder_gpu_gates_divisor_inc
#define obs_encoder_gpu_gates_divisor_wrap oracle_obs_encoder_gpu_gates_divisor_wrap
#define obs_encoder_gpu_gates_divisor_skip oracle_obs_encoder_gpu_gates_divisor_skip
#define obs_encoder_gpu_gates_next_key oracle_obs_encoder_gpu_gates_next_key
#define obs_encoder_gpu_gates_record_timing oracle_obs_encoder_gpu_gates_record_timing
#define obs_encoder_gpu_gates_count_next oracle_obs_encoder_gpu_gates_count_next
#define obs_encoder_gpu_gates_requeue oracle_obs_encoder_gpu_gates_requeue
#define obs_encoder_gpu_gates_requeue_ts oracle_obs_encoder_gpu_gates_requeue_ts
#define obs_encoder_gpu_gates_p010 oracle_obs_encoder_gpu_gates_p010

#include "obs-encoder-gpu-gates.c.h"

#undef obs_encoder_gpu_gates_p010
#undef obs_encoder_gpu_gates_requeue_ts
#undef obs_encoder_gpu_gates_requeue
#undef obs_encoder_gpu_gates_count_next
#undef obs_encoder_gpu_gates_record_timing
#undef obs_encoder_gpu_gates_next_key
#undef obs_encoder_gpu_gates_divisor_skip
#undef obs_encoder_gpu_gates_divisor_wrap
#undef obs_encoder_gpu_gates_divisor_inc
#undef obs_encoder_gpu_gates_warm_next
#undef obs_encoder_gpu_gates_warming

/* In the order of rust/obs-c-oracle/src/encoder_gpu_gates.rs. */
enum {
	ORACLE_ENCODER_GPU_GATES_CONSTANT_COUNT = 2,
};

long long oracle_encoder_gpu_gates_constant(size_t which);

long long oracle_encoder_gpu_gates_constant(size_t which)
{
	const long long values[ORACLE_ENCODER_GPU_GATES_CONSTANT_COUNT] = {
		OBS_ENCODER_GPU_GATES_WAIT_FRAMES,
		OBS_ENCODER_GPU_GATES_FORMAT_P010,
	};
	return which < ORACLE_ENCODER_GPU_GATES_CONSTANT_COUNT ? values[which] : -1;
}
