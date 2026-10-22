/*
Copyright (C) 2014 by Leonhard Oelke <leonhard@in-verted.de>
Copyright (C) 2017 by Fabio Madia <admshao@gmail.com>

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
*/

#include "libobs/build/libobs-ported-pulse-format.pch.h"

enum speaker_layout pulseaudio_channels_to_obs_speakers(uint_fast32_t channels)
{
	switch (channels) {
	case 0:
		return SPEAKERS_UNKNOWN;
	case 1:
		return SPEAKERS_MONO;
	case 2:
		return SPEAKERS_STEREO;
	case 3:
		return SPEAKERS_2POINT1;
	case 4:
		return SPEAKERS_4POINT0;
	case 5:
		return SPEAKERS_4POINT1;
	case 6:
		return SPEAKERS_5POINT1;
	case 8:
		return SPEAKERS_7POINT1;
	default:
		return SPEAKERS_UNKNOWN;
	}
}

enum audio_format pulseaudio_to_obs_audio_format(pa_sample_format_t format)
{
	switch (format) {
	case PA_SAMPLE_U8:
		return AUDIO_FORMAT_U8BIT;
	case PA_SAMPLE_S16LE:
		return AUDIO_FORMAT_16BIT;
	case PA_SAMPLE_S32LE:
		return AUDIO_FORMAT_32BIT;
	case PA_SAMPLE_FLOAT32LE:
		return AUDIO_FORMAT_FLOAT;
	default:
		return AUDIO_FORMAT_UNKNOWN;
	}
}

pa_channel_map pulseaudio_channel_map(enum speaker_layout layout)
{
	pa_channel_map ret;

	ret.map[0] = PA_CHANNEL_POSITION_FRONT_LEFT;
	ret.map[1] = PA_CHANNEL_POSITION_FRONT_RIGHT;
	ret.map[2] = PA_CHANNEL_POSITION_FRONT_CENTER;
	ret.map[3] = PA_CHANNEL_POSITION_LFE;
	ret.map[4] = PA_CHANNEL_POSITION_REAR_LEFT;
	ret.map[5] = PA_CHANNEL_POSITION_REAR_RIGHT;
	ret.map[6] = PA_CHANNEL_POSITION_SIDE_LEFT;
	ret.map[7] = PA_CHANNEL_POSITION_SIDE_RIGHT;

	switch (layout) {
	case SPEAKERS_MONO:
		ret.channels = 1;
		ret.map[0] = PA_CHANNEL_POSITION_MONO;
		break;

	case SPEAKERS_STEREO:
		ret.channels = 2;
		break;

	case SPEAKERS_2POINT1:
		ret.channels = 3;
		ret.map[2] = PA_CHANNEL_POSITION_LFE;
		break;

	case SPEAKERS_4POINT0:
		ret.channels = 4;
		ret.map[3] = PA_CHANNEL_POSITION_REAR_CENTER;
		break;

	case SPEAKERS_4POINT1:
		ret.channels = 5;
		ret.map[4] = PA_CHANNEL_POSITION_REAR_CENTER;
		break;

	case SPEAKERS_5POINT1:
		ret.channels = 6;
		break;

	case SPEAKERS_7POINT1:
		ret.channels = 8;
		break;

	case SPEAKERS_UNKNOWN:
	default:
		ret.channels = 0;
		break;
	}

	return ret;
}

pa_sample_format_t pulseaudio_monitor_sink_format(pa_sample_format_t format)
{
	if (pulseaudio_to_obs_audio_format(format) == AUDIO_FORMAT_UNKNOWN)
		format = PA_SAMPLE_FLOAT32LE;

	return format;
}

uint8_t pulseaudio_monitor_sink_channels(uint8_t channels)
{
	if (pulseaudio_channels_to_obs_speakers(channels) == SPEAKERS_UNKNOWN)
		channels = 2;

	return channels;
}

/* The integer formats saturate to their range: above a volume of 1 a loud
 * sample leaves it, and converting it unclamped is undefined behaviour (it
 * wrapped). The float format is not clamped; the server clips it. Inside the
 * range the result is the truncated product, as before. */
static void process_byte(void *p, size_t frames, size_t channels, float vol)
{
	register uint8_t *cur = (uint8_t *)p;
	register uint8_t *end = cur + frames * channels;

	for (; cur < end; cur++) {
		float v = ((int)*cur - 128) * vol + 128;
		*cur = v >= 255.0f ? 255 : v <= 0.0f ? 0 : (uint8_t)v;
	}
}

static void process_s16(void *p, size_t frames, size_t channels, float vol)
{
	register int16_t *cur = (int16_t *)p;
	register int16_t *end = cur + frames * channels;

	for (; cur < end; cur++) {
		float v = *cur * vol;
		*cur = v >= (float)INT16_MAX ? INT16_MAX : v <= (float)INT16_MIN ? INT16_MIN : (int16_t)v;
	}
}

static void process_s32(void *p, size_t frames, size_t channels, float vol)
{
	register int32_t *cur = (int32_t *)p;
	register int32_t *end = cur + frames * channels;

	/* INT32_MAX is not a float: 2^31 is the first value past it, and
	 * -2^31 (INT32_MIN) is exact. */
	for (; cur < end; cur++) {
		float v = *cur * vol;
		*cur = v >= 2147483648.0f ? INT32_MAX : v <= -2147483648.0f ? INT32_MIN : (int32_t)v;
	}
}

static void process_float(void *p, size_t frames, size_t channels, float vol)
{
	register float *cur = (float *)p;
	register float *end = cur + frames * channels;

	while (cur < end)
		*(cur++) *= vol;
}

void pulseaudio_monitor_process_volume(pa_sample_format_t format, uint_fast8_t channels, float vol,
				       uint8_t *const *resample_data, uint32_t resample_frames)
{
	switch (format) {
	case PA_SAMPLE_U8:
		process_byte(resample_data[0], resample_frames, channels, vol);
		break;
	case PA_SAMPLE_S16LE:
		process_s16(resample_data[0], resample_frames, channels, vol);
		break;
	case PA_SAMPLE_S32LE:
		process_s32(resample_data[0], resample_frames, channels, vol);
		break;
	case PA_SAMPLE_FLOAT32LE:
		process_float(resample_data[0], resample_frames, channels, vol);
		break;
	default:
		// just ignore
		break;
	}
}

uint64_t pulseaudio_monitor_queued_bytes(int64_t write_index, int64_t read_index, uint64_t played_usec,
					 uint64_t bytes_per_sec, size_t unwritten)
{
	int64_t held = write_index - read_index;
	held -= (int64_t)util_mul_div64(played_usec, bytes_per_sec, 1000000ULL);
	return (held > 0 ? (uint64_t)held : 0) + unwritten;
}

struct pulseaudio_monitor_drop pulseaudio_monitor_drop_split(uint64_t excess_usec, uint64_t bytes_per_sec,
							     size_t bytes_per_frame, size_t unwritten)
{
	size_t bytes = (size_t)util_mul_div64(excess_usec, bytes_per_sec, 1000000ULL);
	bytes -= bytes % bytes_per_frame;

	struct pulseaudio_monitor_drop drop;
	drop.unwritten = bytes < unwritten ? bytes : unwritten;
	drop.rewind = bytes - drop.unwritten;
	return drop;
}

bool pulseaudio_monitor_devices_match(const char *id1, const char *id2, const char *default_sink)
{
	bool match;
	char *name1 = NULL;
	char *name2 = NULL;

	if (!id1 || !id2)
		return false;

	if (strcmp(id1, "default") == 0) {
		name1 = bzalloc(strlen(default_sink) + 9);
		strcat(name1, default_sink);
		strcat(name1, ".monitor");
	} else {
		name1 = bstrdup(id1);
	}

	if (strcmp(id2, "default") == 0) {
		name2 = bzalloc(strlen(default_sink) + 9);
		strcat(name2, default_sink);
		strcat(name2, ".monitor");
	} else {
		name2 = bzalloc(strlen(id2) + 9);
		strcat(name2, id2);
		strcat(name2, ".monitor");
	}

	match = strcmp(name1, name2) == 0;
	bfree(name1);
	bfree(name2);
	return match;
}
