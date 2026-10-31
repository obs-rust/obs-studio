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

#include "libobs/build/libobs-monitor-wasapi.pch.h"

/* Declared in monitor-video-sync.h without EXPORT. */

void audio_monitor_track_video_ts(uint64_t *prev_video_ts, uint64_t *time_since_prev, uint64_t last_frame_ts,
				  uint64_t packet_ns)
{
	if (!*prev_video_ts) {
		*prev_video_ts = last_frame_ts;

	} else if (*prev_video_ts == last_frame_ts) {
		*time_since_prev += packet_ns;
	} else {
		*prev_video_ts = last_frame_ts;
		*time_since_prev = 0;
	}
}
