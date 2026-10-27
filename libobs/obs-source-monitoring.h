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

/* The monitoring, monitor-only mute, balance, mixer, flag, push-to and audio
 * active rules of the sources in obs-source.c, on plain values
 * (obs-source-monitoring.c):
 * - obs_source_set_flags(), obs_source_set_audio_mixers() and
 *   obs_source_set_monitoring_enabled(): a set does something only when the
 *   value changes; obs_source_enable_push_to_mute() and
 *   obs_source_enable_push_to_talk() ask the same rule for their signal;
 * - obs_source_set_audio_mixers() and obs_source_get_audio_mixers(): the
 *   mixers belong to a source that owns its info id or has audio, and a
 *   handler's value is stored as its low 32 bits;
 * - obs_source_enable_push_to_mute() and obs_source_enable_push_to_talk():
 *   the log names a change on an audio source only;
 * - obs_source_set_monitoring_enabled() and
 *   obs_source_get_monitoring_type(): monitoring on and muted is Monitor
 *   Only, else Monitor and Output (#2447); the stored type, and whether the
 *   monitor is created or destroyed;
 * - obs_source_set_monitoring_type(): the steps of each type (#2444,
 *   #2447);
 * - clamp_balance(): NaN is centred, else clamped to [0, 1] (#2051);
 * - obs_source_set_audio_active(): which signal a change sends.
 *
 * Each decision is its own function, asked at its own point, so obs-source.c
 * keeps every read and call where it was and under its condition:
 * obs_source_valid(), audio_mutex, every os_atomic_*() on
 * monitor_only_muted and audio_active, the reads of the source's fields,
 * obs_source_get_output_flags(), obs_source_muted() and
 * obs_source_set_muted() (the C's own && before each), every calldata_*(),
 * signal and log, audio_monitor_create() and audio_monitor_destroy(), and
 * the getters' answers for an invalid source (0, false and 0.5) as the
 * literals they are. None of these functions takes a lock, calls into
 * libobs, touches memory or keeps state.
 *
 * The monitoring type crosses as int, so this header needs no other libobs
 * header; obs-source.c checks that the constants below are the libobs
 * values and that enum obs_monitoring_type is int-sized.
 *
 * Hidden in libobs: not EXPORT. */

/* OBS_SOURCE_AUDIO (obs-source.h). */
#define OBS_SOURCE_MONITORING_AUDIO (1 << 1)

/* enum obs_monitoring_type (obs.h). */
#define OBS_SOURCE_MONITORING_TYPE_NONE 0
#define OBS_SOURCE_MONITORING_TYPE_MONITOR_ONLY 1
#define OBS_SOURCE_MONITORING_TYPE_MONITOR_AND_OUTPUT 2

/* What obs_source_set_monitoring_enabled() does with the monitor. */
enum obs_source_monitoring_monitor {
	/* leave it */
	OBS_SOURCE_MONITORING_MONITOR_KEEP,
	/* audio_monitor_create() */
	OBS_SOURCE_MONITORING_MONITOR_CREATE,
	/* audio_monitor_destroy(), and none */
	OBS_SOURCE_MONITORING_MONITOR_DESTROY,
};

/* The steps obs_source_set_monitoring_type() takes for a type. */
enum obs_source_monitoring_route {
	/* none: turn monitoring off, undo the mute Monitor Only set if it is
	 * still on (obs_source_monitoring_off_reads_mute() and
	 * obs_source_monitoring_off_unmutes()), and clear the flag */
	OBS_SOURCE_MONITORING_ROUTE_OFF,
	/* Monitor Only: mute first if not muted
	 * (obs_source_monitoring_only_mutes()) and mark it, turn monitoring
	 * on, and store the mark */
	OBS_SOURCE_MONITORING_ROUTE_MONITOR_ONLY,
	/* Monitor and Output: unmute, and turn monitoring on */
	OBS_SOURCE_MONITORING_ROUTE_MONITOR_AND_OUTPUT,
	/* any other value: turn monitoring on, as before */
	OBS_SOURCE_MONITORING_ROUTE_ON,
};

/* Which signal obs_source_set_audio_active() sends. */
enum obs_source_monitoring_active {
	/* none: the value did not change */
	OBS_SOURCE_MONITORING_ACTIVE_NONE,
	/* source_audio_activate and audio_activate */
	OBS_SOURCE_MONITORING_ACTIVE_ACTIVATE,
	/* source_audio_deactivate and audio_deactivate */
	OBS_SOURCE_MONITORING_ACTIVE_DEACTIVATE,
};

/* Whether a set changes the value: cur != value. obs_source_set_flags(),
 * obs_source_set_audio_mixers() and obs_source_set_monitoring_enabled()
 * return without a signal otherwise; obs_source_enable_push_to_mute() and
 * obs_source_enable_push_to_talk() store the value either way and signal
 * only a change. */
bool obs_source_monitoring_changes(uint32_t cur, uint32_t value);

/* Whether output flags name an audio source: OBS_SOURCE_AUDIO is set.
 * obs_source_set_audio_mixers() and obs_source_get_audio_mixers() ask it of
 * a source that does not own its info id. */
bool obs_source_monitoring_has_audio(uint32_t output_flags);

/* The mixers obs_source_set_audio_mixers() stores from the int64_t a
 * handler of the audio_mixers signal leaves: its low 32 bits. */
uint32_t obs_source_monitoring_mixers_store(int64_t mixers);

/* Whether obs_source_enable_push_to_mute() and
 * obs_source_enable_push_to_talk() log the change: an audio source (its
 * output flags), and changed. */
bool obs_source_monitoring_push_to_logs(uint32_t output_flags, bool changed);

/* The monitoring type of a source with monitoring on: MONITOR_ONLY when it
 * is muted, else MONITOR_AND_OUTPUT (#2447). The deprecated type
 * obs_source_set_monitoring_enabled() sends when it turns monitoring on,
 * and obs_source_get_monitoring_type() of a monitored source. */
int obs_source_monitoring_on_type(bool muted);

/* The type obs_source_set_monitoring_enabled() stores: MONITOR_AND_OUTPUT
 * when on, else NONE. */
int obs_source_monitoring_stored_type(bool enabled);

/* What obs_source_set_monitoring_enabled() does with the monitor, from the
 * value it read after the signals (was_on) and the new one: create it when
 * monitoring turns on, destroy it when it turns off, else leave it. */
enum obs_source_monitoring_monitor obs_source_monitoring_monitor(bool was_on, bool enabled);

/* The steps obs_source_set_monitoring_type() takes for a type: NONE,
 * MONITOR_ONLY and MONITOR_AND_OUTPUT their own, any other value ON. */
enum obs_source_monitoring_route obs_source_monitoring_route(int type);

/* Whether leaving for none looks at the mute: only when Monitor Only set it
 * (muted_for_monitor_only, read once before the steps). */
bool obs_source_monitoring_off_reads_mute(bool muted_for_monitor_only);

/* Whether leaving for none unmutes, given the mute read after monitoring
 * turned off: the mute Monitor Only set is still on. */
bool obs_source_monitoring_off_unmutes(bool muted);

/* Whether Monitor Only mutes the source before it turns monitoring on: it
 * is not muted. */
bool obs_source_monitoring_only_mutes(bool muted);

/* The balance a source stores and sends: NaN is 0.5 (centred), else the
 * value clamped to [0, 1]; -0.0 stays -0.0. */
float obs_source_monitoring_balance(float balance);

/* Which signal obs_source_set_audio_active() sends, from the value
 * os_atomic_set_bool() returned (prev) and the new one. */
enum obs_source_monitoring_active obs_source_monitoring_audio_active(bool prev, bool active);
