/* Test-only oracle: the original libobs/obs-encoder-audio-start.c.h,
 * unmodified, with its functions renamed to oracle_* so they can link next
 * to the Rust shims, plus the constant and the layout of its struct as the
 * C sees them. Nothing in it divides, so no input traps on any host. */
#include <stddef.h>

/* send_audio_data()'s pts step adds a size_t to an int64_t and converts
 * the sum back, as obs-encoder.c always did. libobs builds with
 * -Wno-sign-conversion; the oracle has the warning. */
#if defined(__clang__) || defined(__GNUC__)
#pragma GCC diagnostic ignored "-Wsign-conversion"
#endif

#define obs_encoder_audio_start_group_ready oracle_obs_encoder_audio_start_group_ready
#define obs_encoder_audio_start_paired_late oracle_obs_encoder_audio_start_paired_late
#define obs_encoder_audio_start_video_pts oracle_obs_encoder_audio_start_video_pts
#define obs_encoder_audio_start_size oracle_obs_encoder_audio_start_size
#define obs_encoder_audio_start_paired oracle_obs_encoder_audio_start_paired
#define obs_encoder_audio_start_unpaired oracle_obs_encoder_audio_start_unpaired
#define obs_encoder_audio_start_no_video oracle_obs_encoder_audio_start_no_video
#define obs_encoder_audio_start_packet oracle_obs_encoder_audio_start_packet
#define obs_encoder_audio_start_offset_ns oracle_obs_encoder_audio_start_offset_ns
#define obs_encoder_audio_start_offset_size oracle_obs_encoder_audio_start_offset_size
#define obs_encoder_audio_start_buffer_offset oracle_obs_encoder_audio_start_buffer_offset
#define obs_encoder_audio_start_push_size oracle_obs_encoder_audio_start_push_size
#define obs_encoder_audio_start_can_send oracle_obs_encoder_audio_start_can_send
#define obs_encoder_audio_start_audio_pts oracle_obs_encoder_audio_start_audio_pts
#define obs_encoder_audio_start_not_video oracle_obs_encoder_audio_start_not_video
#define obs_encoder_audio_start_divisor_ok oracle_obs_encoder_audio_start_divisor_ok

#include "obs-encoder-audio-start.c.h"

#undef obs_encoder_audio_start_divisor_ok
#undef obs_encoder_audio_start_not_video
#undef obs_encoder_audio_start_audio_pts
#undef obs_encoder_audio_start_can_send
#undef obs_encoder_audio_start_push_size
#undef obs_encoder_audio_start_buffer_offset
#undef obs_encoder_audio_start_offset_size
#undef obs_encoder_audio_start_offset_ns
#undef obs_encoder_audio_start_packet
#undef obs_encoder_audio_start_no_video
#undef obs_encoder_audio_start_unpaired
#undef obs_encoder_audio_start_paired
#undef obs_encoder_audio_start_size
#undef obs_encoder_audio_start_video_pts
#undef obs_encoder_audio_start_paired_late
#undef obs_encoder_audio_start_group_ready

/* In the order of rust/obs-c-oracle/src/encoder_audio_start.rs. */
enum {
	ORACLE_ENCODER_AUDIO_START_CONSTANT_COUNT = 1,
	ORACLE_ENCODER_AUDIO_START_LAYOUT_COUNT = 6,
};

long long oracle_encoder_audio_start_constant(size_t which);
size_t oracle_encoder_audio_start_layout(size_t which);

long long oracle_encoder_audio_start_constant(size_t which)
{
	const long long values[ORACLE_ENCODER_AUDIO_START_CONSTANT_COUNT] = {
		OBS_ENCODER_AUDIO_START_TYPE_VIDEO,
	};
	return which < ORACLE_ENCODER_AUDIO_START_CONSTANT_COUNT ? values[which] : -1;
}

size_t oracle_encoder_audio_start_layout(size_t which)
{
	const size_t values[ORACLE_ENCODER_AUDIO_START_LAYOUT_COUNT] = {
		sizeof(struct obs_encoder_audio_start_plan),
		_Alignof(struct obs_encoder_audio_start_plan),
		offsetof(struct obs_encoder_audio_start_plan, wait),
		offsetof(struct obs_encoder_audio_start_plan, truncate),
		offsetof(struct obs_encoder_audio_start_plan, clear),
		offsetof(struct obs_encoder_audio_start_plan, from_buffer),
	};
	return which < ORACLE_ENCODER_AUDIO_START_LAYOUT_COUNT ? values[which] : (size_t)-1;
}
