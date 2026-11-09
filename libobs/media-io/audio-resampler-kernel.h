/* Private FFmpeg boundary; not part of the libobs public API. */
#pragma once
#include <stdint.h>
struct SwrContext;
struct resample_info;
struct SwrContext *obs_audio_resampler_alloc(const struct resample_info *dst, const struct resample_info *src);
void obs_audio_resampler_log(int kind, int code, uint64_t input, uint64_t output, double rate);
