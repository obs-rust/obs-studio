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

/* The audio monitor holds a source's audio back, or cuts it, to keep it in
 * sync with the source's video. A source whose video stands still must not
 * hold its audio for good: an audio file with embedded cover art shows one
 * picture and then no more frames, so its last video timestamp stays where
 * the picture was while the audio runs on.
 *
 * Called for every audio packet the monitor receives, with the timestamp of
 * the source's last video frame and the packet's duration in nanoseconds.
 * *prev_video_ts is the video timestamp seen so far (0 before the first
 * packet), *time_since_prev how long it has stood still. Once that is more
 * than AUDIO_MONITOR_VIDEO_STALL_NS, the monitor plays the audio without
 * syncing it to the video. */
#define AUDIO_MONITOR_VIDEO_STALL_NS 100000000ULL

static inline void audio_monitor_track_video_ts(uint64_t *prev_video_ts, uint64_t *time_since_prev,
						uint64_t last_frame_ts, uint64_t packet_ns)
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
