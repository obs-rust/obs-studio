/* Test-only oracle: the original libobs/obs-encoder-size.c.h, unmodified,
 * with its functions renamed to oracle_* so they can link next to the Rust
 * shims, plus the constants and the layouts of its structs as the C sees
 * them. */
#include <stddef.h>

#define obs_encoder_size_audio_defaults oracle_obs_encoder_size_audio_defaults
#define obs_encoder_size_cap_override oracle_obs_encoder_size_cap_override
#define obs_encoder_size_scaled_gate oracle_obs_encoder_size_scaled_gate
#define obs_encoder_size_scaled_match oracle_obs_encoder_size_scaled_match
#define obs_encoder_size_scaling oracle_obs_encoder_size_scaling
#define obs_encoder_size_video_route oracle_obs_encoder_size_video_route
#define obs_encoder_size_rate_route oracle_obs_encoder_size_rate_route

#include "obs-encoder-size.c.h"

#undef obs_encoder_size_rate_route
#undef obs_encoder_size_video_route
#undef obs_encoder_size_scaling
#undef obs_encoder_size_scaled_match
#undef obs_encoder_size_scaled_gate
#undef obs_encoder_size_cap_override
#undef obs_encoder_size_audio_defaults

/* In the order of rust/obs-c-oracle/src/encoder_size.rs. */
enum {
	ORACLE_ENCODER_SIZE_CONSTANT_COUNT = 15,
	ORACLE_ENCODER_SIZE_LAYOUT_COUNT = 11,
};

long long oracle_encoder_size_constant(size_t which);
size_t oracle_encoder_size_layout(size_t which);

long long oracle_encoder_size_constant(size_t which)
{
	const long long values[ORACLE_ENCODER_SIZE_CONSTANT_COUNT] = {
		OBS_ENCODER_SIZE_TYPE_AUDIO,
		OBS_ENCODER_SIZE_TYPE_VIDEO,
		OBS_ENCODER_SIZE_CAP_SCALING,
		OBS_ENCODER_SIZE_AUDIO_FORMAT_UNKNOWN,
		OBS_ENCODER_SIZE_SPEAKERS_UNKNOWN,
		OBS_ENCODER_SIZE_WRONG_TYPE,
		OBS_ENCODER_SIZE_NO_MEDIA,
		OBS_ENCODER_SIZE_STORED,
		OBS_ENCODER_SIZE_MEDIA,
		OBS_ENCODER_SCALED_NOT_VIDEO,
		OBS_ENCODER_SCALED_ACTIVE,
		OBS_ENCODER_SCALED_INITIALIZED,
		OBS_ENCODER_SCALED_CHECK,
		OBS_ENCODER_SCALED_DISABLE,
		OBS_ENCODER_SCALED_SET,
	};
	return which < ORACLE_ENCODER_SIZE_CONSTANT_COUNT ? values[which] : -1;
}

size_t oracle_encoder_size_layout(size_t which)
{
	const size_t values[ORACLE_ENCODER_SIZE_LAYOUT_COUNT] = {
		sizeof(struct obs_encoder_audio_fill),
		_Alignof(struct obs_encoder_audio_fill),
		offsetof(struct obs_encoder_audio_fill, format),
		offsetof(struct obs_encoder_audio_fill, samples_per_sec),
		offsetof(struct obs_encoder_audio_fill, speakers),
		sizeof(struct obs_encoder_size),
		_Alignof(struct obs_encoder_size),
		offsetof(struct obs_encoder_size, width),
		offsetof(struct obs_encoder_size, height),
		sizeof(enum obs_encoder_size_source),
		sizeof(enum obs_encoder_scaled_action),
	};
	return which < ORACLE_ENCODER_SIZE_LAYOUT_COUNT ? values[which] : (size_t)-1;
}
