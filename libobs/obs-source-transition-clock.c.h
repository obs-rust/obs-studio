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

#include "libobs/build/libobs-ported-source-transition-clock.pch.h"

/* The clock and fit decisions of the transitions, moved from
 * obs-source-transition.c: calc_time(), the manual step of
 * obs_transition_tick(), get_cx() and get_cy(), the size fold of
 * recalculate_transition_size(), the scale of
 * recalculate_transition_matrix(), mix_spaces() and transition_calc_min_ts(),
 * on plain values. obs-source-transition.c keeps the locks, the field reads
 * and writes, the child reads, calc_torquef(), the offsets, add_alignment(),
 * the matrix build and get_sample_time(). Declared in
 * obs-source-transition-clock.h without EXPORT. In hybrid libobs the Rust
 * port (rust/obs-source) replaces this file. */

float obs_source_transition_clock_time(int mode, float manual_val, uint64_t start_time, uint64_t duration, uint64_t ts)
{
	if (mode == OBS_SOURCE_TRANSITION_CLOCK_MODE_MANUAL)
		return manual_val;

	uint64_t end;

	if (ts <= start_time)
		return 0.0f;

	end = duration;
	ts -= start_time;
	if (ts >= end || end == 0)
		return 1.0f;

	return (float)((long double)ts / (long double)end);
}

enum obs_source_transition_clock_step obs_source_transition_clock_manual(int mode, float torque)
{
	if (mode != OBS_SOURCE_TRANSITION_CLOCK_MODE_MANUAL)
		return OBS_SOURCE_TRANSITION_CLOCK_HOLD;

	return torque == 0.0f ? OBS_SOURCE_TRANSITION_CLOCK_SNAP : OBS_SOURCE_TRANSITION_CLOCK_STEP;
}

uint32_t obs_source_transition_clock_size(uint32_t set, uint32_t actual)
{
	return set ? set : actual;
}

uint32_t obs_source_transition_clock_fold(uint32_t size, uint32_t child)
{
	if (child > size)
		size = child;
	return size;
}

bool obs_source_transition_clock_fit(float tr_cx, float tr_cy, float source_cx, float source_cy, int scale_type,
				     float *scale_x, float *scale_y)
{
	float tr_aspect = tr_cx / tr_cy;
	float source_aspect;

	if (source_cx == 0.0f || source_cy == 0.0f)
		return false;

	source_aspect = source_cx / source_cy;

	if (scale_type == OBS_SOURCE_TRANSITION_CLOCK_SCALE_MAX_ONLY) {
		if (source_cx > tr_cx || source_cy > tr_cy) {
			scale_type = OBS_SOURCE_TRANSITION_CLOCK_SCALE_ASPECT;
		} else {
			*scale_x = 1.0f;
			*scale_y = 1.0f;
		}
	}

	if (scale_type == OBS_SOURCE_TRANSITION_CLOCK_SCALE_ASPECT) {
		bool use_width = tr_aspect < source_aspect;
		*scale_x = *scale_y = use_width ? tr_cx / source_cx : tr_cy / source_cy;

	} else if (scale_type == OBS_SOURCE_TRANSITION_CLOCK_SCALE_STRETCH) {
		*scale_x = tr_cx / source_cx;
		*scale_y = tr_cy / source_cy;
	}

	return true;
}

int obs_source_transition_clock_mix_spaces(int a, int b)
{
	if ((a == OBS_SOURCE_TRANSITION_CLOCK_CS_709_EXTENDED) || (a == OBS_SOURCE_TRANSITION_CLOCK_CS_709_SCRGB) ||
	    (b == OBS_SOURCE_TRANSITION_CLOCK_CS_709_EXTENDED) || (b == OBS_SOURCE_TRANSITION_CLOCK_CS_709_SCRGB))
		return OBS_SOURCE_TRANSITION_CLOCK_CS_709_EXTENDED;
	if ((a == OBS_SOURCE_TRANSITION_CLOCK_CS_SRGB_16F) || (b == OBS_SOURCE_TRANSITION_CLOCK_CS_SRGB_16F))
		return OBS_SOURCE_TRANSITION_CLOCK_CS_SRGB_16F;
	return OBS_SOURCE_TRANSITION_CLOCK_CS_SRGB;
}

uint64_t obs_source_transition_clock_min_ts(uint64_t a, uint64_t b)
{
	const uint64_t timestamps[2] = {a, b};
	uint64_t min_ts = 0;

	for (size_t i = 0; i < 2; i++) {
		uint64_t timestamp = timestamps[i];
		if (timestamp && (!min_ts || timestamp < min_ts))
			min_ts = timestamp;
	}

	return min_ts;
}
