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

/* The clock and fit decisions of the transitions in obs-source-transition.c,
 * on plain values (obs-source-transition-clock.c):
 * - obs_source_transition_clock_time(): calc_time(), a transition's position
 *   from 0 to 1 at a timestamp. The manual value in manual mode; otherwise 0
 *   up to the start, 1 from the end on (and for a duration of 0), and the
 *   elapsed time over the duration, divided in long double, in between.
 * - obs_source_transition_clock_manual(): the manual step of
 *   obs_transition_tick(). Nothing outside manual mode; in it the value
 *   snaps to the target when the torque is 0, and otherwise steps with
 *   calc_torquef(), which obs_transition_tick() calls.
 * - obs_source_transition_clock_size() and _fold(): get_cx() and get_cy()
 *   (the size set, or the actual size when none is set), and the size fold
 *   of recalculate_transition_size() (the larger of the two children).
 * - obs_source_transition_clock_fit(): the scale of
 *   recalculate_transition_matrix(): none for a child with no size; for
 *   max-only, 1 when the child fits, else aspect; for aspect, the smaller
 *   ratio of the two sides; for stretch, each side's ratio.
 * - obs_source_transition_clock_mix_spaces(): mix_spaces(), the colour space
 *   two children are rendered in.
 * - obs_source_transition_clock_min_ts(): transition_calc_min_ts(), the
 *   earliest of the children's audio timestamps that are not 0.
 *
 * obs-source-transition.c keeps the locks, the field reads and writes, the
 * child size and timestamp reads, calc_torquef(), the offsets passed to
 * add_alignment() and add_alignment() itself, the matrix build,
 * get_sample_time(), the texrenders and the swap handover.
 *
 * None of them takes a lock, calls into libobs or keeps state.
 *
 * The modes, scale types and colour spaces are enum obs_transition_mode,
 * enum obs_transition_scale_type and enum gs_color_space values passed as
 * int, so this header needs neither obs.h nor a graphics header;
 * obs-source-transition.c checks the sizes and the values.
 *
 * Hidden in libobs: not EXPORT. */

/* obs-source-transition.c checks that they are OBS_TRANSITION_MODE_MANUAL,
 * enum obs_transition_scale_type's and enum gs_color_space's values */
#define OBS_SOURCE_TRANSITION_CLOCK_MODE_MANUAL 1
#define OBS_SOURCE_TRANSITION_CLOCK_SCALE_MAX_ONLY 0
#define OBS_SOURCE_TRANSITION_CLOCK_SCALE_ASPECT 1
#define OBS_SOURCE_TRANSITION_CLOCK_SCALE_STRETCH 2
#define OBS_SOURCE_TRANSITION_CLOCK_CS_SRGB 0
#define OBS_SOURCE_TRANSITION_CLOCK_CS_SRGB_16F 1
#define OBS_SOURCE_TRANSITION_CLOCK_CS_709_EXTENDED 2
#define OBS_SOURCE_TRANSITION_CLOCK_CS_709_SCRGB 3

/* What obs_transition_tick() does with transition_manual_val. */
enum obs_source_transition_clock_step {
	/* not manual: the value is left as it is */
	OBS_SOURCE_TRANSITION_CLOCK_HOLD,
	/* the value becomes transition_manual_target */
	OBS_SOURCE_TRANSITION_CLOCK_SNAP,
	/* the value becomes calc_torquef(value, target, torque, clamp, t) */
	OBS_SOURCE_TRANSITION_CLOCK_STEP,
};

/* The transition's position at ts: mode is transition_mode, manual_val
 * transition_manual_val, start_time transition_start_time and duration
 * transition_duration. */
float obs_source_transition_clock_time(int mode, float manual_val, uint64_t start_time, uint64_t duration, uint64_t ts);

/* The manual step: mode is transition_mode and torque
 * transition_manual_torque. */
enum obs_source_transition_clock_step obs_source_transition_clock_manual(int mode, float torque);

/* get_cx() and get_cy(): set is transition_cx (or _cy), actual
 * transition_actual_cx (or _cy). */
uint32_t obs_source_transition_clock_size(uint32_t set, uint32_t actual);

/* One step of the size fold: size so far and a child's width (or height). */
uint32_t obs_source_transition_clock_fold(uint32_t size, uint32_t child);

/* The scale of a child of source_cx by source_cy in a transition of tr_cx by
 * tr_cy (both get_cx() and get_cy() as float), with scale_type
 * (transition_scale_type). False, and nothing written, for a child with a
 * width or height of 0: no matrix. Otherwise true, with the scale in
 * *scale_x and *scale_y; a scale type outside the enum writes neither. */
bool obs_source_transition_clock_fit(float tr_cx, float tr_cy, float source_cx, float source_cy, int scale_type,
				     float *scale_x, float *scale_y);

/* The colour space for two children's spaces a and b. */
int obs_source_transition_clock_mix_spaces(int a, int b);

/* The earliest of the two audio timestamps that is not 0; 0 when both are.
 * A child that has none (no child, or its audio is pending) passes 0. */
uint64_t obs_source_transition_clock_min_ts(uint64_t a, uint64_t b);
