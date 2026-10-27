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

#include "libobs/build/libobs-ported-source-monitoring.pch.h"

/* The monitoring, monitor-only mute, balance, mixer, flag, push-to and audio
 * active rules of the sources, moved from obs-source.c: the same-value
 * returns of obs_source_set_flags(), obs_source_set_audio_mixers() and
 * obs_source_set_monitoring_enabled() and the changed rule of the push-to
 * enables, the audio gate and the stored value of the mixers, the push-to
 * log rule, the deprecated and stored monitoring types and the monitor
 * route, the steps of obs_source_set_monitoring_type(), clamp_balance() and
 * the signal of obs_source_set_audio_active(), on plain values. obs-source.c
 * keeps obs_source_valid(), every read of the source's fields at its point,
 * the mutex, the atomics, the mute calls, the signals, the monitor and the
 * logs. Declared in obs-source-monitoring.h without EXPORT. In hybrid libobs
 * the Rust port (rust/obs-source, monitoring) replaces this file. */

bool obs_source_monitoring_changes(uint32_t cur, uint32_t value)
{
	return cur != value;
}

bool obs_source_monitoring_has_audio(uint32_t output_flags)
{
	return (output_flags & OBS_SOURCE_MONITORING_AUDIO) != 0;
}

uint32_t obs_source_monitoring_mixers_store(int64_t mixers)
{
	return (uint32_t)mixers;
}

bool obs_source_monitoring_push_to_logs(uint32_t output_flags, bool changed)
{
	return output_flags & OBS_SOURCE_MONITORING_AUDIO && changed;
}

/* a monitored source that is muted is monitor-only (#2447) */
int obs_source_monitoring_on_type(bool muted)
{
	return muted ? OBS_SOURCE_MONITORING_TYPE_MONITOR_ONLY : OBS_SOURCE_MONITORING_TYPE_MONITOR_AND_OUTPUT;
}

int obs_source_monitoring_stored_type(bool enabled)
{
	return enabled ? OBS_SOURCE_MONITORING_TYPE_MONITOR_AND_OUTPUT : OBS_SOURCE_MONITORING_TYPE_NONE;
}

enum obs_source_monitoring_monitor obs_source_monitoring_monitor(bool was_on, bool enabled)
{
	if (was_on == enabled)
		return OBS_SOURCE_MONITORING_MONITOR_KEEP;
	return !was_on ? OBS_SOURCE_MONITORING_MONITOR_CREATE : OBS_SOURCE_MONITORING_MONITOR_DESTROY;
}

enum obs_source_monitoring_route obs_source_monitoring_route(int type)
{
	switch (type) {
	case OBS_SOURCE_MONITORING_TYPE_NONE:
		return OBS_SOURCE_MONITORING_ROUTE_OFF;
	case OBS_SOURCE_MONITORING_TYPE_MONITOR_ONLY:
		return OBS_SOURCE_MONITORING_ROUTE_MONITOR_ONLY;
	case OBS_SOURCE_MONITORING_TYPE_MONITOR_AND_OUTPUT:
		return OBS_SOURCE_MONITORING_ROUTE_MONITOR_AND_OUTPUT;
	default:
		/* not a type: on, as before */
		return OBS_SOURCE_MONITORING_ROUTE_ON;
	}
}

/* Leaving monitor-only for none undoes the mute monitor-only set, as long as
 * nothing has set the mute since; any other mute stays (#2444). */
bool obs_source_monitoring_off_reads_mute(bool muted_for_monitor_only)
{
	return muted_for_monitor_only;
}

bool obs_source_monitoring_off_unmutes(bool muted)
{
	return muted;
}

/* The mute comes first, so the source never plays into the outputs in
 * between (#2447). */
bool obs_source_monitoring_only_mutes(bool muted)
{
	return !muted;
}

/* The balance is 0 (left) to 1 (right); NaN is the default, centred
 * (#2051). */
float obs_source_monitoring_balance(float balance)
{
	if (isnan(balance))
		return 0.5f;
	return balance < 0.0f ? 0.0f : balance > 1.0f ? 1.0f : balance;
}

enum obs_source_monitoring_active obs_source_monitoring_audio_active(bool prev, bool active)
{
	if (prev == active)
		return OBS_SOURCE_MONITORING_ACTIVE_NONE;
	return active ? OBS_SOURCE_MONITORING_ACTIVE_ACTIVATE : OBS_SOURCE_MONITORING_ACTIVE_DEACTIVATE;
}
