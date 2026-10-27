/*
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

#pragma once

#include <stdint.h>

#include "../media-io/audio-io.h"

/* The WASAPI monitor plays in the device's mix format and resamples OBS's
 * audio to the speaker layout of that format. The device describes it by a
 * channel mask (dwChannelMask, a KSAUDIO_SPEAKER_* value) and a channel
 * count (nChannels). Kept free of Windows headers so it can be tested on
 * every platform: the masks below are the ksmedia.h values, and
 * wasapi-output.c checks them against it. */
#define AUDIO_MONITOR_SPEAKER_MASK_2POINT1 0x0000000BU /* FL FR LFE */
#define AUDIO_MONITOR_SPEAKER_MASK_SURROUND 0x00000107U /* FL FR FC BC */
#define AUDIO_MONITOR_SPEAKER_MASK_4POINT1 0x0000010FU /* FL FR FC LFE BC */
#define AUDIO_MONITOR_SPEAKER_MASK_5POINT1 0x0000003FU /* FL FR FC LFE BL BR */
#define AUDIO_MONITOR_SPEAKER_MASK_7POINT1 0x000000FFU /* FL FR FC LFE BL BR FLC FRC */

/* The layout to resample to for a device with this mask and channel count:
 * the mask's layout, or for any other mask the layout with that many
 * channels. The monitor copies channels floats per frame, so a layout with
 * another channel count is never returned: SPEAKERS_UNKNOWN when there is
 * none (0, 7 or more than 8 channels, or a mask that disagrees with the
 * count). */
static inline enum speaker_layout audio_monitor_speaker_layout(uint32_t mask, uint32_t channels)
{
	enum speaker_layout layout;

	switch (mask) {
	case AUDIO_MONITOR_SPEAKER_MASK_2POINT1:
		layout = SPEAKERS_2POINT1;
		break;
	case AUDIO_MONITOR_SPEAKER_MASK_SURROUND:
		layout = SPEAKERS_4POINT0;
		break;
	case AUDIO_MONITOR_SPEAKER_MASK_4POINT1:
		layout = SPEAKERS_4POINT1;
		break;
	case AUDIO_MONITOR_SPEAKER_MASK_5POINT1:
		layout = SPEAKERS_5POINT1;
		break;
	case AUDIO_MONITOR_SPEAKER_MASK_7POINT1:
		layout = SPEAKERS_7POINT1;
		break;
	default:
		/* SPEAKERS_MONO to SPEAKERS_5POINT1 are 1 to 6 and
		 * SPEAKERS_7POINT1 is 8; there is no layout 7. */
		layout = channels <= 8 ? (enum speaker_layout)channels : SPEAKERS_UNKNOWN;
		break;
	}

	return get_audio_channels(layout) == channels ? layout : SPEAKERS_UNKNOWN;
}
