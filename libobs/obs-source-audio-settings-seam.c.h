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

/* Private adapter, not a new public API (obs-source-audio-settings-seam.h). */

_Static_assert(sizeof(enum obs_monitoring_type) == sizeof(int), "enum obs_monitoring_type is passed as int");

bool obs_source_audio_settings_seam_valid(const obs_source_t *source, const char *func)
{
	return obs_source_valid(source, func);
}

void obs_source_audio_settings_seam_warn(const char *message)
{
	blog(LOG_WARNING, "%s", message);
}

bool obs_source_audio_settings_seam_is_private(const obs_source_t *source)
{
	return source->context.private;
}

/* Locking synchronizes the reads without changing the logical source. */
void obs_source_audio_settings_seam_lock_audio_buf(const obs_source_t *source)
{
	pthread_mutex_lock((pthread_mutex_t *)(uintptr_t)&source->audio_buf_mutex);
}

void obs_source_audio_settings_seam_unlock_audio_buf(const obs_source_t *source)
{
	pthread_mutex_unlock((pthread_mutex_t *)(uintptr_t)&source->audio_buf_mutex);
}

void obs_source_audio_settings_seam_lock_actions(const obs_source_t *source)
{
	pthread_mutex_lock((pthread_mutex_t *)(uintptr_t)&source->audio_actions_mutex);
}

void obs_source_audio_settings_seam_unlock_actions(const obs_source_t *source)
{
	pthread_mutex_unlock((pthread_mutex_t *)(uintptr_t)&source->audio_actions_mutex);
}

float obs_source_audio_settings_seam_get_user_volume(const obs_source_t *source)
{
	return source->user_volume;
}

void obs_source_audio_settings_seam_set_user_volume(obs_source_t *source, float value)
{
	source->user_volume = value;
}

int64_t obs_source_audio_settings_seam_get_sync_offset(const obs_source_t *source)
{
	return source->sync_offset;
}

void obs_source_audio_settings_seam_set_sync_offset(obs_source_t *source, int64_t value)
{
	source->sync_offset = value;
}

float obs_source_audio_settings_seam_get_balance(const obs_source_t *source)
{
	return source->balance;
}

void obs_source_audio_settings_seam_set_balance(obs_source_t *source, float value)
{
	source->balance = value;
}

void obs_source_audio_settings_seam_set_monitoring_type(obs_source_t *source, int value)
{
	source->monitoring_type = (enum obs_monitoring_type)value;
}

bool obs_source_audio_settings_seam_get_user_muted(const obs_source_t *source)
{
	return os_atomic_load_bool(&source->user_muted);
}

void obs_source_audio_settings_seam_set_user_muted(obs_source_t *source, bool value)
{
	os_atomic_set_bool(&source->user_muted, value);
}

bool obs_source_audio_settings_seam_get_monitor_only_muted(const obs_source_t *source)
{
	return os_atomic_load_bool(&source->monitor_only_muted);
}

void obs_source_audio_settings_seam_set_monitor_only_muted(obs_source_t *source, bool value)
{
	os_atomic_set_bool(&source->monitor_only_muted, value);
}

bool obs_source_audio_settings_seam_get_monitoring_enabled(const obs_source_t *source)
{
	return os_atomic_load_bool(&source->monitoring_enabled);
}

void obs_source_audio_settings_seam_set_monitoring_enabled(obs_source_t *source, bool value)
{
	os_atomic_set_bool(&source->monitoring_enabled, value);
}

void obs_source_audio_settings_seam_push_volume_action(obs_source_t *source, uint64_t timestamp, float volume)
{
	struct audio_action action = {.timestamp = timestamp, .type = AUDIO_ACTION_VOL, .vol = volume};

	da_push_back(source->audio_actions, &action);
}

void obs_source_audio_settings_seam_push_mute_action(obs_source_t *source, uint64_t timestamp, bool muted)
{
	struct audio_action action = {.timestamp = timestamp, .type = AUDIO_ACTION_MUTE, .set = muted};

	da_push_back(source->audio_actions, &action);
}

double obs_source_audio_settings_seam_signal_volume(obs_source_t *source, double volume, bool also_core)
{
	struct calldata data;
	uint8_t stack[128];

	calldata_init_fixed(&data, stack, sizeof(stack));
	calldata_set_ptr(&data, "source", source);
	calldata_set_float(&data, "volume", volume);

	signal_handler_signal(source->context.signals, "volume", &data);
	if (also_core)
		signal_handler_signal(obs->signals, "source_volume", &data);

	return calldata_float(&data, "volume");
}

int64_t obs_source_audio_settings_seam_signal_sync_offset(obs_source_t *source, int64_t offset)
{
	struct calldata data;
	uint8_t stack[128];

	calldata_init_fixed(&data, stack, sizeof(stack));
	calldata_set_ptr(&data, "source", source);
	calldata_set_int(&data, "offset", offset);

	signal_handler_signal(source->context.signals, "audio_sync", &data);

	return calldata_int(&data, "offset");
}

double obs_source_audio_settings_seam_signal_balance(obs_source_t *source, double balance)
{
	struct calldata data;
	uint8_t stack[128];

	calldata_init_fixed(&data, stack, sizeof(stack));
	calldata_set_ptr(&data, "source", source);
	calldata_set_float(&data, "balance", balance);

	signal_handler_signal(source->context.signals, "audio_balance", &data);

	return calldata_float(&data, "balance");
}

void obs_source_audio_settings_seam_signal_muted(obs_source_t *source, bool muted)
{
	struct calldata data;
	uint8_t stack[128];

	calldata_init_fixed(&data, stack, sizeof(stack));
	calldata_set_ptr(&data, "source", source);
	calldata_set_bool(&data, "muted", muted);

	signal_handler_signal(source->context.signals, "mute", &data);
}

void obs_source_audio_settings_seam_signal_monitor(obs_source_t *source, bool enabled)
{
	struct calldata data;
	uint8_t stack[128];

	calldata_init_fixed(&data, stack, sizeof(stack));
	calldata_set_ptr(&data, "source", source);
	calldata_set_bool(&data, "monitor", enabled);

	signal_handler_signal(source->context.signals, "monitor", &data);
}

void obs_source_audio_settings_seam_signal_audio_monitoring(obs_source_t *source, int monitoring_type)
{
	struct calldata data;
	uint8_t stack[128];

	calldata_init_fixed(&data, stack, sizeof(stack));
	calldata_set_ptr(&data, "source", source);
	calldata_set_int(&data, "type", monitoring_type);

	signal_handler_signal(source->context.signals, "audio_monitoring", &data);
}

void obs_source_audio_settings_seam_monitor_create(obs_source_t *source)
{
	source->monitor = audio_monitor_create(source);
}

void obs_source_audio_settings_seam_monitor_destroy(obs_source_t *source)
{
	audio_monitor_destroy(source->monitor);
	source->monitor = NULL;
}
