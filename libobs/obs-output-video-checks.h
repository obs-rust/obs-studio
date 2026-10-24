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
#include <stddef.h>
#include <stdint.h>

/* The video size, conversion and capture checks of obs-output.c, on plain
 * values (obs-output-video-checks.c). Each one is made where the C made it,
 * from values the caller reads at that point:
 * - obs_output_first_mixer(): the mixer a raw output connects its audio to,
 *   the lowest set bit of mixer_mask below MAX_AUDIO_MIXES, else 0
 *   (get_first_mixer());
 * - obs_output_size_route(): where obs_output_get_width2() and
 *   obs_output_get_height2() take one axis from: the encoder of an encoded
 *   output (0 without one), else the scaled size when it is set, else the
 *   video's size;
 * - obs_output_capture_ready(): the flag and presence checks of
 *   can_begin_data_capture() (video_valid(), audio_valid() and the service);
 * - obs_output_begin_gate(): the delay-active and active gates of
 *   obs_output_can_begin_data_capture(), before the join;
 * - obs_output_conversion_plan(): what obs_output_get_video_conversion()
 *   returns: the set conversion (a 0 width or height filled from the
 *   output's size), one built from the scaled size when it differs from
 *   the video's (has_scaling()), or none;
 * - obs_output_congestion_clamp(): the 0-1 clamp of the plugin's
 *   congestion in obs_output_get_congestion(). A NaN passes through.
 *
 * obs-output.c keeps obs_output_valid(), the log_flag_* warnings, the
 * pthread_join() of end_data_capture_thread, obs_encoder_get_width() and
 * _height(), video_output_get_*() and info.get_congestion(): it calls them
 * and passes the results, loads the atomics and passes plain bools, and
 * fills video_conversion from the plan. Nothing here takes a lock, calls
 * into libobs or keeps state.
 *
 * The flags and limits are copied below, so this header needs no core
 * header; obs-output.c checks them against obs-output.h and audio-io.h.
 *
 * Hidden in libobs: not EXPORT. */

/* OBS_OUTPUT_VIDEO, OBS_OUTPUT_AUDIO, OBS_OUTPUT_ENCODED, OBS_OUTPUT_SERVICE */
#define OBS_OUTPUT_VIDEO_CHECKS_VIDEO (1 << 0)
#define OBS_OUTPUT_VIDEO_CHECKS_AUDIO (1 << 1)
#define OBS_OUTPUT_VIDEO_CHECKS_ENCODED (1 << 2)
#define OBS_OUTPUT_VIDEO_CHECKS_SERVICE (1 << 3)
/* MAX_AUDIO_MIXES, MAX_OUTPUT_VIDEO_ENCODERS, MAX_OUTPUT_AUDIO_ENCODERS */
#define OBS_OUTPUT_VIDEO_CHECKS_MAX_AUDIO_MIXES 6
#define OBS_OUTPUT_VIDEO_CHECKS_MAX_VIDEO_ENCODERS 10
#define OBS_OUTPUT_VIDEO_CHECKS_MAX_AUDIO_ENCODERS 6

/* Where obs_output_get_width2() / _height2() take one axis from. */
enum obs_output_size_source {
	/* an encoded output without an encoder at the index: 0 */
	OBS_OUTPUT_SIZE_ZERO = 0,
	/* obs_encoder_get_width() / _height() of the encoder at the index */
	OBS_OUTPUT_SIZE_ENCODER = 1,
	/* scaled_width / scaled_height */
	OBS_OUTPUT_SIZE_SCALED = 2,
	/* video_output_get_width() / _height() of output->video */
	OBS_OUTPUT_SIZE_VIDEO = 3,
};

/* What obs_output_can_begin_data_capture() does before the join. */
enum obs_output_begin_action {
	/* the delay is active: true */
	OBS_OUTPUT_BEGIN_READY = 0,
	/* the output is active: false */
	OBS_OUTPUT_BEGIN_REFUSE = 1,
	/* join a data capture that is ending, then can_begin_data_capture() */
	OBS_OUTPUT_BEGIN_CHECK = 2,
};

/* What obs_output_get_video_conversion() returns. */
enum obs_output_conversion_kind {
	/* NULL */
	OBS_OUTPUT_CONVERSION_NONE = 0,
	/* the set video_conversion, its width and height set to the plan's */
	OBS_OUTPUT_CONVERSION_SET = 1,
	/* video_conversion built from the video's format, the default colour
	 * space and range, and the plan's width and height (the scaled size) */
	OBS_OUTPUT_CONVERSION_SCALED = 2,
};

/* What can_begin_data_capture() looks at. */
struct obs_output_capture_presence {
	/* info.flags */
	uint32_t flags;
	/* bit i set: video_encoders[i] is set (i < MAX_OUTPUT_VIDEO_ENCODERS) */
	uint32_t video_encoders;
	/* bit i set: audio_encoders[i] is set (i < MAX_OUTPUT_AUDIO_ENCODERS) */
	uint32_t audio_encoders;
	/* output->video != NULL */
	bool video;
	/* output->audio != NULL */
	bool audio;
	/* output->service != NULL */
	bool service;
};

/* What obs_output_get_video_conversion() looks at, once it has checked that
 * the output is a raw video output. */
struct obs_output_conversion_sizes {
	/* video_conversion_set */
	bool set;
	/* video_conversion.width and .height */
	uint32_t set_width;
	uint32_t set_height;
	/* scaled_width and scaled_height */
	uint32_t scaled_width;
	uint32_t scaled_height;
	/* video_output_get_width() and _height() of output->video */
	uint32_t video_width;
	uint32_t video_height;
};

struct obs_output_conversion_choice {
	/* an enum obs_output_conversion_kind value */
	int kind;
	/* the conversion's width and height; 0 with OBS_OUTPUT_CONVERSION_NONE */
	uint32_t width;
	uint32_t height;
};

/* get_first_mixer(): the lowest i below MAX_AUDIO_MIXES whose bit is set in
 * mixer_mask, or 0 when none is. */
size_t obs_output_first_mixer(size_t mixer_mask);

/* One axis of obs_output_get_width2() / _height2(), after the validity, flag
 * and index checks: encoded is OBS_OUTPUT_ENCODED, has_encoder whether
 * video_encoders[idx] is set, scaled the scaled size on that axis. */
enum obs_output_size_source obs_output_size_route(bool encoded, bool has_encoder, uint32_t scaled);

/* can_begin_data_capture(): false when the output wants video without a
 * video encoder (encoded) or a video (raw), audio without an audio encoder
 * or audio, or a service without one; true otherwise. */
bool obs_output_capture_ready(const struct obs_output_capture_presence *presence);

/* The gates of obs_output_can_begin_data_capture(): delay_active and active
 * are the loaded atomics. */
enum obs_output_begin_action obs_output_begin_gate(bool delay_active, bool active);

/* obs_output_get_video_conversion(), after its flag checks. On
 * OBS_OUTPUT_CONVERSION_SCALED the caller still returns NULL when the output
 * has no video, since there is no format to convert from (#2127). */
struct obs_output_conversion_choice obs_output_conversion_plan(const struct obs_output_conversion_sizes *sizes);

/* obs_output_get_congestion(): below 0 is 0, above 1 is 1, the rest (NaN
 * included) unchanged. */
float obs_output_congestion_clamp(float congestion);
