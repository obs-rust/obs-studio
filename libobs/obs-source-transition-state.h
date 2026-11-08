/******************************************************************************
    Copyright (C) 2023 by Lain Bailey <lain@obsproject.com>

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
******************************************************************************/

#pragma once

#include "obs-internal.h"

/* Private adapter, not a new public API: what the transition renders copy
 * under transition_mutex before they let it go (obs-source-transition-video.c
 * for the video, obs-source-transition.c for the audio): both children,
 * referenced, and the two transitioning flags. The video renders leave the
 * clock fields unset, and read none of them.
 *
 * Hidden in libobs: not EXPORT. */

struct transition_state {
	obs_source_t *s[2];
	bool transitioning_video;
	bool transitioning_audio;

	/* The clock, copied under the lock for an audio block (#3499): the mix
	 * runs after the lock is dropped, and a write then must not split the
	 * block between two positions. */
	enum obs_transition_mode mode;
	float manual_val;
	uint64_t start_time;
	uint64_t duration;
};
