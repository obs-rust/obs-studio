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

#include "libobs/build/libobs-ported-output-stop-stats.pch.h"

struct obs_output_stop_stats obs_output_stop_stats(uint32_t total_frames, uint32_t lagged_frames,
						   uint32_t starting_drawn, uint32_t starting_lagged, int dropped,
						   int total)
{
	struct obs_output_stop_stats stats = {0};
	uint32_t drawn = total_frames - starting_drawn;
	uint32_t lagged = lagged_frames - starting_lagged;

	double percentage_lagged = 0.0f;
	double percentage_dropped = 0.0f;

	if (drawn)
		percentage_lagged = (double)lagged / (double)drawn * 100.0;
	if (dropped)
		percentage_dropped = (double)dropped / (double)total * 100.0;

	stats.drawn = drawn;
	stats.lagged = lagged;
	stats.percentage_lagged = percentage_lagged;
	stats.percentage_dropped = percentage_dropped;

	if (!dropped || !total) {
		stats.frames_output = total;
	} else {
		stats.frames_output = total - dropped;
		stats.output_attempted = true;
	}

	if (!lagged || !drawn) {
		stats.frames_drawn = drawn;
	} else {
		stats.frames_drawn = drawn - lagged;
		stats.drawn_attempted = true;
	}

	stats.lag_line = drawn && lagged;
	stats.drop_line = total && dropped;
	return stats;
}
