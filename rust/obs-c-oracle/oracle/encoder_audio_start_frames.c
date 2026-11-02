/* Test-only oracle: the original libobs/obs-encoder-audio-start-frames.c.h,
 * unmodified, with its functions renamed to oracle_* so they can link next
 * to the Rust shims. util_mul_div64() stays the header inline, the
 * platform's own formula. Callers keep the rate above 0 for
 * oracle_obs_encoder_audio_start_frames_duration(), which divides by it, and
 * keep the quotients within 64 bits, where MSVC x64's _udiv128() faults. */
#include <stdint.h>

#define obs_encoder_audio_start_frames_offset oracle_obs_encoder_audio_start_frames_offset
#define obs_encoder_audio_start_frames_duration oracle_obs_encoder_audio_start_frames_duration

#include "obs-encoder-audio-start-frames.c.h"

#undef obs_encoder_audio_start_frames_duration
#undef obs_encoder_audio_start_frames_offset
