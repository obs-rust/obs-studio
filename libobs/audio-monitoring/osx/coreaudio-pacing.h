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
#include <stddef.h>
#include <stdint.h>

/* The queue pacing of the CoreAudio monitor (coreaudio-output.c), on plain
 * integers (coreaudio-pacing.c). The monitor plays through an AudioQueue of
 * COREAUDIO_MONITOR_BUFFERS buffers: a free buffer is filled whenever a whole
 * buffer of audio is queued, the queue pauses once every buffer has come back
 * unfilled, and it starts again when the wait size is queued:
 * - audio_monitor_init(): coreaudio_monitor_buffer_size() and
 *   coreaudio_monitor_wait_size();
 * - fill_buffer(): coreaudio_monitor_can_fill(), and after a failed
 *   AudioQueueEnqueueBuffer(), coreaudio_monitor_recover_failed_enqueue();
 * - on_audio_playback(): coreaudio_monitor_needs_volume() and
 *   coreaudio_monitor_wait_over();
 * - coreaudio_buffer_audio(): coreaudio_monitor_all_back().
 *
 * coreaudio-output.c keeps both deques (every push and pop), the mutex,
 * every AudioQueue call, the resample, the volume loop, the logs, the fill
 * loops and the paused flag, and reads each size where it did. Nothing here
 * includes an Apple header, so the C tests build it on every platform. Not
 * EXPORT: only libobs calls these. */

/* The AudioQueue buffers a monitor has. */
#define COREAUDIO_MONITOR_BUFFERS 3

/* Bytes in one AudioQueue buffer: 30 ms of packed float audio, in the order
 * the C computes it (channels * sizeof(float) * rate / 100 * 3). */
size_t coreaudio_monitor_buffer_size(uint32_t channels, uint32_t rate);

/* Bytes queued before a paused queue starts again: every buffer's worth. */
size_t coreaudio_monitor_wait_size(size_t buffer_size);

/* Whether queued bytes fill a buffer. */
bool coreaudio_monitor_can_fill(size_t queued, size_t buffer_size);

/* Whether the volume step runs: the volume is not 1 within EPSILON. */
bool coreaudio_monitor_needs_volume(float vol);

/* Whether the wait is over once queued bytes are queued: the wait size is
 * cleared, the free buffers filled and a paused queue started. */
bool coreaudio_monitor_wait_over(size_t queued, size_t wait_size);

/* Whether every buffer is back on the free list, which holds empty_bytes of
 * buffer pointers of ptr_size bytes: the queue pauses and waits for
 * coreaudio_monitor_wait_size() again. */
bool coreaudio_monitor_all_back(size_t empty_bytes, size_t ptr_size);

/* After AudioQueueEnqueueBuffer() failed and the queue was stopped: whether
 * the buffer goes back on the free list and the monitor takes the paused
 * state with coreaudio_monitor_wait_size(), so that later audio starts the
 * queue again. */
bool coreaudio_monitor_recover_failed_enqueue(void);
