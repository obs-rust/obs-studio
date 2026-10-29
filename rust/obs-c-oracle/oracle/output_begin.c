/* Test-only oracle: the original libobs/obs-output-begin.c.h, unmodified,
 * with its five functions renamed to oracle_* so they can link next to the
 * Rust shims. The parity test never passes the inputs where C is undefined
 * (an fps_num, sample rate or interval of 0, a quotient above 64 bits on
 * MSVC x64). */
#include <stddef.h>
#include <stdint.h>

#define obs_output_begin_video_interval oracle_obs_output_begin_video_interval
#define obs_output_begin_audio_interval oracle_obs_output_begin_audio_interval
#define obs_output_begin_batch_size oracle_obs_output_begin_batch_size
#define obs_output_begin_pair_video oracle_obs_output_begin_pair_video
#define obs_output_begin_pair_audio oracle_obs_output_begin_pair_audio

#include "obs-output-begin.c.h"

#undef obs_output_begin_pair_audio
#undef obs_output_begin_pair_video
#undef obs_output_begin_batch_size
#undef obs_output_begin_audio_interval
#undef obs_output_begin_video_interval
