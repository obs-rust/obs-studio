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

/* Private adapter, not a new public API: the clock read and stop helpers
 * that the transition renders call (obs-source-transition-stop.c).
 *
 * - transition_calc_time(): the transition's position at ts, from its mode,
 *   manual value, start time and duration
 *   (obs_source_transition_clock_time()). Reads the clock fields: called
 *   with transition_mutex held.
 * - transition_get_video_time(): transition_calc_time() at the latest video
 *   frame's time, which it reads under obs->video.stats.mutex. Called with
 *   transition_mutex held.
 * - transition_stop(): the end of a transition: slot A's child is released
 *   (and removed as an active child if it was one), slot B's child becomes
 *   slot A, active, and slot B empties. Called with transition_mutex held.
 * - transition_handle_stop(): the transition_stop callback, when the type
 *   has one, then the transition_stop and source_transition_stop signals.
 *   Called after transition_mutex is let go.
 *
 * Hidden in libobs: not EXPORT. */

float transition_calc_time(obs_source_t *transition, uint64_t ts);
float transition_get_video_time(obs_source_t *transition);
void transition_stop(obs_source_t *transition);
void transition_handle_stop(obs_source_t *transition);
