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

#include "libobs/build/libobs-ported-monitor-delay.pch.h"

/* Ported to rust/obs-audio-monitoring (speaker_layout): the hybrid libobs
 * drops this file. */

/* Declared in monitor-speaker-layout.h without EXPORT. */

enum speaker_layout audio_monitor_speaker_layout(uint32_t mask, uint32_t channels)
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
