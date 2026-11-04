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

/* The source audio settings: moved out of obs-source.c as they are (#3619). */

/* The volume multiplies every sample, so it is 0 or more. NaN is the
 * default, 1, and +inf is 20 (+26 dB), the most the Advanced Audio
 * Properties (2000%) and obs-websocket's SetInputVolume set. Other volumes
 * of 0 or more are kept, above 20 too (#2108). */
static inline float clamp_volume(float volume)
{
	if (isnan(volume))
		return 1.0f;
	if (volume < 0.0f)
		return 0.0f;
	return isinf(volume) ? 20.0f : volume;
}

void obs_source_set_volume(obs_source_t *source, float volume)
{
	if (obs_source_valid(source, "obs_source_set_volume")) {
		volume = clamp_volume(volume);
		struct audio_action action = {.timestamp = os_gettime_ns(), .type = AUDIO_ACTION_VOL, .vol = volume};

		struct calldata data;
		uint8_t stack[128];

		calldata_init_fixed(&data, stack, sizeof(stack));
		calldata_set_ptr(&data, "source", source);
		calldata_set_float(&data, "volume", (double)volume);

		signal_handler_signal(source->context.signals, "volume", &data);
		if (!source->context.private)
			signal_handler_signal(obs->signals, "source_volume", &data);

		/* a handler may change it */
		volume = clamp_volume((float)calldata_float(&data, "volume"));

		pthread_mutex_lock(&source->audio_actions_mutex);
		da_push_back(source->audio_actions, &action);
		pthread_mutex_unlock(&source->audio_actions_mutex);

		pthread_mutex_lock(&source->audio_buf_mutex);
		source->user_volume = volume;
		pthread_mutex_unlock(&source->audio_buf_mutex);
	}
}

float obs_source_get_volume(const obs_source_t *source)
{
	if (!obs_source_valid(source, "obs_source_get_volume"))
		return 0.0f;
	/* Locking synchronizes the read without changing the logical source. */
	pthread_mutex_t *mutex = (pthread_mutex_t *)(uintptr_t)&source->audio_buf_mutex;
	pthread_mutex_lock(mutex);
	float volume = source->user_volume;
	pthread_mutex_unlock(mutex);
	return volume;
}

void obs_source_set_sync_offset(obs_source_t *source, int64_t offset)
{
	if (obs_source_valid(source, "obs_source_set_sync_offset")) {
		struct calldata data;
		uint8_t stack[128];

		calldata_init_fixed(&data, stack, sizeof(stack));
		calldata_set_ptr(&data, "source", source);
		calldata_set_int(&data, "offset", offset);

		signal_handler_signal(source->context.signals, "audio_sync", &data);

		const int64_t new_offset = calldata_int(&data, "offset");

		pthread_mutex_lock(&source->audio_buf_mutex);
		source->sync_offset = new_offset;
		pthread_mutex_unlock(&source->audio_buf_mutex);
	}
}

int64_t obs_source_get_sync_offset(const obs_source_t *source)
{
	if (!obs_source_valid(source, "obs_source_get_sync_offset"))
		return 0;
	/* Locking synchronizes the read without changing the logical source. */
	pthread_mutex_t *mutex = (pthread_mutex_t *)(uintptr_t)&source->audio_buf_mutex;
	pthread_mutex_lock(mutex);
	int64_t offset = source->sync_offset;
	pthread_mutex_unlock(mutex);
	return offset;
}

bool obs_source_muted(const obs_source_t *source)
{
	return obs_source_valid(source, "obs_source_muted") ? os_atomic_load_bool(&source->user_muted) : false;
}

void obs_source_set_muted(obs_source_t *source, bool muted)
{
	struct calldata data;
	uint8_t stack[128];
	struct audio_action action = {.timestamp = os_gettime_ns(), .type = AUDIO_ACTION_MUTE, .set = muted};

	if (!obs_source_valid(source, "obs_source_set_muted"))
		return;

	os_atomic_set_bool(&source->user_muted, muted);
	/* a mute set by anything else is not Monitor Only's to undo (#2447) */
	os_atomic_set_bool(&source->monitor_only_muted, false);

	calldata_init_fixed(&data, stack, sizeof(stack));
	calldata_set_ptr(&data, "source", source);
	calldata_set_bool(&data, "muted", muted);

	signal_handler_signal(source->context.signals, "mute", &data);

	pthread_mutex_lock(&source->audio_actions_mutex);
	da_push_back(source->audio_actions, &action);
	pthread_mutex_unlock(&source->audio_actions_mutex);
}

void obs_source_set_monitoring_enabled(obs_source_t *source, bool enabled)
{
	struct calldata data;
	struct calldata compat_data;
	uint8_t stack[128];
	enum obs_monitoring_type compat_type;
	bool was_on;

	if (!obs_source_valid(source, "obs_source_set_monitoring_enabled"))
		return;

	if (!obs_source_monitoring_changes(os_atomic_load_bool(&source->monitoring_enabled), enabled))
		return;

	/* the deprecated type: a monitored source that is muted is monitor-only
	 * (#2447); the stored type stays monitor and output */
	compat_type = OBS_MONITORING_TYPE_NONE;
	if (enabled)
		compat_type = (enum obs_monitoring_type)obs_source_monitoring_on_type(
			os_atomic_load_bool(&source->user_muted));

	calldata_init_fixed(&data, stack, sizeof(stack));
	calldata_set_ptr(&data, "source", source);
	calldata_set_bool(&data, "monitor", enabled);

	signal_handler_signal(source->context.signals, "monitor", &data);

	calldata_init_fixed(&compat_data, stack, sizeof(stack));
	calldata_set_ptr(&compat_data, "source", source);
	calldata_set_int(&compat_data, "type", compat_type);

	signal_handler_signal(source->context.signals, "audio_monitoring", &compat_data);

	was_on = os_atomic_load_bool(&source->monitoring_enabled);
	os_atomic_set_bool(&source->monitoring_enabled, enabled);
	pthread_mutex_lock(&source->audio_buf_mutex);
	source->monitoring_type = (enum obs_monitoring_type)obs_source_monitoring_stored_type(enabled);
	pthread_mutex_unlock(&source->audio_buf_mutex);

	switch (obs_source_monitoring_monitor(was_on, enabled)) {
	case OBS_SOURCE_MONITORING_MONITOR_CREATE:
		source->monitor = audio_monitor_create(source);
		break;
	case OBS_SOURCE_MONITORING_MONITOR_DESTROY:
		audio_monitor_destroy(source->monitor);
		source->monitor = NULL;
		break;
	case OBS_SOURCE_MONITORING_MONITOR_KEEP:
		break;
	}
}

bool obs_source_get_monitoring_enabled(const obs_source_t *source)
{
	return obs_source_valid(source, "obs_source_get_monitoring_enabled")
		       ? os_atomic_load_bool(&source->monitoring_enabled)
		       : false;
}

void obs_source_set_monitoring_type(obs_source_t *source, enum obs_monitoring_type type)
{
	bool muted_for_monitor_only;

	if (!obs_source_valid(source, "obs_source_set_monitoring_type"))
		return;

	blog(LOG_WARNING,
	     "obs_source_set_monitoring_type is deprecated. Use obs_source_set_monitoring_enabled instead.");

	/* Since 33.0 monitoring is on or off, on means monitor and output, and
	 * the monitor ignores mute: monitor-only is monitoring on and muted, as
	 * a pre-33.0 Monitor Only source loads (#2444, #2447). The mute comes
	 * first, so the source never plays into the outputs in between. Leaving
	 * monitor-only for none undoes the mute monitor-only set, as long as
	 * nothing has set the mute since; any other mute stays. */
	muted_for_monitor_only = os_atomic_load_bool(&source->monitor_only_muted);

	switch (obs_source_monitoring_route((int)type)) {
	case OBS_SOURCE_MONITORING_ROUTE_OFF:
		obs_source_set_monitoring_enabled(source, false);
		if (obs_source_monitoring_off_reads_mute(muted_for_monitor_only) &&
		    obs_source_monitoring_off_unmutes(obs_source_muted(source)))
			obs_source_set_muted(source, false);
		os_atomic_set_bool(&source->monitor_only_muted, false);
		break;
	case OBS_SOURCE_MONITORING_ROUTE_MONITOR_ONLY:
		if (obs_source_monitoring_only_mutes(obs_source_muted(source))) {
			obs_source_set_muted(source, true);
			muted_for_monitor_only = true;
		}
		obs_source_set_monitoring_enabled(source, true);
		os_atomic_set_bool(&source->monitor_only_muted, muted_for_monitor_only);
		break;
	case OBS_SOURCE_MONITORING_ROUTE_MONITOR_AND_OUTPUT:
		obs_source_set_muted(source, false);
		obs_source_set_monitoring_enabled(source, true);
		break;
	case OBS_SOURCE_MONITORING_ROUTE_ON:
		obs_source_set_monitoring_enabled(source, true);
		break;
	}
}

enum obs_monitoring_type obs_source_get_monitoring_type(const obs_source_t *source)
{
	blog(LOG_WARNING,
	     "obs_source_get_monitoring_type is deprecated. Use obs_source_get_monitoring_enabled instead.");

	if (!obs_source_valid(source, "obs_source_get_monitoring_type") ||
	    !os_atomic_load_bool(&source->monitoring_enabled))
		return OBS_MONITORING_TYPE_NONE;

	/* monitoring on and muted is monitor-only (#2447) */
	return (enum obs_monitoring_type)obs_source_monitoring_on_type(os_atomic_load_bool(&source->user_muted));
}

static inline float clamp_balance(float balance)
{
	return obs_source_monitoring_balance(balance);
}

void obs_source_set_balance_value(obs_source_t *source, float balance)
{
	if (obs_source_valid(source, "obs_source_set_balance_value")) {
		struct calldata data;
		uint8_t stack[128];

		calldata_init_fixed(&data, stack, sizeof(stack));
		calldata_set_ptr(&data, "source", source);
		calldata_set_float(&data, "balance", (double)clamp_balance(balance));

		signal_handler_signal(source->context.signals, "audio_balance", &data);

		/* a handler may change it */
		const float new_balance = clamp_balance((float)calldata_float(&data, "balance"));

		pthread_mutex_lock(&source->audio_buf_mutex);
		source->balance = new_balance;
		pthread_mutex_unlock(&source->audio_buf_mutex);
	}
}

float obs_source_get_balance_value(const obs_source_t *source)
{
	if (!obs_source_valid(source, "obs_source_get_balance_value"))
		return 0.5f;
	/* Locking synchronizes the read without changing the logical source. */
	pthread_mutex_t *mutex = (pthread_mutex_t *)(uintptr_t)&source->audio_buf_mutex;
	pthread_mutex_lock(mutex);
	float balance = source->balance;
	pthread_mutex_unlock(mutex);
	return balance;
}
