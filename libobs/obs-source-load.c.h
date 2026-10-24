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

#include "libobs/build/libobs-ported-source-load.pch.h"

/* The source load migration and value rules, moved from obs.c: only values,
 * no source, data or libobs state. obs.c reads each saved value and calls
 * each setter. Ported to rust/obs-core (source_load): the hybrid libobs drops
 * this file. */

uint32_t obs_source_load_prev_ver(int64_t stored)
{
	return (uint32_t)stored;
}

float obs_source_load_volume(double volume)
{
	/* in float's range before the cast; the setter maps NaN, negative
	 * and infinite volumes (#2108) */
	if (volume < 0.0)
		volume = 0.0;
	else if (volume > (double)FLT_MAX)
		volume = (double)INFINITY;
	return (float)volume;
}

float obs_source_load_balance(double balance)
{
	/* in range before the float cast; the setter clamps it and maps NaN
	 * to 0.5 (#2051) */
	if (balance < 0.0)
		balance = 0.0;
	else if (balance > 1.0)
		balance = 1.0;
	return (float)balance;
}

uint64_t obs_source_load_delay(int64_t delay)
{
	/* stored signed; a negative delay is none (#2051) */
	return delay < 0 ? 0 : (uint64_t)delay;
}

int obs_source_load_deinterlace_mode(int64_t mode)
{
	/* stored as int64; a mode past the int range is no mode, not its low
	 * 32 bits, and the setter disables a mode it does not know (#2090) */
	return mode < INT_MIN || mode > INT_MAX ? OBS_SOURCE_LOAD_DEINTERLACE_DISABLE : (int)mode;
}

struct obs_source_load_monitoring obs_source_load_monitoring_migration(uint32_t prev_ver, uint32_t caps,
								       int64_t stored_type)
{
	struct obs_source_load_monitoring plan = {0};
	int monitoring_type = (int)stored_type;

	if (prev_ver < OBS_SOURCE_LOAD_MONITOR_ONLY_BEFORE) {
		if ((caps & OBS_SOURCE_LOAD_MONITOR_BY_DEFAULT) != 0) {
			/* updates older sources to enable monitoring
			 * automatically if they added monitoring by default in
			 * version 24 */
			monitoring_type = OBS_SOURCE_LOAD_MONITORING_MONITOR_ONLY;
			plan.set_mixers = true;
			plan.mixers = OBS_SOURCE_LOAD_ALL_MIXERS;
		}
	}

	if (prev_ver < OBS_SOURCE_LOAD_MONITORING_ENABLED_BEFORE) {
		plan.store_enabled = true;
		plan.enabled = monitoring_type != OBS_SOURCE_LOAD_MONITORING_NONE;
	}

	return plan;
}
