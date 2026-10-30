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

/* Ported to rust/obs-audio-monitoring (monitor_delay): the hybrid libobs drops
 * this file. */

/* The delay decisions of the WASAPI monitor's process_audio_delay()
 * (wasapi-output.c), moved: the stale cut and the front packet's verdict, on
 * plain integers. wasapi-output.c keeps the queue, the clock, every read and
 * the loop. Declared in monitor-delay.h without EXPORT. */

/* cut off audio if long-since leftover audio in delay buffer */
bool audio_monitor_delay_stale(uint64_t *last_recv_time, uint64_t cur_time)
{
	bool stale = cur_time - *last_recv_time > AUDIO_MONITOR_DELAY_STALE_NS;
	*last_recv_time = cur_time;
	return stale;
}

int64_t audio_monitor_delay_diff(uint64_t cur_ts, uint64_t pad_ns, uint64_t last_frame_ts, uint64_t *front_ts)
{
	*front_ts = cur_ts - pad_ns;
	return (int64_t)*front_ts - (int64_t)last_frame_ts;
}

bool audio_monitor_delay_unsynced(uint64_t last_frame_ts, int64_t diff)
{
	return !last_frame_ts || llabs(diff) > AUDIO_MONITOR_DELAY_MAX_DIFF_NS;
}

bool audio_monitor_delay_stalled(uint64_t time_since_prev)
{
	return time_since_prev > AUDIO_MONITOR_VIDEO_STALL_NS;
}

enum audio_monitor_delay_verdict audio_monitor_delay_judge_diff(bool bad_diff, int64_t diff)
{
	/* delay audio if rushing */
	if (!bad_diff && diff > AUDIO_MONITOR_DELAY_SYNC_NS)
		return AUDIO_MONITOR_DELAY_HOLD;

	/* cut audio if dragging */
	if (!bad_diff && diff < -AUDIO_MONITOR_DELAY_SYNC_NS)
		return AUDIO_MONITOR_DELAY_DRAG;

	return AUDIO_MONITOR_DELAY_PLAY;
}
