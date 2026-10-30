/* Test-only oracle: the original libobs/obs-scene-audio-mix.c.h, unmodified,
 * with its two functions renamed to oracle_* so they can link next to the
 * Rust shims. Built with -ffp-contract=off like every oracle, so the
 * multiply and the add of obs_scene_audio_mix_with_buf() stay separate. */
#include <stddef.h>

#define obs_scene_audio_mix_with_buf oracle_obs_scene_audio_mix_with_buf
#define obs_scene_audio_mix_add oracle_obs_scene_audio_mix_add

#include "obs-scene-audio-mix.c.h"

#undef obs_scene_audio_mix_add
#undef obs_scene_audio_mix_with_buf
