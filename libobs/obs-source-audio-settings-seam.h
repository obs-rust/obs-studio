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

#include "obs-internal.h"

/* Private adapter, not a new public API: the seam between the source audio
 * settings (rust/obs-source, obs_source::audio_settings) and the fields of
 * struct obs_source (obs-source-audio-settings-seam.c).
 *
 * - One lock and one unlock per mutex, each a single pthread_mutex_*() call.
 * - One get and one set per field, each a single read or write. The caller
 *   holds audio_buf_mutex around user_volume, sync_offset, balance and
 *   monitoring_type. user_muted, monitor_only_muted and monitoring_enabled
 *   are atomic.
 * - One queue per action type. The caller holds audio_actions_mutex.
 * - One function per signal, which builds the calldata, sends the signal and
 *   returns what a handler can change. The caller holds no lock.
 *
 * The fields stay in C so that ThreadSanitizer keeps seeing their reads and
 * writes. Hidden in libobs: not EXPORT. */

bool obs_source_audio_settings_seam_valid(const obs_source_t *source, const char *func);
void obs_source_audio_settings_seam_warn(const char *message);
bool obs_source_audio_settings_seam_is_private(const obs_source_t *source);

void obs_source_audio_settings_seam_lock_audio_buf(const obs_source_t *source);
void obs_source_audio_settings_seam_unlock_audio_buf(const obs_source_t *source);
void obs_source_audio_settings_seam_lock_actions(const obs_source_t *source);
void obs_source_audio_settings_seam_unlock_actions(const obs_source_t *source);

float obs_source_audio_settings_seam_get_user_volume(const obs_source_t *source);
void obs_source_audio_settings_seam_set_user_volume(obs_source_t *source, float value);
int64_t obs_source_audio_settings_seam_get_sync_offset(const obs_source_t *source);
void obs_source_audio_settings_seam_set_sync_offset(obs_source_t *source, int64_t value);
float obs_source_audio_settings_seam_get_balance(const obs_source_t *source);
void obs_source_audio_settings_seam_set_balance(obs_source_t *source, float value);
void obs_source_audio_settings_seam_set_monitoring_type(obs_source_t *source, int value);

bool obs_source_audio_settings_seam_get_user_muted(const obs_source_t *source);
void obs_source_audio_settings_seam_set_user_muted(obs_source_t *source, bool value);
bool obs_source_audio_settings_seam_get_monitor_only_muted(const obs_source_t *source);
void obs_source_audio_settings_seam_set_monitor_only_muted(obs_source_t *source, bool value);
bool obs_source_audio_settings_seam_get_monitoring_enabled(const obs_source_t *source);
void obs_source_audio_settings_seam_set_monitoring_enabled(obs_source_t *source, bool value);

void obs_source_audio_settings_seam_push_volume_action(obs_source_t *source, uint64_t timestamp, float volume);
void obs_source_audio_settings_seam_push_mute_action(obs_source_t *source, uint64_t timestamp, bool muted);

double obs_source_audio_settings_seam_signal_volume(obs_source_t *source, double volume, bool also_core);
int64_t obs_source_audio_settings_seam_signal_sync_offset(obs_source_t *source, int64_t offset);
double obs_source_audio_settings_seam_signal_balance(obs_source_t *source, double balance);
void obs_source_audio_settings_seam_signal_muted(obs_source_t *source, bool muted);
void obs_source_audio_settings_seam_signal_monitor(obs_source_t *source, bool enabled);
void obs_source_audio_settings_seam_signal_audio_monitoring(obs_source_t *source, int monitoring_type);

void obs_source_audio_settings_seam_monitor_create(obs_source_t *source);
void obs_source_audio_settings_seam_monitor_destroy(obs_source_t *source);
