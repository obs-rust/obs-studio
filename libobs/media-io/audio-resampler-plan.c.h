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

/* The output plan of the libswresample resampler (audio-resampler-ffmpeg.c),
 * moved: the channel mask of each speaker layout, the output channels,
 * whether the output is planar, the input channels, the 4.1 layouts and the
 * mono upmix gate. audio-resampler-ffmpeg.c keeps every libswresample and
 * libavutil call, and asks each where it read its input before.
 * Declared in audio-resampler-drift.h without EXPORT. */

uint64_t audio_resampler_layout_mask(enum speaker_layout layout)
{
	switch (layout) {
	case SPEAKERS_UNKNOWN:
		return 0;
	case SPEAKERS_MONO:
		return AUDIO_RESAMPLER_CH_LAYOUT_MONO;
	case SPEAKERS_STEREO:
		return AUDIO_RESAMPLER_CH_LAYOUT_STEREO;
	case SPEAKERS_2POINT1:
		return AUDIO_RESAMPLER_CH_LAYOUT_SURROUND;
	case SPEAKERS_4POINT0:
		return AUDIO_RESAMPLER_CH_LAYOUT_4POINT0;
	case SPEAKERS_4POINT1:
		return AUDIO_RESAMPLER_CH_LAYOUT_4POINT1;
	case SPEAKERS_5POINT1:
		return AUDIO_RESAMPLER_CH_LAYOUT_5POINT1_BACK;
	case SPEAKERS_7POINT1:
		return AUDIO_RESAMPLER_CH_LAYOUT_7POINT1;
	}

	/* shouldn't get here */
	return 0;
}

uint32_t audio_resampler_output_channels(enum speaker_layout dst)
{
	return get_audio_channels(dst);
}

bool audio_resampler_planar_output(enum audio_format dst_format)
{
	return is_audio_planar(dst_format);
}

int audio_resampler_input_channels(enum speaker_layout src)
{
	return (int)get_audio_channels(src);
}

bool audio_resampler_is_4point1(enum speaker_layout layout)
{
	return layout == SPEAKERS_4POINT1;
}

bool audio_resampler_upmixes_mono(uint32_t output_ch)
{
	return output_ch > 1;
}
