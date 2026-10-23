/* Test-only oracle: the original libobs/obs-source-audio-input.c.h,
 * unmodified, with its seven functions renamed to oracle_* so they can link
 * next to the Rust shims. */
#include <stddef.h>

#define obs_source_audio_input_discontinuous oracle_obs_source_audio_input_discontinuous
#define obs_source_audio_input_reset_needed oracle_obs_source_audio_input_reset_needed
#define obs_source_audio_input_resampler_needed oracle_obs_source_audio_input_resampler_needed
#define obs_source_audio_input_balance_needed oracle_obs_source_audio_input_balance_needed
#define obs_source_audio_input_mono_needed oracle_obs_source_audio_input_mono_needed
#define obs_source_audio_input_balance oracle_obs_source_audio_input_balance
#define obs_source_audio_input_downmix oracle_obs_source_audio_input_downmix

#include "obs-source-audio-input.c.h"

#undef obs_source_audio_input_downmix
#undef obs_source_audio_input_balance
#undef obs_source_audio_input_mono_needed
#undef obs_source_audio_input_balance_needed
#undef obs_source_audio_input_resampler_needed
#undef obs_source_audio_input_reset_needed
#undef obs_source_audio_input_discontinuous

/* The layouts and values the C uses, in the order of
 * rust/obs-c-oracle/src/source_audio_input.rs. */
enum {
	ORACLE_SOURCE_AUDIO_INPUT_LAYOUT_COUNT = 13,
};

size_t oracle_source_audio_input_layout(size_t which);

size_t oracle_source_audio_input_layout(size_t which)
{
	const size_t values[ORACLE_SOURCE_AUDIO_INPUT_LAYOUT_COUNT] = {
		sizeof(struct obs_source_audio_input_format),
		_Alignof(struct obs_source_audio_input_format),
		offsetof(struct obs_source_audio_input_format, format),
		offsetof(struct obs_source_audio_input_format, samples_per_sec),
		offsetof(struct obs_source_audio_input_format, speakers),
		(size_t)OBS_SOURCE_AUDIO_INPUT_SMOOTHING_THRESHOLD,
		OBS_SOURCE_AUDIO_INPUT_SPEAKERS_STEREO,
		OBS_SOURCE_AUDIO_INPUT_FORCE_MONO,
		OBS_SOURCE_AUDIO_INPUT_SINE_LAW,
		OBS_SOURCE_AUDIO_INPUT_SQUARE_LAW,
		OBS_SOURCE_AUDIO_INPUT_LINEAR,
		sizeof(int),
		sizeof(bool),
	};
	return which < ORACLE_SOURCE_AUDIO_INPUT_LAYOUT_COUNT ? values[which] : 0;
}
