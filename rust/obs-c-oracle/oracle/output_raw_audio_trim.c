/* Test-only oracle: the original libobs/obs-output-raw-audio-trim.c.h,
 * unmodified, with its functions renamed to oracle_* so they can link next
 * to the Rust shims. util_mul_div64() stays the header inline, the
 * platform's own formula. Callers keep the rate above 0 for
 * oracle_obs_output_raw_audio_trim_duration(), which divides by it, and keep
 * the quotients within 64 bits, where MSVC x64's _udiv128() faults. */
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#define obs_output_raw_audio_trim_duration oracle_obs_output_raw_audio_trim_duration
#define obs_output_raw_audio_trim_frames oracle_obs_output_raw_audio_trim_frames

#include "obs-output-raw-audio-trim.c.h"

#undef obs_output_raw_audio_trim_frames
#undef obs_output_raw_audio_trim_duration
