/* frame-pacing.c
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 2 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <http://www.gnu.org/licenses/>.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later
 */

#include "frame-pacing.h"

/* Not implemented yet: this is today's behaviour in pipewire.c
 * (find_latest_buffer() and process_video_sync()). Every arriving buffer
 * replaces the texture at once and each video tick draws whatever arrived
 * last. When the compositor's frames land close to OBS's tick with a little
 * jitter, one tick sees two new frames (one is never drawn) and the next sees
 * none (the previous one is drawn twice): obsproject/obs-studio#13292. */

void obs_pw_frame_pacer_init(struct obs_pw_frame_pacer *pacer)
{
	pacer->queued = 0;
	pacer->shown = OBS_PW_FRAME_NONE;
}

uint64_t obs_pw_frame_pacer_push(struct obs_pw_frame_pacer *pacer, uint64_t frame)
{
	uint64_t released = pacer->shown;

	pacer->shown = frame;
	return released;
}

uint64_t obs_pw_frame_pacer_tick(struct obs_pw_frame_pacer *pacer, uint64_t *released)
{
	*released = OBS_PW_FRAME_NONE;
	return pacer->shown;
}

size_t obs_pw_frame_pacer_flush(struct obs_pw_frame_pacer *pacer, uint64_t *released)
{
	size_t count = 0;

	for (size_t i = 0; i < pacer->queued; i++)
		released[count++] = pacer->queue[i];
	if (pacer->shown != OBS_PW_FRAME_NONE)
		released[count++] = pacer->shown;

	obs_pw_frame_pacer_init(pacer);
	return count;
}
