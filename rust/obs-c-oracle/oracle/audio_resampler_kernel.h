/* A deterministic FFmpeg dependency double, not a replacement for Tier 2's
 * real libswresample. Both the unchanged C wrapper and Rust see this kernel.
 * State is per context, except an explicitly serialized failure script. */
#pragma once
#include <stdint.h>
#include <stdbool.h>
#include <stddef.h>
#include <inttypes.h>
#include <stdarg.h>
#include <stdlib.h>
#include <string.h>
#include "media-io/audio-resampler.h"
#include "media-io/audio-resampler-kernel.h"
#include "media-io/audio-resampler-drift.h"
#include "util/bmem.h"
#include "util/base.h"
#define AV_VERSION_INT(a, b, c) (((a) << 16) | ((b) << 8) | (c))
#define LIBSWRESAMPLE_VERSION_INT AV_VERSION_INT(4, 4, 100)
#define AV_CH_LAYOUT_MONO AUDIO_RESAMPLER_CH_LAYOUT_MONO
#define AV_CH_LAYOUT_STEREO AUDIO_RESAMPLER_CH_LAYOUT_STEREO
#define AV_CH_LAYOUT_SURROUND AUDIO_RESAMPLER_CH_LAYOUT_SURROUND
#define AV_CH_LAYOUT_4POINT0 AUDIO_RESAMPLER_CH_LAYOUT_4POINT0
#define AV_CH_LAYOUT_4POINT1 AUDIO_RESAMPLER_CH_LAYOUT_4POINT1
#define AV_CH_LAYOUT_5POINT1_BACK AUDIO_RESAMPLER_CH_LAYOUT_5POINT1_BACK
#define AV_CH_LAYOUT_7POINT1 AUDIO_RESAMPLER_CH_LAYOUT_7POINT1
#define AV_ROUND_UP 3
/* Renaming these keeps the remux stand-in independent. Rust's adapter uses
 * these public swr/av samples symbols only in the category test link. */
enum AVSampleFormat {
	AV_SAMPLE_FMT_U8 = 0,
	AV_SAMPLE_FMT_S16 = 1,
	AV_SAMPLE_FMT_S32 = 2,
	AV_SAMPLE_FMT_FLT = 3,
	AV_SAMPLE_FMT_U8P = 5,
	AV_SAMPLE_FMT_S16P = 6,
	AV_SAMPLE_FMT_S32P = 7,
	AV_SAMPLE_FMT_FLTP = 8
};
struct SwrContext;
struct SwrContext *swr_alloc_set_opts(struct SwrContext *, uint64_t, int, int, uint64_t, int, int, int, void *);
int swr_init(struct SwrContext *);
int swr_set_matrix(struct SwrContext *, const double *, int);
int swr_set_compensation(struct SwrContext *, int, int);
int64_t swr_get_delay(struct SwrContext *, int64_t);
int64_t av_rescale_rnd(int64_t, int64_t, int64_t, int);
int av_samples_alloc(uint8_t **, int *, int, int, int, int);
int swr_convert(struct SwrContext *, uint8_t **, int, const uint8_t **, int);
void swr_free(struct SwrContext **);
void av_freep(void *);
void oracle_resampler_kernel_script(int failure);
int oracle_resampler_kernel_live(void);

uint64_t oracle_resampler_kernel_trace(void);
