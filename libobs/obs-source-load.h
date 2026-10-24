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

/* The source load migration and value rules of obs_load_source_type()
 * (obs.c), on plain values (obs-source-load.c). obs.c reads each value from
 * the source's saved data, passes it in, and hands the answer to the setter,
 * at the point where the C did before:
 * - obs_source_load_prev_ver(): the saved "prev_ver", as the uint32_t the
 *   source is created with and the migration compares;
 * - obs_source_load_volume(): the saved "volume" in float's range before the
 *   cast; obs_source_set_volume() maps NaN, negative and infinite volumes;
 * - obs_source_load_balance(): the saved "balance" in 0 to 1 before the cast;
 *   obs_source_set_balance_value() maps NaN;
 * - obs_source_load_delay(): a saved push-to-mute or push-to-talk delay, with
 *   a negative one as none;
 * - obs_source_load_deinterlace_mode(): the saved "deinterlace_mode", with a
 *   value past the int range as no mode;
 * - obs_source_load_monitoring_migration(): the monitoring migration: before
 *   23.2.2, a monitor-by-default source becomes monitor-only with all six
 *   mixers; before 33.0.0, "monitoring_enabled" is stored from the type.
 *
 * obs.c keeps every obs_data_*() call and setter, in the same order, the
 * source creation, the canvas lookup and the filter recursion. Nothing here
 * takes a lock, calls into libobs or keeps state.
 *
 * The constants are copied below, so this header needs no core header; obs.c
 * checks them against obs-config.h, obs-source.h and obs.h.
 *
 * Hidden in libobs: not EXPORT. */

/* MAKE_SEMANTIC_VERSION(23, 2, 2) (obs-config.h): a source saved before it,
 * with OBS_SOURCE_MONITOR_BY_DEFAULT, becomes monitor-only. */
#define OBS_SOURCE_LOAD_MONITOR_ONLY_BEFORE ((23 << 24) | (2 << 16) | 2)
/* MAKE_SEMANTIC_VERSION(33, 0, 0): a source saved before it gets its
 * "monitoring_enabled" from its monitoring type. */
#define OBS_SOURCE_LOAD_MONITORING_ENABLED_BEFORE ((33 << 24) | (0 << 16) | 0)
/* OBS_SOURCE_MONITOR_BY_DEFAULT (obs-source.h) */
#define OBS_SOURCE_LOAD_MONITOR_BY_DEFAULT (1 << 11)
/* enum obs_monitoring_type (obs.h) */
#define OBS_SOURCE_LOAD_MONITORING_NONE 0
#define OBS_SOURCE_LOAD_MONITORING_MONITOR_ONLY 1
/* All six audio mixers (MAX_AUDIO_MIXES). */
#define OBS_SOURCE_LOAD_ALL_MIXERS 0x3F
/* OBS_DEINTERLACE_MODE_DISABLE (obs.h) */
#define OBS_SOURCE_LOAD_DEINTERLACE_DISABLE 0

/* What obs_load_source_type() does after reading "monitoring_type". */
struct obs_source_load_monitoring {
	/* obs_source_set_audio_mixers(source, mixers): a monitor-by-default
	 * source saved before 23.2.2. */
	bool set_mixers;
	/* set_mixers: OBS_SOURCE_LOAD_ALL_MIXERS. Otherwise 0. */
	uint32_t mixers;
	/* obs_data_set_bool(source_data, "monitoring_enabled", enabled): a
	 * source saved before 33.0.0. */
	bool store_enabled;
	/* store_enabled: whether the (migrated) monitoring type is not none.
	 * Otherwise false. */
	bool enabled;
};

/* The saved "prev_ver" (an int64) as a uint32_t: its low 32 bits. */
uint32_t obs_source_load_prev_ver(int64_t stored);

/* The saved "volume": 0 when negative, INFINITY when past FLT_MAX, then as a
 * float. NaN stays NaN. */
float obs_source_load_volume(double volume);

/* The saved "balance": 0 when negative, 1 when past 1, then as a float. NaN
 * stays NaN. */
float obs_source_load_balance(double balance);

/* A saved push-to-mute or push-to-talk delay: 0 when negative. */
uint64_t obs_source_load_delay(int64_t delay);

/* The saved "deinterlace_mode": OBS_SOURCE_LOAD_DEINTERLACE_DISABLE when past
 * the int range, else as an int. */
int obs_source_load_deinterlace_mode(int64_t mode);

/* The monitoring migration for a source saved by prev_ver, with the output
 * flags caps and the saved "monitoring_type" (an int64, read as an int). */
struct obs_source_load_monitoring obs_source_load_monitoring_migration(uint32_t prev_ver, uint32_t caps,
								       int64_t monitoring_type);
