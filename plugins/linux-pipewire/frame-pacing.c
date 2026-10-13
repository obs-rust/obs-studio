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

#include <string.h>

/* Frames wait in arrival order and each video tick moves on by at most one,
 * so two frames landing in one tick interval are drawn on consecutive ticks
 * instead of the first being dropped and the next tick repeating the second
 * (obsproject/obs-studio#13292). The backlog is bounded: a source faster
 * than the output loses its oldest waiting frame, keeping the drawn frame at
 * most OBS_PW_FRAME_PACER_MAX_FRAMES - 1 frames behind the newest. */

#define MAX_QUEUED (OBS_PW_FRAME_PACER_MAX_FRAMES - 1)

void obs_pw_frame_pacer_init(struct obs_pw_frame_pacer *pacer)
{
	pacer->queued = 0;
	pacer->shown = OBS_PW_FRAME_NONE;
}

static uint64_t pop_oldest(struct obs_pw_frame_pacer *pacer)
{
	uint64_t frame = pacer->queue[0];

	pacer->queued--;
	memmove(pacer->queue, pacer->queue + 1, pacer->queued * sizeof(pacer->queue[0]));
	return frame;
}

uint64_t obs_pw_frame_pacer_push(struct obs_pw_frame_pacer *pacer, uint64_t frame)
{
	uint64_t released = OBS_PW_FRAME_NONE;

	if (pacer->queued == MAX_QUEUED)
		released = pop_oldest(pacer);

	pacer->queue[pacer->queued++] = frame;
	return released;
}

uint64_t obs_pw_frame_pacer_tick(struct obs_pw_frame_pacer *pacer, uint64_t *released)
{
	*released = OBS_PW_FRAME_NONE;

	if (pacer->queued > 0) {
		*released = pacer->shown;
		pacer->shown = pop_oldest(pacer);
	}

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
