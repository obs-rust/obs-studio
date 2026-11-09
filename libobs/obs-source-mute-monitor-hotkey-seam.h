#pragma once

#include <stdbool.h>

typedef bool (*obs_source_mute_monitor_getter)(const void *source);
typedef void (*obs_source_mute_monitor_setter)(void *source, bool enabled);

bool obs_source_mute_monitor_hotkey_dispatch(void *source, bool monitor, bool target, bool pressed,
					     obs_source_mute_monitor_getter getter,
					     obs_source_mute_monitor_setter setter);
