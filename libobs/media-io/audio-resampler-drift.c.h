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

#include "libobs/build/libobs-ported-audio-resampler-drift.pch.h"

/* Ported to rust/obs-media-io (audio_resampler_drift): the hybrid libobs drops
 * this file. */

/* The decisions of the libswresample resampler (audio-resampler-ffmpeg.c),
 * moved: the sample format, the mono upmix row, the clock drift compensation
 * and the output frames. audio-resampler-ffmpeg.c keeps every libswresample
 * and libavutil call, the buffer and every log. Declared in
 * audio-resampler-drift.h without EXPORT. */

int audio_resampler_sample_format(enum audio_format format)
{
	switch (format) {
	case AUDIO_FORMAT_UNKNOWN:
		return AUDIO_RESAMPLER_FMT_S16;
	case AUDIO_FORMAT_U8BIT:
		return AUDIO_RESAMPLER_FMT_U8;
	case AUDIO_FORMAT_16BIT:
		return AUDIO_RESAMPLER_FMT_S16;
	case AUDIO_FORMAT_32BIT:
		return AUDIO_RESAMPLER_FMT_S32;
	case AUDIO_FORMAT_FLOAT:
		return AUDIO_RESAMPLER_FMT_FLT;
	case AUDIO_FORMAT_U8BIT_PLANAR:
		return AUDIO_RESAMPLER_FMT_U8P;
	case AUDIO_FORMAT_16BIT_PLANAR:
		return AUDIO_RESAMPLER_FMT_S16P;
	case AUDIO_FORMAT_32BIT_PLANAR:
		return AUDIO_RESAMPLER_FMT_S32P;
	case AUDIO_FORMAT_FLOAT_PLANAR:
		return AUDIO_RESAMPLER_FMT_FLTP;
	}

	/* shouldn't get here */
	return AUDIO_RESAMPLER_FMT_S16;
}

bool audio_resampler_upmix_row(uint32_t output_ch, double row[MAX_AUDIO_CHANNELS])
{
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

	if (output_ch == 0 || output_ch > MAX_AUDIO_CHANNELS)
		return false;

	for (size_t i = 0; i < MAX_AUDIO_CHANNELS; i++)
		row[i] = matrix[output_ch - 1][i];
	return true;
}

void audio_resampler_comp_set_error(struct audio_resampler_comp *c, int64_t error_ns)
{
	if (!c->enabled) {
		c->enabled = true;
		c->vc1 = 0.0;
		c->vc2 = 0.0;
	}

	if (error_ns > AUDIO_RESAMPLER_COMP_DEAD_BAND_NS)
		c->error_ns = error_ns - AUDIO_RESAMPLER_COMP_DEAD_BAND_NS;
	else if (error_ns < -AUDIO_RESAMPLER_COMP_DEAD_BAND_NS)
		c->error_ns = error_ns + AUDIO_RESAMPLER_COMP_DEAD_BAND_NS;
	else
		c->error_ns = 0;
}

static inline double clamp_drift(double drift, double max)
{
	return drift > max ? max : (drift < -max ? -max : drift);
}

enum audio_resampler_comp_action audio_resampler_comp_step(struct audio_resampler_comp *c, uint32_t input_freq,
							   uint32_t in_frames)
{
	double fs = (double)input_freq;
	double max = AUDIO_RESAMPLER_COMP_MAX_PPM * 1e-6 * fs;
	enum audio_resampler_comp_action action = AUDIO_RESAMPLER_COMP_SET;

	/* Lag-lead filter: a charge pump (AUDIO_RESAMPLER_COMP_GAIN) into C1 in
	 * parallel with a series 1 ohm and C2, advanced by in_frames samples. */
	double icp = AUDIO_RESAMPLER_COMP_GAIN * (double)c->error_ns * 1e-9;
	double ic2 = (c->vc1 - c->vc2) / fs;
	double ic1 = icp - ic2;

	c->vc1 = clamp_drift(c->vc1 + ic1 / AUDIO_RESAMPLER_COMP_C1 * in_frames, max);
	c->vc2 = clamp_drift(c->vc2 + ic2 / AUDIO_RESAMPLER_COMP_C2 * in_frames, max);

	if (!c->engaged) {
		if (!c->error_ns)
			return AUDIO_RESAMPLER_COMP_IDLE;
		c->engaged = true;
		action = AUDIO_RESAMPLER_COMP_ENGAGE;
	}

	return action;
}

int audio_resampler_comp_delta(const struct audio_resampler_comp *c, uint32_t input_freq)
{
	double fs = (double)input_freq;

	/* Positive drift: the device delivers too many samples, so drop
	 * some (a negative sample_delta). */
	return -(int)lround(c->vc1 * AUDIO_RESAMPLER_COMP_DISTANCE / fs);
}

int audio_resampler_output_frames(int estimated, bool engaged)
{
	/* room for the samples compensation adds */
	if (engaged)
		estimated += estimated / 512 + 2;
	return estimated;
}

void audio_resampler_comp_count(struct audio_resampler_comp *c, uint32_t in_frames, int converted)
{
	if (c->engaged) {
		c->input_samples += in_frames;
		c->output_samples += (uint64_t)converted;
	}
}

bool audio_resampler_comp_reports_drift(const struct audio_resampler_comp *c)
{
	return c->engaged && c->output_samples;
}

double audio_resampler_comp_input_rate(const struct audio_resampler_comp *c, uint32_t output_freq)
{
	return (double)c->input_samples / (double)c->output_samples * output_freq;
}
