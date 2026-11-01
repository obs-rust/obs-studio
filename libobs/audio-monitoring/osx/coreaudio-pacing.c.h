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

#include "libobs/build/libobs-ported-coreaudio-pacing.pch.h"

/* Ported to rust/obs-audio-monitoring (coreaudio_pacing): the hybrid libobs
 * drops this file. */

/* The queue pacing of the CoreAudio monitor (coreaudio-output.c), moved: the
 * buffer and wait sizes, the fill test, the volume gate, the end of the wait,
 * the pause once every buffer is back, and what follows a failed enqueue, on
 * plain integers. coreaudio-output.c keeps the deques, the mutex, every
 * AudioQueue call and the loops. Declared in coreaudio-pacing.h without
 * EXPORT. */

size_t coreaudio_monitor_buffer_size(uint32_t channels, uint32_t rate)
{
	return channels * sizeof(float) * rate / 100 * 3;
}

size_t coreaudio_monitor_wait_size(size_t buffer_size)
{
	return buffer_size * COREAUDIO_MONITOR_BUFFERS;
}

bool coreaudio_monitor_can_fill(size_t queued, size_t buffer_size)
{
	return queued >= buffer_size;
}

bool coreaudio_monitor_needs_volume(float vol)
{
	return !close_float(vol, 1.0f, EPSILON);
}

bool coreaudio_monitor_wait_over(size_t queued, size_t wait_size)
{
	return queued >= wait_size;
}

bool coreaudio_monitor_all_back(size_t empty_bytes, size_t ptr_size)
{
	return empty_bytes == ptr_size * COREAUDIO_MONITOR_BUFFERS;
}

bool coreaudio_monitor_recover_failed_enqueue(void)
{
	/* The buffer stays out and the queue stopped. */
	return false;
}
