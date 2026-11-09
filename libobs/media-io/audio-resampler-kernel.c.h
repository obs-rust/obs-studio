/* FFmpeg's version-specific channel layout allocation. The resampler's
 * ownership, compensation and packet orchestration live in obs-media-io. */
#include "libobs/build/libobs-media-resampler-kernel.pch.h"

struct SwrContext *obs_audio_resampler_alloc(const struct resample_info *dst, const struct resample_info *src)
{
	struct SwrContext *context = NULL;
	enum AVSampleFormat input_format = (enum AVSampleFormat)audio_resampler_sample_format(src->format);
	enum AVSampleFormat output_format = (enum AVSampleFormat)audio_resampler_sample_format(dst->format);
#if LIBSWRESAMPLE_VERSION_INT < AV_VERSION_INT(4, 5, 100)
	context = swr_alloc_set_opts(NULL, audio_resampler_layout_mask(dst->speakers), output_format,
				     (int)dst->samples_per_sec, audio_resampler_layout_mask(src->speakers),
				     input_format, (int)src->samples_per_sec, 0, NULL);
#else
	AVChannelLayout input_layout, output_layout;
	av_channel_layout_default(&input_layout, audio_resampler_input_channels(src->speakers));
	av_channel_layout_default(&output_layout, (int)audio_resampler_output_channels(dst->speakers));
	if (audio_resampler_is_4point1(src->speakers))
		input_layout = (AVChannelLayout)AV_CHANNEL_LAYOUT_4POINT1;
	if (audio_resampler_is_4point1(dst->speakers))
		output_layout = (AVChannelLayout)AV_CHANNEL_LAYOUT_4POINT1;
	swr_alloc_set_opts2(&context, &output_layout, output_format, (int)dst->samples_per_sec, &input_layout,
			    input_format, (int)src->samples_per_sec, 0, NULL);
	av_channel_layout_uninit(&input_layout);
	av_channel_layout_uninit(&output_layout);
#endif
	return context;
}

void obs_audio_resampler_log(int kind, int code, uint64_t input, uint64_t output, double rate)
{
	switch (kind) {
	case 0:
		blog(LOG_ERROR, "swr_alloc_set_opts failed");
		break;
	case 1:
		blog(LOG_DEBUG, "swr_set_matrix failed for mono upmix\n");
		break;
	case 2:
		blog(LOG_ERROR, "avresample_open failed: error code %d", code);
		break;
	case 3:
		blog(LOG_INFO, "audio_resampler: compensating for the clock drift of an audio device");
		break;
	case 4:
		blog(LOG_ERROR, "swr_set_compensation failed: %d", code);
		break;
	case 5:
		blog(LOG_ERROR, "swr_convert failed: %d", code);
		break;
	case 6:
		blog(LOG_INFO,
		     "audio_resampler (clock drift compensation): input %" PRIu64 " samples, output %" PRIu64
		     " samples, estimated input rate %.3f Hz",
		     input, output, rate);
		break;
	}
}
