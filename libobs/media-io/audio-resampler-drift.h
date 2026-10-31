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

#pragma once

#include <stdbool.h>
#include <stdint.h>

#include "media-io/audio-io.h"

/* The decisions of the libswresample resampler (audio-resampler-ffmpeg.c), on
 * plain values and the compensation state (audio-resampler-drift.c):
 * - audio_resampler_sample_format(): the sample format for an audio format;
 * - audio_resampler_upmix_row(): the matrix row for a mono input;
 * - the clock drift compensation: audio_resampler_comp_set_error() (the dead
 *   band), audio_resampler_comp_step() (one step of the loop filter),
 *   audio_resampler_comp_count() (the sample counters) and
 *   audio_resampler_comp_drift_report() (the report at destroy);
 * - audio_resampler_output_frames(): the room the compensation needs.
 *
 * audio-resampler-ffmpeg.c keeps every libswresample and libavutil call, the
 * output buffer and its size, and every log. Nothing here includes an FFmpeg
 * header: the sample formats are given as their values, which
 * audio-resampler-ffmpeg.c checks against libavutil's. Not EXPORT: only
 * libobs calls these. */

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
 * Errors within AUDIO_RESAMPLER_COMP_DEAD_BAND_NS are arrival jitter or a
 * start offset, not drift, and are ignored, so a device on the system clock
 * is never resampled. The correction is bounded by
 * AUDIO_RESAMPLER_COMP_MAX_PPM. */
#define AUDIO_RESAMPLER_COMP_DEAD_BAND_NS 10000000LL
#define AUDIO_RESAMPLER_COMP_MAX_PPM 1000.0
/* swr applies sample_delta per AUDIO_RESAMPLER_COMP_DISTANCE samples: 0.06 ppm
 * steps. */
#define AUDIO_RESAMPLER_COMP_DISTANCE (1 << 24)
#define AUDIO_RESAMPLER_COMP_GAIN 3.756e-2
#define AUDIO_RESAMPLER_COMP_C1 8.250
#define AUDIO_RESAMPLER_COMP_C2 107.1

/* The libavutil sample formats (enum AVSampleFormat) the resampler uses. */
#define AUDIO_RESAMPLER_FMT_U8 0
#define AUDIO_RESAMPLER_FMT_S16 1
#define AUDIO_RESAMPLER_FMT_S32 2
#define AUDIO_RESAMPLER_FMT_FLT 3
#define AUDIO_RESAMPLER_FMT_U8P 5
#define AUDIO_RESAMPLER_FMT_S16P 6
#define AUDIO_RESAMPLER_FMT_S32P 7
#define AUDIO_RESAMPLER_FMT_FLTP 8

struct audio_resampler_comp {
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

/* What audio_resampler_resample() does after a step of the loop filter. */
enum audio_resampler_comp_action {
	/* not engaged, and the error is 0: nothing */
	AUDIO_RESAMPLER_COMP_IDLE = 0,
	/* the error left the dead band for the first time: log that the drift is
	 * compensated, then set the delta (audio_resampler_comp_delta()) */
	AUDIO_RESAMPLER_COMP_ENGAGE = 1,
	/* set the delta (audio_resampler_comp_delta()) */
	AUDIO_RESAMPLER_COMP_SET = 2,
};

/* The libavutil sample format (an AUDIO_RESAMPLER_FMT_* value) for format: S16
 * for AUDIO_FORMAT_UNKNOWN and for any value past the last. */
int audio_resampler_sample_format(enum audio_format format);

/* The upmix matrix row for a mono input to output_ch channels: the front pair
 * and the centre take the signal, the LFE doesn't. Writes output_ch
 * coefficients to row, the rest 0, and returns true; returns false, and
 * writes nothing, for 0 or more than MAX_AUDIO_CHANNELS channels. */
bool audio_resampler_upmix_row(uint32_t output_ch, double row[MAX_AUDIO_CHANNELS]);

/* audio_resampler_set_compensation_error(): the first call turns compensation
 * on and clears the loop filter, then error_ns less the dead band (0 within
 * it) is the error the next steps see. */
void audio_resampler_comp_set_error(struct audio_resampler_comp *c, int64_t error_ns);

/* One step of the loop filter over in_frames frames at input_freq: both states
 * advance, clamped to AUDIO_RESAMPLER_COMP_MAX_PPM of input_freq; then whether
 * a delta is set, and whether this step engages. */
enum audio_resampler_comp_action audio_resampler_comp_step(struct audio_resampler_comp *c, uint32_t input_freq,
							   uint32_t in_frames);

/* The sample_delta for swr_set_compensation() over
 * AUDIO_RESAMPLER_COMP_DISTANCE samples, from the correction at input_freq.
 * audio_resampler_resample() asks it after the engage log, as before. */
int audio_resampler_comp_delta(const struct audio_resampler_comp *c, uint32_t input_freq);

/* The output frames to make room for: estimated, padded for the samples
 * compensation adds while engaged. */
int audio_resampler_output_frames(int estimated, bool engaged);

/* While engaged, counts in_frames input and converted output samples. */
void audio_resampler_comp_count(struct audio_resampler_comp *c, uint32_t in_frames, int converted);

/* Whether audio_resampler_destroy() reports the drift: engaged, with output
 * counted. */
bool audio_resampler_comp_reports_drift(const struct audio_resampler_comp *c);

/* The input rate the counts give at output_freq, for the report.
 * audio_resampler_destroy() reads output_freq and asks this only when
 * audio_resampler_comp_reports_drift() is true. */
double audio_resampler_comp_input_rate(const struct audio_resampler_comp *c, uint32_t output_freq);
