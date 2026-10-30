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

#include <stdbool.h>
#include <stdint.h>

/* The delay decisions of the WASAPI monitor's process_audio_delay()
 * (wasapi-output.c), on plain integers (monitor-delay.c). A monitored source
 * with video has its audio queued, and each packet at the front of the queue
 * is held back, cut or played to stay in step with the source's video:
 * - audio_monitor_delay_stale(): the stale cut. More than
 *   AUDIO_MONITOR_DELAY_STALE_NS since the last packet arrived empties the
 *   queue; the receive time is stored either way;
 * - the front packet's verdict, in the steps process_audio_delay() takes:
 *   audio_monitor_delay_diff() (its timestamp less the device padding, and
 *   the difference from the source's last video timestamp), the "bad diff"
 *   gate in two parts, audio_monitor_delay_unsynced() (no video timestamp
 *   yet, or more than 5 s apart) and audio_monitor_delay_stalled() (the
 *   video stood still), then audio_monitor_delay_judge_diff().
 *
 * wasapi-output.c keeps the queue (every deque call), the clock, the reads of
 * the source and the monitor (the stall count only when the first part of
 * the gate does not decide it, as before), both util_mul_div64() calls, the
 * "more are queued" test after the pops, the logs and the loop. Nothing here
 * includes a Windows header, so the C tests build it on every platform. Not
 * EXPORT: only libobs calls these. */

/* More than this since the last packet arrived: the queue is stale. */
#define AUDIO_MONITOR_DELAY_STALE_NS 1000000000

/* Audio more than this away from the video is not synced to it. */
#define AUDIO_MONITOR_DELAY_MAX_DIFF_NS 5000000000

/* Audio more than this ahead of the video waits; more than this behind it is
 * cut while more is queued. */
#define AUDIO_MONITOR_DELAY_SYNC_NS 75000000

enum audio_monitor_delay_verdict {
	/* play the front packet */
	AUDIO_MONITOR_DELAY_PLAY = 0,
	/* rushing: play nothing now and keep the queue */
	AUDIO_MONITOR_DELAY_HOLD = 1,
	/* dragging: drop the front packet if more are queued after it */
	AUDIO_MONITOR_DELAY_DRAG = 2,
};

/* Whether the queue is stale at cur_time, the packet's arrival; stores
 * cur_time in *last_recv_time. */
bool audio_monitor_delay_stale(uint64_t *last_recv_time, uint64_t cur_time);

/* The packet at the front of the queue, with timestamp cur_ts, when the
 * device still has pad_ns of audio to play and the source's last video frame
 * is at last_frame_ts: writes its timestamp less the padding to *front_ts and
 * returns their difference. */
int64_t audio_monitor_delay_diff(uint64_t cur_ts, uint64_t pad_ns, uint64_t last_frame_ts, uint64_t *front_ts);

/* The first part of the "bad diff" gate: no video frame yet (last_frame_ts
 * 0), or audio and video more than AUDIO_MONITOR_DELAY_MAX_DIFF_NS apart. */
bool audio_monitor_delay_unsynced(uint64_t last_frame_ts, int64_t diff);

/* The second part: the video has stood still for more than
 * AUDIO_MONITOR_VIDEO_STALL_NS (time_since_prev, monitor-video-sync.h). */
bool audio_monitor_delay_stalled(uint64_t time_since_prev);

/* The front packet's verdict from the gate and the difference. */
enum audio_monitor_delay_verdict audio_monitor_delay_judge_diff(bool bad_diff, int64_t diff);
