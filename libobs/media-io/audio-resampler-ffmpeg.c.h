/******************************************************************************
    Copyright (C) 2023 by Lain Bailey <lain@obsproject.com>

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 2 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <http://www.gnu.org/licenses/>.
******************************************************************************/

#include "build/libobs-media.pch.h"

struct audio_resampler {
	struct SwrContext *context;
	bool opened;

	uint32_t input_freq;
	enum AVSampleFormat input_format;
	uint8_t *output_buffer[MAX_AV_PLANES];
	enum AVSampleFormat output_format;
	int output_size;
	uint32_t output_ch;
	uint32_t output_freq;
	uint32_t output_planes;
#if LIBSWRESAMPLE_VERSION_INT < AV_VERSION_INT(4, 5, 100)
	uint64_t input_layout;
	uint64_t output_layout;
#else
	AVChannelLayout input_ch_layout;
	AVChannelLayout output_ch_layout;
#endif

	struct audio_resampler_comp comp;
};

/* audio-resampler-drift.c gives the sample formats as their values. */
_Static_assert(AUDIO_RESAMPLER_FMT_U8 == AV_SAMPLE_FMT_U8, "AV_SAMPLE_FMT_U8");
_Static_assert(AUDIO_RESAMPLER_FMT_S16 == AV_SAMPLE_FMT_S16, "AV_SAMPLE_FMT_S16");
_Static_assert(AUDIO_RESAMPLER_FMT_S32 == AV_SAMPLE_FMT_S32, "AV_SAMPLE_FMT_S32");
_Static_assert(AUDIO_RESAMPLER_FMT_FLT == AV_SAMPLE_FMT_FLT, "AV_SAMPLE_FMT_FLT");
_Static_assert(AUDIO_RESAMPLER_FMT_U8P == AV_SAMPLE_FMT_U8P, "AV_SAMPLE_FMT_U8P");
_Static_assert(AUDIO_RESAMPLER_FMT_S16P == AV_SAMPLE_FMT_S16P, "AV_SAMPLE_FMT_S16P");
_Static_assert(AUDIO_RESAMPLER_FMT_S32P == AV_SAMPLE_FMT_S32P, "AV_SAMPLE_FMT_S32P");
_Static_assert(AUDIO_RESAMPLER_FMT_FLTP == AV_SAMPLE_FMT_FLTP, "AV_SAMPLE_FMT_FLTP");

#if LIBSWRESAMPLE_VERSION_INT < AV_VERSION_INT(4, 5, 100)
static inline uint64_t convert_speaker_layout(enum speaker_layout layout)
{
	switch (layout) {
	case SPEAKERS_UNKNOWN:
		return 0;
	case SPEAKERS_MONO:
		return AV_CH_LAYOUT_MONO;
	case SPEAKERS_STEREO:
		return AV_CH_LAYOUT_STEREO;
	case SPEAKERS_2POINT1:
		return AV_CH_LAYOUT_SURROUND;
	case SPEAKERS_4POINT0:
		return AV_CH_LAYOUT_4POINT0;
	case SPEAKERS_4POINT1:
		return AV_CH_LAYOUT_4POINT1;
	case SPEAKERS_5POINT1:
		return AV_CH_LAYOUT_5POINT1_BACK;
	case SPEAKERS_7POINT1:
		return AV_CH_LAYOUT_7POINT1;
	}

	/* shouldn't get here */
	return 0;
}
#endif

audio_resampler_t *audio_resampler_create(const struct resample_info *dst, const struct resample_info *src)
{
	struct audio_resampler *rs = bzalloc(sizeof(struct audio_resampler));
	int errcode;

	rs->opened = false;
	rs->input_freq = src->samples_per_sec;
	rs->input_format = (enum AVSampleFormat)audio_resampler_sample_format(src->format);
	rs->output_size = 0;
	rs->output_ch = get_audio_channels(dst->speakers);
	rs->output_freq = dst->samples_per_sec;
	rs->output_format = (enum AVSampleFormat)audio_resampler_sample_format(dst->format);
	rs->output_planes = is_audio_planar(dst->format) ? rs->output_ch : 1;

#if (LIBSWRESAMPLE_VERSION_INT < AV_VERSION_INT(4, 5, 100))
	rs->input_layout = convert_speaker_layout(src->speakers);
	rs->output_layout = convert_speaker_layout(dst->speakers);
	rs->context = swr_alloc_set_opts(NULL, rs->output_layout, rs->output_format, dst->samples_per_sec,
					 rs->input_layout, rs->input_format, src->samples_per_sec, 0, NULL);
#else
	int nb_ch = (int)get_audio_channels(src->speakers);
	av_channel_layout_default(&rs->input_ch_layout, nb_ch);
	av_channel_layout_default(&rs->output_ch_layout, (int)rs->output_ch);
	if (src->speakers == SPEAKERS_4POINT1)
		rs->input_ch_layout = (AVChannelLayout)AV_CHANNEL_LAYOUT_4POINT1;

	if (dst->speakers == SPEAKERS_4POINT1)
		rs->output_ch_layout = (AVChannelLayout)AV_CHANNEL_LAYOUT_4POINT1;

	swr_alloc_set_opts2(&rs->context, &rs->output_ch_layout, rs->output_format, (int)dst->samples_per_sec,
			    &rs->input_ch_layout, rs->input_format, (int)src->samples_per_sec, 0, NULL);
#endif

	if (!rs->context) {
		blog(LOG_ERROR, "swr_alloc_set_opts failed");
		audio_resampler_destroy(rs);
		return NULL;
	}

#if (LIBSWRESAMPLE_VERSION_INT < AV_VERSION_INT(4, 5, 100))
	if (rs->input_layout == AV_CH_LAYOUT_MONO && rs->output_ch > 1) {
#else
	AVChannelLayout test_ch = AV_CHANNEL_LAYOUT_MONO;
	if (av_channel_layout_compare(&rs->input_ch_layout, &test_ch) == 0 && rs->output_ch > 1) {
#endif
		double row[MAX_AUDIO_CHANNELS];
		if (audio_resampler_upmix_row(rs->output_ch, row) && swr_set_matrix(rs->context, row, 1) < 0)
			blog(LOG_DEBUG, "swr_set_matrix failed for mono upmix\n");
	}

	errcode = swr_init(rs->context);
	if (errcode != 0) {
		blog(LOG_ERROR, "avresample_open failed: error code %d", errcode);
		audio_resampler_destroy(rs);
		return NULL;
	}

	return rs;
}

void audio_resampler_destroy(audio_resampler_t *rs)
{
	if (rs) {
		if (audio_resampler_comp_reports_drift(&rs->comp))
			blog(LOG_INFO,
			     "audio_resampler (clock drift compensation): input %" PRIu64 " samples, output %" PRIu64
			     " samples, estimated input rate %.3f Hz",
			     rs->comp.input_samples, rs->comp.output_samples,
			     audio_resampler_comp_input_rate(&rs->comp, rs->output_freq));

		if (rs->context)
			swr_free(&rs->context);
		if (rs->output_buffer[0])
			av_freep(&rs->output_buffer[0]);

		bfree(rs);
	}
}

void audio_resampler_set_compensation_error(audio_resampler_t *rs, int64_t error_ns)
{
	if (!rs)
		return;

	audio_resampler_comp_set_error(&rs->comp, error_ns);
}

static bool update_compensation(audio_resampler_t *rs, uint32_t in_frames)
{
	enum audio_resampler_comp_action action = audio_resampler_comp_step(&rs->comp, rs->input_freq, in_frames);
	if (action == AUDIO_RESAMPLER_COMP_IDLE)
		return true;
	if (action == AUDIO_RESAMPLER_COMP_ENGAGE)
		blog(LOG_INFO, "audio_resampler: compensating for the clock drift of an audio device");

	int delta = audio_resampler_comp_delta(&rs->comp, rs->input_freq);

	int ret = swr_set_compensation(rs->context, delta, AUDIO_RESAMPLER_COMP_DISTANCE);
	if (ret < 0) {
		blog(LOG_ERROR, "swr_set_compensation failed: %d", ret);
		return false;
	}
	return true;
}

bool audio_resampler_resample(audio_resampler_t *rs, uint8_t *output[], uint32_t *out_frames, uint64_t *ts_offset,
			      const uint8_t *const input[], uint32_t in_frames)
{
	if (!rs)
		return false;

	struct SwrContext *context = rs->context;
	int ret;

	if (rs->comp.enabled && !update_compensation(rs, in_frames))
		return false;

	int64_t delay = swr_get_delay(context, rs->input_freq);
	int estimated = (int)av_rescale_rnd(delay + (int64_t)in_frames, (int64_t)rs->output_freq,
					    (int64_t)rs->input_freq, AV_ROUND_UP);
	estimated = audio_resampler_output_frames(estimated, rs->comp.engaged);

	*ts_offset = (uint64_t)swr_get_delay(context, 1000000000);

	/* resize the buffer if bigger */
	if (estimated > rs->output_size) {
		if (rs->output_buffer[0])
			av_freep(&rs->output_buffer[0]);

		av_samples_alloc(rs->output_buffer, NULL, (int)rs->output_ch, estimated, rs->output_format, 0);

		rs->output_size = estimated;
	}

	ret = swr_convert(context, rs->output_buffer, rs->output_size, (const uint8_t **)(uintptr_t)input,
			  (int)in_frames);

	if (ret < 0) {
		blog(LOG_ERROR, "swr_convert failed: %d", ret);
		return false;
	}

	for (uint32_t i = 0; i < rs->output_planes; i++)
		output[i] = rs->output_buffer[i];

	audio_resampler_comp_count(&rs->comp, in_frames, ret);

	*out_frames = (uint32_t)ret;
	return true;
}
