/* Original wrapper, verbatim apart from the build-only PCH extraction. */
#include "audio_resampler_kernel.h"
#define audio_resampler_create oracle_audio_resampler_create
#define audio_resampler_destroy oracle_audio_resampler_destroy
#define audio_resampler_resample oracle_audio_resampler_resample
#define audio_resampler_set_compensation_error oracle_audio_resampler_set_compensation_error
void oracle_audio_resampler_set_compensation_error(audio_resampler_t *, int64_t);
bool oracle_audio_resampler_resample(audio_resampler_t *, uint8_t **, uint32_t *, uint64_t *, const uint8_t *const *,
				     uint32_t);
/* Prototypes precede the extract, whose create calls destroy. */
audio_resampler_t *oracle_audio_resampler_create(const struct resample_info *, const struct resample_info *);
void oracle_audio_resampler_destroy(audio_resampler_t *);
/* The original pre-4.5 FFmpeg path passes unsigned rates to int arguments.
 * Keep the unchanged source as the oracle, including that narrowing. */
#ifndef _MSC_VER
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wsign-conversion"
#endif
#include "audio-resampler-ffmpeg.inc"
#ifndef _MSC_VER
#pragma GCC diagnostic pop
#endif
size_t oracle_resample_info_layout(int field);
size_t oracle_resample_info_layout(int field)
{
	switch (field) {
	case 0:
		return sizeof(struct resample_info);
	case 1:
		return _Alignof(struct resample_info);
	case 2:
		return offsetof(struct resample_info, samples_per_sec);
	case 3:
		return offsetof(struct resample_info, format);
	default:
		return offsetof(struct resample_info, speakers);
	}
}

struct audio_resampler_comp oracle_audio_resampler_snapshot(audio_resampler_t *rs);
struct audio_resampler_comp oracle_audio_resampler_snapshot(audio_resampler_t *rs)
{
	return rs->comp;
}
