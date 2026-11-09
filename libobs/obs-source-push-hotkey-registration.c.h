#include "libobs/build/libobs.pch.h"

static void obs_source_hotkey_push_to_mute(void *data, obs_hotkey_id id, obs_hotkey_t *key, bool pressed)
{
	UNUSED_PARAMETER(id);
	UNUSED_PARAMETER(key);
	obs_source_push_hotkey_dispatch(data, false, pressed);
}

static void obs_source_hotkey_push_to_talk(void *data, obs_hotkey_id id, obs_hotkey_t *key, bool pressed)
{
	UNUSED_PARAMETER(id);
	UNUSED_PARAMETER(key);
	obs_source_push_hotkey_dispatch(data, true, pressed);
}
