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

#include <stddef.h>
#include <stdint.h>

/* The time of each audio sample a transition mixes, on plain values
 * (obs-source-transition-sample-time.c): get_sample_time() of
 * obs-source-transition.c. The sample's offset from the start of the block,
 * util_mul_div64(sample, 1000000000, sample_rate) ns, is added to the
 * block's timestamp ts in uint64_t, and the transition's clock
 * (obs_source_transition_clock_time(), calc_time()) is read at that time.
 *
 * mode, manual_val, start_time and duration are the transition's
 * transition_mode, transition_manual_val, transition_start_time and
 * transition_duration, which obs_transition_audio_render() copies once per
 * block under the transition's lock (#3499) and mix_child() passes for
 * every sample. obs-source-transition.c keeps that copy, calc_time() for
 * its other callers, the mix loops and the plugin's mix callback.
 *
 * Nothing here takes a lock, calls into libobs beyond the clock, or keeps
 * state.
 *
 * Hidden in libobs: not EXPORT. */

/* The position of the transition, from 0 to 1, at sample sample of a block
 * of sample_rate Hz that starts at ts. */
float obs_source_transition_sample_time(int mode, float manual_val, uint64_t start_time, uint64_t duration, uint64_t ts,
					size_t sample_rate, size_t sample);
