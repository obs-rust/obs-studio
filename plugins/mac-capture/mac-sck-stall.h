/*
 * This program is free software; you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation; either version 2 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program. If not, see <http://www.gnu.org/licenses/>.
 */

#pragma once

#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* How long a ScreenCaptureKit source may go without publishing a frame
 * before the stall is worth a log line: at 30 fps the stream is asked for
 * a frame every 0.9 x the output interval (~33 ms), so this is ~60 frame
 * intervals. */
#define SCK_STALL_WARN_MS 2000

/* A frozen source logs once per window, not once per tick. */
#define SCK_STALL_REWARN_MS 30000

enum sck_stall_cause {
	/* Nothing has been delivered since capture started. */
	SCK_STALL_NO_FRAMES = 0,
	/* The newest frame never reached the tick: no IOSurface, or the
	 * tick held the mutex while it arrived. */
	SCK_STALL_FRAMES_DROPPED,
	/* A frame was published and nothing has been refused since: the
	 * tick simply had no newer surface to show. */
	SCK_STALL_FRAMES_ARRIVED,
};

/* Says where a stall sits, from the two timestamps the capture path keeps:
 * the last published frame (last_delivery_ns) and the last frame the
 * delegate refused (last_drop_ns). Never delivered is SCK_STALL_NO_FRAMES.
 * A refusal newer than the newest delivery means the stream is producing
 * frames and we are refusing them, SCK_STALL_FRAMES_DROPPED. A delivery
 * newer than the newest refusal means the refusals are stale, so the
 * current gap is frames that stopped arriving: SCK_STALL_NO_FRAMES.
 * Nothing was ever refused and a frame did arrive is
 * SCK_STALL_FRAMES_ARRIVED. */
enum sck_stall_cause sck_stall_classify(uint64_t now_ns, uint64_t last_delivery_ns, uint64_t last_drop_ns);

/* True once threshold_ms has passed since the last published frame, and at
 * most once per rewarn_ms, so a frozen source does not flood the log. False
 * while last_delivery_ns is 0, which is the state before capture starts. */
bool sck_stall_should_warn(uint64_t now_ns, uint64_t last_delivery_ns, uint64_t warned_at_ns, unsigned int threshold_ms,
			   unsigned int rewarn_ms);

/* The cause as it appears in the log line, never NULL. */
const char *sck_stall_cause_name(enum sck_stall_cause cause);

#ifdef __cplusplus
}
#endif
