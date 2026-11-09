#include "libobs/build/libobs.pch.h"

/* Typed adapters keep the public C function pointer types exact. */
static bool mute_monitor_hotkey_muted(const void *source)
{
	return obs_source_muted(source);
}

static void mute_monitor_hotkey_set_muted(void *source, bool enabled)
{
	obs_source_set_muted(source, enabled);
}

static bool mute_monitor_hotkey_monitored(const void *source)
{
	return obs_source_get_monitoring_enabled(source);
}

static void mute_monitor_hotkey_set_monitored(void *source, bool enabled)
{
	obs_source_set_monitoring_enabled(source, enabled);
}

static bool obs_source_hotkey_mute(void *data, obs_hotkey_pair_id id, obs_hotkey_t *key, bool pressed)
{
	UNUSED_PARAMETER(id);
	UNUSED_PARAMETER(key);
	return obs_source_mute_monitor_hotkey_dispatch(data, false, true, pressed, mute_monitor_hotkey_muted,
						       mute_monitor_hotkey_set_muted);
}

static bool obs_source_hotkey_unmute(void *data, obs_hotkey_pair_id id, obs_hotkey_t *key, bool pressed)
{
	UNUSED_PARAMETER(id);
	UNUSED_PARAMETER(key);
	return obs_source_mute_monitor_hotkey_dispatch(data, false, false, pressed, mute_monitor_hotkey_muted,
						       mute_monitor_hotkey_set_muted);
}

static bool obs_source_hotkey_monitor_on(void *data, obs_hotkey_pair_id id, obs_hotkey_t *key, bool pressed)
{
	UNUSED_PARAMETER(id);
	UNUSED_PARAMETER(key);
	return obs_source_mute_monitor_hotkey_dispatch(data, true, true, pressed, mute_monitor_hotkey_monitored,
						       mute_monitor_hotkey_set_monitored);
}

static bool obs_source_hotkey_monitor_off(void *data, obs_hotkey_pair_id id, obs_hotkey_t *key, bool pressed)
{
	UNUSED_PARAMETER(id);
	UNUSED_PARAMETER(key);
	return obs_source_mute_monitor_hotkey_dispatch(data, true, false, pressed, mute_monitor_hotkey_monitored,
						       mute_monitor_hotkey_set_monitored);
}
