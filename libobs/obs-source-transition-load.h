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
#include <stdint.h>

/* The load, scale type and swap checks of the transitions in
 * obs-source-transition.c, on plain values (obs-source-transition-load.c):
 * - transition_valid(): the type check after obs_ptr_valid();
 * - obs_transition_set_scale_type() and obs_transition_load(): the three
 *   valid scale types, else Max Only;
 * - obs_transition_load(): whether the saved source is looked up (it has a
 *   name), and what the lookup and the add come to (keep it, not found, or
 *   refused for recursion);
 * - copy_source_state(): whether the new child is stored, or refused for
 *   recursion;
 * - obs_transition_swap_begin() and obs_transition_swap_end(): the swap of a
 *   transition with itself does nothing.
 *
 * Each decision is its own function, asked at its own point, so
 * obs-source-transition.c keeps every read and call where it was and under
 * its condition: obs_ptr_valid(), the locks and their order, the pointer
 * comparisons, the obs_data reads, obs_get_source_by_name(), the add only
 * for a found source or an active new child, every obs_source_get_ref() and
 * obs_source_release(), the active child calls, the (uint32_t) truncations
 * of the loaded alignment and size, the mode taken as is, the fields the
 * swap hands over, the texrender swap, the size and matrix recalculation,
 * and the logs. The getters' answers for an invalid transition (0, 0x0,
 * false and Max Only) stay there as the literals they are. Whether the load
 * takes the old source A out of the active children is
 * obs_source_transition_start_child_active(), the rule copy_source_state()
 * and obs_transition_set() already ask. None of these functions takes a
 * lock, calls into libobs, touches memory or keeps state.
 *
 * The source type and the scale type cross as int (the loaded scale type as
 * the int64_t obs_data holds), so this header needs no other libobs header;
 * obs-source-transition.c checks that the constants below are the libobs
 * values and that both enums are int-sized.
 *
 * Hidden in libobs: not EXPORT. */

/* OBS_SOURCE_TYPE_TRANSITION (enum obs_source_type, obs-source.h). */
#define OBS_SOURCE_TRANSITION_LOAD_TYPE_TRANSITION 2

/* enum obs_transition_scale_type (obs.h). */
#define OBS_SOURCE_TRANSITION_LOAD_SCALE_MAX_ONLY 0
#define OBS_SOURCE_TRANSITION_LOAD_SCALE_ASPECT 1
#define OBS_SOURCE_TRANSITION_LOAD_SCALE_STRETCH 2

/* What obs_transition_load() does with the source it looked up. */
enum obs_source_transition_load_outcome {
	/* store it in slot A */
	OBS_SOURCE_TRANSITION_LOAD_KEEP,
	/* warn that it was not found, and store none */
	OBS_SOURCE_TRANSITION_LOAD_NOT_FOUND,
	/* warn of the recursion, release it and store none: the add failed,
	 * because it contains the transition or is the transition */
	OBS_SOURCE_TRANSITION_LOAD_REFUSE,
};

/* Whether a source of this type is a transition: type ==
 * OBS_SOURCE_TYPE_TRANSITION. transition_valid() asks it once
 * obs_ptr_valid() passed. */
bool obs_source_transition_load_is_transition(int type);

/* A scale type as stored: MAX_ONLY, ASPECT and STRETCH as they are, any
 * other value (from a scene collection or a caller) MAX_ONLY. */
int obs_source_transition_load_scale_type(int64_t type);

/* Whether obs_transition_load() looks its saved source up: it has a name
 * (the string is not NULL). */
bool obs_source_transition_load_looks_up(bool has_name);

/* What obs_transition_load() does with the looked-up source, after the add
 * (asked only for a found source): NOT_FOUND when none was found, REFUSE
 * when the add failed, else KEEP. */
enum obs_source_transition_load_outcome obs_source_transition_load_source(bool found, bool add_ok);

/* Whether obs_transition_swap_begin() and obs_transition_swap_end() do
 * anything: not for a transition swapped with itself (same). */
bool obs_source_transition_load_swaps(bool same);

/* Whether copy_source_state() stores the new child (and its slot's flag):
 * it was not added as an active child (adds false), or the add worked.
 * Otherwise it releases the child and stores none, inactive. */
bool obs_source_transition_load_copy_stores(bool adds, bool add_ok);
