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

/* The encoder size and info choices of obs-encoder.c, on plain values
 * (obs-encoder-size.c). Each one is made where the C made it, from values the
 * caller reads at that point:
 * - obs_encoder_size_audio_defaults(): get_audio_info()'s default fill, before
 *   the plugin's get_audio_info: an unknown format, a 0 sample rate and
 *   unknown speakers come from the audio mix;
 * - obs_encoder_size_cap_override(): get_video_info()'s override after the
 *   plugin's get_video_info: an encoder with OBS_ENCODER_CAP_SCALING gets the
 *   video's size, whatever the plugin set;
 * - obs_encoder_size_scaled_gate() and obs_encoder_size_scaled_match(): the
 *   refusals of obs_encoder_set_scaled_size() (not a video encoder, active,
 *   initialized, in that order), then its "matches the output, scaling
 *   disabled" rule;
 * - obs_encoder_size_scaling(): obs_encoder_scaling_enabled(), either scaled
 *   axis set;
 * - obs_encoder_size_video_route(): where obs_encoder_get_width() and
 *   obs_encoder_get_height() take one axis from;
 * - obs_encoder_size_rate_route(): where obs_encoder_get_sample_rate() takes
 *   the rate from.
 *
 * obs-encoder.c keeps obs_encoder_valid(), every warning with the encoder's
 * name, video_output_get_info(), audio_output_get_info(),
 * video_output_get_width() / _height(), audio_output_get_sample_rate(), the
 * plugin's get_audio_info and get_video_info between the steps, the load of
 * the active flag and every write to the encoder. It calls them and passes
 * the results here. Nothing here takes a lock, calls into libobs or keeps
 * state.
 *
 * The constants are copied below, so this header needs no core header;
 * obs-encoder.c checks them against obs-encoder.h and audio-io.h.
 *
 * Hidden in libobs: not EXPORT. */

/* OBS_ENCODER_AUDIO, OBS_ENCODER_VIDEO (enum obs_encoder_type) */
#define OBS_ENCODER_SIZE_TYPE_AUDIO 0
#define OBS_ENCODER_SIZE_TYPE_VIDEO 1
/* OBS_ENCODER_CAP_SCALING */
#define OBS_ENCODER_SIZE_CAP_SCALING (1 << 5)
/* AUDIO_FORMAT_UNKNOWN, SPEAKERS_UNKNOWN */
#define OBS_ENCODER_SIZE_AUDIO_FORMAT_UNKNOWN 0
#define OBS_ENCODER_SIZE_SPEAKERS_UNKNOWN 0

/* Where obs_encoder_get_width() / _height() take one axis from, and
 * obs_encoder_get_sample_rate() the rate. */
enum obs_encoder_size_source {
	/* not a video encoder (an audio one for the rate): a warning, 0 */
	OBS_ENCODER_SIZE_WRONG_TYPE = 0,
	/* no video (audio) set: 0 */
	OBS_ENCODER_SIZE_NO_MEDIA = 1,
	/* scaled_width / scaled_height (samplerate) */
	OBS_ENCODER_SIZE_STORED = 2,
	/* video_output_get_width() / _height() (audio_output_get_sample_rate())
	 * of the encoder's media */
	OBS_ENCODER_SIZE_MEDIA = 3,
};

/* What obs_encoder_set_scaled_size() does. obs_encoder_size_scaled_gate()
 * returns one of the first four, obs_encoder_size_scaled_match() one of the
 * last two. */
enum obs_encoder_scaled_action {
	/* not a video encoder: a warning, nothing set */
	OBS_ENCODER_SCALED_NOT_VIDEO = 0,
	/* the encoder is active: a warning, nothing set */
	OBS_ENCODER_SCALED_ACTIVE = 1,
	/* the encoder is initialized: a warning, nothing set */
	OBS_ENCODER_SCALED_INITIALIZED = 2,
	/* read the video's info, then obs_encoder_size_scaled_match() */
	OBS_ENCODER_SCALED_CHECK = 3,
	/* the size equals the video's: a warning, the scaled size set to 0 x 0 */
	OBS_ENCODER_SCALED_DISABLE = 4,
	/* the scaled size set to the size asked for */
	OBS_ENCODER_SCALED_SET = 5,
};

/* A width and a height. */
struct obs_encoder_size {
	uint32_t width;
	uint32_t height;
};

/* The fields of struct audio_convert_info and struct audio_output_info that
 * get_audio_info() fills: the enums as int. */
struct obs_encoder_audio_fill {
	/* enum audio_format */
	int format;
	uint32_t samples_per_sec;
	/* enum speaker_layout */
	int speakers;
};

/* get_audio_info() before the plugin's get_audio_info: info with an unknown
 * format, a 0 sample rate and unknown speakers each taken from mix (the audio
 * output's info), the other fields kept. */
struct obs_encoder_audio_fill obs_encoder_size_audio_defaults(const struct obs_encoder_audio_fill *info,
							      const struct obs_encoder_audio_fill *mix);

/* get_video_info() after the plugin's get_video_info: the video's size
 * (voi_width x voi_height) when caps has OBS_ENCODER_CAP_SCALING, else the
 * size the plugin left (width x height). */
struct obs_encoder_size obs_encoder_size_cap_override(uint32_t caps, uint32_t width, uint32_t height,
						      uint32_t voi_width, uint32_t voi_height);

/* obs_encoder_set_scaled_size() after obs_encoder_valid(): type is
 * info.type, active the loaded active flag, initialized the initialized
 * field. OBS_ENCODER_SCALED_NOT_VIDEO, _ACTIVE or _INITIALIZED (checked in
 * that order) refuse; OBS_ENCODER_SCALED_CHECK goes on. */
enum obs_encoder_scaled_action obs_encoder_size_scaled_gate(int type, bool active, bool initialized);

/* obs_encoder_set_scaled_size() after the gate: has_voi is
 * video_output_get_info() != NULL, voi_width x voi_height its size (read only
 * when has_voi), width x height the size asked for.
 * OBS_ENCODER_SCALED_DISABLE when they equal the video's, else
 * OBS_ENCODER_SCALED_SET. */
enum obs_encoder_scaled_action obs_encoder_size_scaled_match(bool has_voi, uint32_t voi_width, uint32_t voi_height,
							     uint32_t width, uint32_t height);

/* obs_encoder_scaling_enabled() after obs_encoder_valid(): either scaled
 * axis is not 0. */
bool obs_encoder_size_scaling(uint32_t scaled_width, uint32_t scaled_height);

/* One axis of obs_encoder_get_width() / _height() after obs_encoder_valid():
 * type is info.type, has_media whether the encoder has a video, scaled the
 * scaled size on that axis. */
enum obs_encoder_size_source obs_encoder_size_video_route(int type, bool has_media, uint32_t scaled);

/* obs_encoder_get_sample_rate() after obs_encoder_valid(): type is
 * info.type, has_media whether the encoder has an audio, samplerate the
 * stored rate. */
enum obs_encoder_size_source obs_encoder_size_rate_route(int type, bool has_media, uint32_t samplerate);
