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

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

/* The start checks and child handover of the transitions in
 * obs-source-transition.c, on plain values (obs-source-transition-start.c):
 * - obs_transition_start(): the first refusal (the destination is already
 *   source A and nothing is running), whether the recursion walk runs, the
 *   second admission (refused, already there in manual mode, or go), the
 *   duration in ns and the null destination that starts a transition;
 * - set_source(): the clock restart rule, whether the new child is added,
 *   the old one removed, the admission callback called, and whether the
 *   transition's size is recalculated (a size of 0);
 * - activate_transition(): the child is added only while its slot is not
 *   active yet;
 * - obs_transition_get_active_source() and obs_transition_save(): which
 *   child is the active one (B while transitioning, else A);
 * - obs_transition_set_manual_time(): a manual time of 0 sets source A;
 * - obs_transition_set() and copy_source_state(): whether a child goes out
 *   or comes in as an active child.
 *
 * Each decision is its own function, asked at its own point, so
 * obs-source-transition.c keeps every read where it was. obs_transition_start
 * reads the sources in three separate lock sections on purpose (the plugin's
 * transition_start callback and obs_transition_set() can change them in
 * between), and each of these functions gets only what one section read. It
 * also keeps the locks, the pointer comparisons, transition_active() (the
 * video flag is read only when the audio flag is not set), the fixed
 * duration read (only when it is enabled), the size reads (cy only when cx is
 * not 0), every obs_source_get_ref() and obs_source_release(), the active
 * child calls, the recursion walk, the plugin callbacks, the signals, the
 * size and matrix recalculation, os_gettime_ns() and the logs. None of these
 * functions takes a lock, calls into libobs, touches memory or keeps state.
 *
 * The mode crosses as int, so this header needs no other libobs header;
 * obs-source-transition.c checks that the constant below is
 * OBS_TRANSITION_MODE_MANUAL and that enum obs_transition_mode is int-sized.
 *
 * Hidden in libobs: not EXPORT. */

/* OBS_TRANSITION_MODE_MANUAL (enum obs_transition_mode, obs.h). */
#define OBS_SOURCE_TRANSITION_START_MODE_MANUAL 1

/* What obs_transition_start() does after its second read of the sources. */
enum obs_source_transition_start_admission {
	/* go on: write the mode, call the plugin and set source B */
	OBS_SOURCE_TRANSITION_START_GO,
	/* return false: the destination is source A and nothing is running */
	OBS_SOURCE_TRANSITION_START_REFUSE,
	/* return true: a manual transition already running to this
	 * destination in manual mode */
	OBS_SOURCE_TRANSITION_START_DONE,
};

/* Whether a start is refused: the destination is source A
 * (same_as_source) and the transition is not active. obs_transition_start()
 * asks it after its first read of the sources. */
bool obs_source_transition_start_refused(bool same_as_source, bool active);

/* Whether the recursion walk of the destination's tree runs: there is a
 * destination and it is not the transition itself (recursive, the walk's
 * start value). obs_transition_set() asks it for its new source too. */
bool obs_source_transition_start_walks(bool has_dest, bool recursive);

/* The second admission of obs_transition_start(), from its second read of
 * the sources: REFUSE as obs_source_transition_start_refused(); DONE when
 * active, the requested mode is manual, the mode is the same and the
 * destination is already source B; else GO. */
enum obs_source_transition_start_admission obs_source_transition_start_admit(bool same_as_source, bool same_as_dest,
									     bool same_mode, int mode, bool active);

/* A duration in ms as the start clock's ns: (uint64_t)ms * 1000000. */
uint64_t obs_source_transition_start_duration_ns(uint32_t ms);

/* Whether set_source() restarts the clock: the transition is not active,
 * or the new child is neither the old one (same_as_dest) nor source A
 * (same_as_source). */
bool obs_source_transition_start_restart(bool active, bool same_as_dest, bool same_as_source);

/* Whether set_source() adds the new child as an active child: its slot is
 * already active and there is a new child. */
bool obs_source_transition_start_adds(bool already_active, bool has_new);

/* Whether set_source() removes the old child as an active child: its slot
 * is already active, there is an old child and the add (if any) worked. */
bool obs_source_transition_start_removes(bool already_active, bool has_old, bool add_ok);

/* Whether set_source() calls its admission callback: there is one and the
 * add (if any) worked. */
bool obs_source_transition_start_calls_back(bool has_callback, bool add_ok);

/* Whether a transition size leaves the size to the children: size == 0.
 * set_source() asks it for cx, and for cy only when cx is not 0. */
bool obs_source_transition_start_unsized(uint32_t size);

/* Whether a start to no destination sets both transitioning flags itself:
 * no destination, source B was already none (same_as_dest) and source A is
 * not none (!same_as_source). */
bool obs_source_transition_start_null_dest(bool has_dest, bool same_as_dest, bool same_as_source);

/* Whether activate_transition() adds the child: its slot is not active
 * yet. */
bool obs_source_transition_start_activates(bool slot_active);

/* The index of the active child: 1 (source B) while the transition is
 * active, else 0 (source A). */
size_t obs_source_transition_start_active_index(bool active);

/* Whether obs_transition_set_manual_time() sets source A: the mode is
 * manual and t == 0.0f (true for -0.0f, false for NaN). */
bool obs_source_transition_start_manual_reset(int mode, float t);

/* Whether a child goes out (obs_transition_set()) or comes in
 * (copy_source_state()) as an active child: there is one and its slot is
 * active. */
bool obs_source_transition_start_child_active(bool has_child, bool active);
