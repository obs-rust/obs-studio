/* frame-pacing.h
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

#pragma once

#include <stddef.h>
#include <stdint.h>

/* Which captured frame a synchronous PipeWire source (screen capture) draws
 * on each OBS video tick, kept free of PipeWire and graphics calls so it can
 * be tested on its own.
 *
 * Frames are opaque ids chosen by the caller (pipewire.c uses the index of
 * the frame's texture). Every frame passed to obs_pw_frame_pacer_push() is
 * handed back exactly once as a released frame, from push, tick or flush;
 * only then may the caller free or reuse it. */

#define OBS_PW_FRAME_NONE UINT64_MAX

/* At most this many frames are held, including the one being drawn. */
#define OBS_PW_FRAME_PACER_MAX_FRAMES 3

struct obs_pw_frame_pacer {
	uint64_t queue[OBS_PW_FRAME_PACER_MAX_FRAMES];
	size_t queued;
	uint64_t shown;
};

void obs_pw_frame_pacer_init(struct obs_pw_frame_pacer *pacer);

/* A frame arrived from PipeWire. Returns a frame the pacer no longer needs,
 * or OBS_PW_FRAME_NONE. */
uint64_t obs_pw_frame_pacer_push(struct obs_pw_frame_pacer *pacer, uint64_t frame);

/* An OBS video tick is about to draw the source. Returns the frame to draw,
 * or OBS_PW_FRAME_NONE while nothing has arrived. Sets *released to a frame
 * the pacer no longer needs, or OBS_PW_FRAME_NONE. */
uint64_t obs_pw_frame_pacer_tick(struct obs_pw_frame_pacer *pacer, uint64_t *released);

/* The stream stops. Writes every held frame to released[] (room for
 * OBS_PW_FRAME_PACER_MAX_FRAMES + 1) and returns how many. */
size_t obs_pw_frame_pacer_flush(struct obs_pw_frame_pacer *pacer, uint64_t *released);
