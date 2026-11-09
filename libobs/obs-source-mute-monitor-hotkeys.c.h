#include "libobs/build/libobs.pch.h"

static bool obs_source_hotkey_mute(void *data, obs_hotkey_pair_id id, obs_hotkey_t *key, bool pressed)
{
	UNUSED_PARAMETER(id);
	UNUSED_PARAMETER(key);

	struct obs_source *source = data;

	if (!pressed || obs_source_muted(source))
		return false;

	obs_source_set_muted(source, true);
	return true;
}

static bool obs_source_hotkey_unmute(void *data, obs_hotkey_pair_id id, obs_hotkey_t *key, bool pressed)
{
	UNUSED_PARAMETER(id);
	UNUSED_PARAMETER(key);

	struct obs_source *source = data;

	if (!pressed || !obs_source_muted(source))
		return false;

	obs_source_set_muted(source, false);
	return true;
}

static bool obs_source_hotkey_monitor_on(void *data, obs_hotkey_pair_id id, obs_hotkey_t *key, bool pressed)
{
	UNUSED_PARAMETER(id);
	UNUSED_PARAMETER(key);

	struct obs_source *source = data;

	if (!pressed || obs_source_get_monitoring_enabled(source))
		return false;

	obs_source_set_monitoring_enabled(source, true);
	return true;
}

static bool obs_source_hotkey_monitor_off(void *data, obs_hotkey_pair_id id, obs_hotkey_t *key, bool pressed)
{
	UNUSED_PARAMETER(id);
	UNUSED_PARAMETER(key);

	struct obs_source *source = data;

	if (!pressed || !obs_source_get_monitoring_enabled(source))
		return false;

	obs_source_set_monitoring_enabled(source, false);
	return true;
}
