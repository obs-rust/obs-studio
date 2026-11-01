/* Test-only oracle: the original libobs/media-io/audio-resampler-plan.c.h,
 * unmodified, with its six functions renamed to oracle_* so they can link
 * next to the Rust shims, plus its channel masks as the C sees them. It calls get_audio_channels() and
 * is_audio_planar() (static inline in audio-io.h) and nothing else outside
 * the file. */
#include <stddef.h>
#include <stdint.h>

#define audio_resampler_layout_mask oracle_audio_resampler_layout_mask
#define audio_resampler_output_channels oracle_audio_resampler_output_channels
#define audio_resampler_planar_output oracle_audio_resampler_planar_output
#define audio_resampler_input_channels oracle_audio_resampler_input_channels
#define audio_resampler_is_4point1 oracle_audio_resampler_is_4point1
#define audio_resampler_upmixes_mono oracle_audio_resampler_upmixes_mono

#include "media-io/audio-resampler-plan.c.h"

/* The integer values the C uses, in the order of
 * rust/obs-c-oracle/src/audio_resampler_plan.rs. */
enum {
	ORACLE_RESAMPLER_PLAN_INT_COUNT = 7,
};

int64_t oracle_audio_resampler_plan_int(size_t which);

int64_t oracle_audio_resampler_plan_int(size_t which)
{
	const int64_t values[ORACLE_RESAMPLER_PLAN_INT_COUNT] = {
		(int64_t)AUDIO_RESAMPLER_CH_LAYOUT_MONO,     (int64_t)AUDIO_RESAMPLER_CH_LAYOUT_STEREO,
		(int64_t)AUDIO_RESAMPLER_CH_LAYOUT_SURROUND, (int64_t)AUDIO_RESAMPLER_CH_LAYOUT_4POINT0,
		(int64_t)AUDIO_RESAMPLER_CH_LAYOUT_4POINT1,  (int64_t)AUDIO_RESAMPLER_CH_LAYOUT_5POINT1_BACK,
		(int64_t)AUDIO_RESAMPLER_CH_LAYOUT_7POINT1,
	};
	return which < ORACLE_RESAMPLER_PLAN_INT_COUNT ? values[which] : -1;
}
