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

#include "libobs/build/libobs.pch.h"

/* get_sample_time() of obs-source-transition.c, on plain values. Declared in
 * obs-source-transition-sample-time.h without EXPORT. */

float obs_source_transition_sample_time(int mode, float manual_val, uint64_t start_time, uint64_t duration, uint64_t ts,
					size_t sample_rate, size_t sample)
{
	uint64_t sample_ts_offset = util_mul_div64(sample, 1000000000ULL, sample_rate);
	uint64_t i_ts = ts + sample_ts_offset;
	return obs_source_transition_clock_time(mode, manual_val, start_time, duration, i_ts);
}
