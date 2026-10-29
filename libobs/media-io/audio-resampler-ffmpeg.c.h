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

/* Asynchronous sample rate compensation, after Norihiro Kamae's
 * obsproject/obs-studio#6351.
 *
 * A capture device runs on its own clock, slightly off its nominal rate, so
 * it delivers a few samples per second more or fewer than the system clock
 * consumes. The source reports how far the samples it has delivered run
 * ahead of (positive) or behind (negative) their timestamps, and a lag-lead
 * loop filter turns that into a slight resampling ratio, the way a PLL locks
 * to a reference: the error is integrated into the drift of the device
 * clock, so the error itself settles. The loop crosses unity gain at 1/180 Hz
 * with 60 degrees of phase margin (component values from #6351), so the
 * correction follows the drift but not the jitter of packet arrival.
 *
 * Errors within COMP_DEAD_BAND_NS are arrival jitter or a start offset, not
 * drift, and are ignored, so a device on the system clock is never
 * resampled. The correction is bounded by COMP_MAX_PPM. */
#define COMP_DEAD_BAND_NS 10000000LL
#define COMP_MAX_PPM 1000.0
/* swr applies sample_delta per COMP_DISTANCE samples: 0.06 ppm steps. */
#define COMP_DISTANCE (1 << 24)
#define COMP_GAIN 3.756e-2
#define COMP_C1 8.250
#define COMP_C2 107.1

struct compensation {
	bool enabled;
	/* swr resamples once the error first leaves the dead band */
	bool engaged;
	int64_t error_ns;
	/* loop filter state, in samples per second */
	double vc1;
	double vc2;
	uint64_t input_samples;
	uint64_t output_samples;
};

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

	struct compensation comp;
};

static inline enum AVSampleFormat convert_audio_format(enum audio_format format)
{
	switch (format) {
	case AUDIO_FORMAT_UNKNOWN:
		return AV_SAMPLE_FMT_S16;
	case AUDIO_FORMAT_U8BIT:
		return AV_SAMPLE_FMT_U8;
	case AUDIO_FORMAT_16BIT:
		return AV_SAMPLE_FMT_S16;
	case AUDIO_FORMAT_32BIT:
		return AV_SAMPLE_FMT_S32;
	case AUDIO_FORMAT_FLOAT:
		return AV_SAMPLE_FMT_FLT;
	case AUDIO_FORMAT_U8BIT_PLANAR:
		return AV_SAMPLE_FMT_U8P;
	case AUDIO_FORMAT_16BIT_PLANAR:
		return AV_SAMPLE_FMT_S16P;
	case AUDIO_FORMAT_32BIT_PLANAR:
		return AV_SAMPLE_FMT_S32P;
	case AUDIO_FORMAT_FLOAT_PLANAR:
		return AV_SAMPLE_FMT_FLTP;
	}

	/* shouldn't get here */
	return AV_SAMPLE_FMT_S16;
}

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
	rs->input_format = convert_audio_format(src->format);
	rs->output_size = 0;
	rs->output_ch = get_audio_channels(dst->speakers);
	rs->output_freq = dst->samples_per_sec;
	rs->output_format = convert_audio_format(dst->format);
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
		const double matrix[MAX_AUDIO_CHANNELS][MAX_AUDIO_CHANNELS] = {
			{1},
			{1, 1},
			{1, 1, 0},
			{1, 1, 1, 1},
			{1, 1, 1, 0, 1},
			{1, 1, 1, 1, 1, 1},
			{1, 1, 1, 0, 1, 1, 1},
			{1, 1, 1, 0, 1, 1, 1, 1},
		};
		if (swr_set_matrix(rs->context, matrix[rs->output_ch - 1], 1) < 0)
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
		if (rs->comp.engaged && rs->comp.output_samples)
			blog(LOG_INFO,
			     "audio_resampler (clock drift compensation): input %" PRIu64 " samples, output %" PRIu64
			     " samples, estimated input rate %.3f Hz",
			     rs->comp.input_samples, rs->comp.output_samples,
			     (double)rs->comp.input_samples / (double)rs->comp.output_samples * rs->output_freq);

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

	if (!rs->comp.enabled) {
		rs->comp.enabled = true;
		rs->comp.vc1 = 0.0;
		rs->comp.vc2 = 0.0;
	}

	if (error_ns > COMP_DEAD_BAND_NS)
		rs->comp.error_ns = error_ns - COMP_DEAD_BAND_NS;
	else if (error_ns < -COMP_DEAD_BAND_NS)
		rs->comp.error_ns = error_ns + COMP_DEAD_BAND_NS;
	else
		rs->comp.error_ns = 0;
}

static inline double clamp_drift(double drift, double max)
{
	return drift > max ? max : (drift < -max ? -max : drift);
}

static bool update_compensation(audio_resampler_t *rs, uint32_t in_frames)
{
	struct compensation *c = &rs->comp;
	double fs = (double)rs->input_freq;
	double max = COMP_MAX_PPM * 1e-6 * fs;

	/* Lag-lead filter: a charge pump (COMP_GAIN) into C1 in parallel
	 * with a series 1 ohm and C2, advanced by in_frames samples. */
	double icp = COMP_GAIN * (double)c->error_ns * 1e-9;
	double ic2 = (c->vc1 - c->vc2) / fs;
	double ic1 = icp - ic2;

	c->vc1 = clamp_drift(c->vc1 + ic1 / COMP_C1 * in_frames, max);
	c->vc2 = clamp_drift(c->vc2 + ic2 / COMP_C2 * in_frames, max);

	if (!c->engaged) {
		if (!c->error_ns)
			return true;
		c->engaged = true;
		blog(LOG_INFO, "audio_resampler: compensating for the clock drift of an audio device");
	}

	/* Positive drift: the device delivers too many samples, so drop
	 * some (a negative sample_delta). */
	int delta = -(int)lround(c->vc1 * COMP_DISTANCE / fs);
	int ret = swr_set_compensation(rs->context, delta, COMP_DISTANCE);
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
	/* room for the samples compensation adds */
	if (rs->comp.engaged)
		estimated += estimated / 512 + 2;

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

	if (rs->comp.engaged) {
		rs->comp.input_samples += in_frames;
		rs->comp.output_samples += (uint64_t)ret;
	}

	*out_frames = (uint32_t)ret;
	return true;
}

/* File-local macros (BUILD.md): out of the next file in the unity group. */
#undef COMP_DEAD_BAND_NS
#undef COMP_MAX_PPM
#undef COMP_DISTANCE
#undef COMP_GAIN
#undef COMP_C1
#undef COMP_C2
