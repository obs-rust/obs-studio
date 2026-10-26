/* Test-only oracle: the original libobs/obs-encoder-bind.c.h, unmodified,
 * with its functions renamed to oracle_* so they can link next to the Rust
 * shims, plus its constant and the layout of its struct as the C sees them.
 * Nothing in it divides, so no input traps on any host. */
#include <stddef.h>

#define obs_encoder_bind_not_audio oracle_obs_encoder_bind_not_audio
#define obs_encoder_bind_video_timebase oracle_obs_encoder_bind_video_timebase
#define obs_encoder_bind_audio_timebase oracle_obs_encoder_bind_audio_timebase
#define obs_encoder_bind_divisor_override oracle_obs_encoder_bind_divisor_override
#define obs_encoder_bind_locked oracle_obs_encoder_bind_locked
#define obs_encoder_bind_shutdown_unstarted oracle_obs_encoder_bind_shutdown_unstarted

#include "obs-encoder-bind.c.h"

#undef obs_encoder_bind_shutdown_unstarted
#undef obs_encoder_bind_locked
#undef obs_encoder_bind_divisor_override
#undef obs_encoder_bind_audio_timebase
#undef obs_encoder_bind_video_timebase
#undef obs_encoder_bind_not_audio

/* In the order of rust/obs-c-oracle/src/encoder_bind.rs. */
enum {
	ORACLE_ENCODER_BIND_CONSTANT_COUNT = 1,
	ORACLE_ENCODER_BIND_LAYOUT_COUNT = 4,
};

long long oracle_encoder_bind_constant(size_t which);
size_t oracle_encoder_bind_layout(size_t which);

long long oracle_encoder_bind_constant(size_t which)
{
	const long long values[ORACLE_ENCODER_BIND_CONSTANT_COUNT] = {
		OBS_ENCODER_BIND_TYPE_AUDIO,
	};
	return which < ORACLE_ENCODER_BIND_CONSTANT_COUNT ? values[which] : -1;
}

size_t oracle_encoder_bind_layout(size_t which)
{
	const size_t values[ORACLE_ENCODER_BIND_LAYOUT_COUNT] = {
		sizeof(struct obs_encoder_bind_timebase),
		_Alignof(struct obs_encoder_bind_timebase),
		offsetof(struct obs_encoder_bind_timebase, num),
		offsetof(struct obs_encoder_bind_timebase, den),
	};
	return which < ORACLE_ENCODER_BIND_LAYOUT_COUNT ? values[which] : (size_t)-1;
}
