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

/* The stop statistics of obs-output.c (log_frame_info()), on plain values
 * (obs-output-stop-stats.c): the frames drawn and lagged since the output
 * started, the two percentages, and which of the lines log_frame_info()
 * writes after "stopping" are logged, with which numbers.
 *
 * log_frame_info() keeps obs_video_stats_frame_counts(), the reads of the
 * starting counts and of total_frames, obs_output_get_frames_dropped() (the
 * plugin's get_dropped_frames()), in that order, and every blog() with its
 * format string. It passes the values here and logs from the result.
 * Nothing here takes a lock, calls into libobs or keeps state.
 *
 * Hidden in libobs: not EXPORT. */

struct obs_output_stop_stats {
	/* frames drawn and lagged since the output started: the video
	 * counters now minus at the start (uint32_t, wrapping) */
	uint32_t drawn;
	uint32_t lagged;
	/* (double)lagged / (double)drawn * 100.0 when drawn is not 0, else 0 */
	double percentage_lagged;
	/* (double)dropped / (double)total * 100.0 when dropped is not 0,
	 * else 0 */
	double percentage_dropped;
	/* "Total frames output": total, or with " (%d attempted)" (total)
	 * total - dropped */
	int frames_output;
	bool output_attempted;
	/* "Total drawn frames": drawn, or with " (%u attempted)" (drawn)
	 * drawn - lagged */
	uint32_t frames_drawn;
	bool drawn_attempted;
	/* "Number of lagged frames due to rendering lag/stalls": lagged and
	 * percentage_lagged */
	bool lag_line;
	/* "Number of dropped frames due to insufficient bandwidth/connection
	 * stalls": dropped and percentage_dropped */
	bool drop_line;
};

/* log_frame_info(): total_frames and lagged_frames are the video counters
 * now (obs_video_stats_frame_counts()), starting_drawn and starting_lagged
 * the output's counts at its start, dropped the plugin's dropped frames
 * (obs_output_get_frames_dropped()) and total the output's total_frames. */
struct obs_output_stop_stats obs_output_stop_stats(uint32_t total_frames, uint32_t lagged_frames,
						   uint32_t starting_drawn, uint32_t starting_lagged, int dropped,
						   int total);
