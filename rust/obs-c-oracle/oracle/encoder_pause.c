/* Test-only oracle: the original libobs/obs-encoder-pause.c.h, unmodified,
 * with its ten functions renamed to oracle_* so they can link next to the
 * Rust shims. Every input is defined in C (the sums and differences are
 * unsigned). */
#include <stdbool.h>
#include <stdint.h>

#define obs_encoder_pause_off oracle_obs_encoder_pause_off
#define obs_encoder_pause_video_end oracle_obs_encoder_pause_video_end
#define obs_encoder_pause_video_drop oracle_obs_encoder_pause_video_drop
#define obs_encoder_pause_audio_end oracle_obs_encoder_pause_audio_end
#define obs_encoder_pause_audio_start oracle_obs_encoder_pause_audio_start
#define obs_encoder_pause_audio_paused oracle_obs_encoder_pause_audio_paused
#define obs_encoder_pause_audio_cut_ns oracle_obs_encoder_pause_audio_cut_ns
#define obs_encoder_pause_audio_kept oracle_obs_encoder_pause_audio_kept
#define obs_encoder_pause_audio_left oracle_obs_encoder_pause_audio_left
#define obs_encoder_pause_audio_drop oracle_obs_encoder_pause_audio_drop

#include "obs-encoder-pause.c.h"

#undef obs_encoder_pause_audio_drop
#undef obs_encoder_pause_audio_left
#undef obs_encoder_pause_audio_kept
#undef obs_encoder_pause_audio_cut_ns
#undef obs_encoder_pause_audio_paused
#undef obs_encoder_pause_audio_start
#undef obs_encoder_pause_audio_end
#undef obs_encoder_pause_video_drop
#undef obs_encoder_pause_video_end
#undef obs_encoder_pause_off
