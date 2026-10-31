/* Test-only oracle: the original libobs/media-io/audio-resampler-drift.c.h,
 * unmodified, with its nine functions renamed to oracle_* so they can link
 * next to the Rust shims, plus its constants and the layout of struct
 * audio_resampler_comp as the C sees them. It calls lround() and nothing
 * else outside the file; it is built with -ffp-contract=off, as libobs builds
 * the file. */
#include <stddef.h>
#include <stdint.h>

#define audio_resampler_sample_format oracle_audio_resampler_sample_format
#define audio_resampler_upmix_row oracle_audio_resampler_upmix_row
#define audio_resampler_comp_set_error oracle_audio_resampler_comp_set_error
#define audio_resampler_comp_step oracle_audio_resampler_comp_step
#define audio_resampler_output_frames oracle_audio_resampler_output_frames
#define audio_resampler_comp_count oracle_audio_resampler_comp_count
#define audio_resampler_comp_delta oracle_audio_resampler_comp_delta
#define audio_resampler_comp_reports_drift oracle_audio_resampler_comp_reports_drift
#define audio_resampler_comp_input_rate oracle_audio_resampler_comp_input_rate

#include "media-io/audio-resampler-drift.c.h"

/* The integer values the C uses, in the order of
 * rust/obs-c-oracle/src/audio_resampler_drift.rs. */
enum {
	ORACLE_RESAMPLER_DRIFT_INT_COUNT = 24,
	ORACLE_RESAMPLER_DRIFT_DOUBLE_COUNT = 4,
};

int64_t oracle_audio_resampler_drift_int(size_t which);
double oracle_audio_resampler_drift_double(size_t which);

int64_t oracle_audio_resampler_drift_int(size_t which)
{
	const int64_t values[ORACLE_RESAMPLER_DRIFT_INT_COUNT] = {
		AUDIO_RESAMPLER_COMP_DEAD_BAND_NS,
		AUDIO_RESAMPLER_COMP_DISTANCE,
		AUDIO_RESAMPLER_FMT_U8,
		AUDIO_RESAMPLER_FMT_S16,
		AUDIO_RESAMPLER_FMT_S32,
		AUDIO_RESAMPLER_FMT_FLT,
		AUDIO_RESAMPLER_FMT_U8P,
		AUDIO_RESAMPLER_FMT_S16P,
		AUDIO_RESAMPLER_FMT_S32P,
		AUDIO_RESAMPLER_FMT_FLTP,
		MAX_AUDIO_CHANNELS,
		AUDIO_RESAMPLER_COMP_IDLE,
		AUDIO_RESAMPLER_COMP_ENGAGE,
		AUDIO_RESAMPLER_COMP_SET,
		(int64_t)sizeof(enum audio_resampler_comp_action),
		(int64_t)sizeof(struct audio_resampler_comp),
		(int64_t)_Alignof(struct audio_resampler_comp),
		(int64_t)offsetof(struct audio_resampler_comp, enabled),
		(int64_t)offsetof(struct audio_resampler_comp, engaged),
		(int64_t)offsetof(struct audio_resampler_comp, error_ns),
		(int64_t)offsetof(struct audio_resampler_comp, vc1),
		(int64_t)offsetof(struct audio_resampler_comp, vc2),
		(int64_t)offsetof(struct audio_resampler_comp, input_samples),
		(int64_t)offsetof(struct audio_resampler_comp, output_samples),
	};
	return which < ORACLE_RESAMPLER_DRIFT_INT_COUNT ? values[which] : -1;
}

double oracle_audio_resampler_drift_double(size_t which)
{
	const double values[ORACLE_RESAMPLER_DRIFT_DOUBLE_COUNT] = {
		AUDIO_RESAMPLER_COMP_MAX_PPM,
		AUDIO_RESAMPLER_COMP_GAIN,
		AUDIO_RESAMPLER_COMP_C1,
		AUDIO_RESAMPLER_COMP_C2,
	};
	return which < ORACLE_RESAMPLER_DRIFT_DOUBLE_COUNT ? values[which] : -1.0;
}
