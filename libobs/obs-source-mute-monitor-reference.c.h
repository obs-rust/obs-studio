#include "libobs/build/libobs.pch.h"

/* Reference dispatch uses the unchanged original callbacks in this group. */
bool obs_source_mute_monitor_hotkey_dispatch(void *source, bool monitor, bool target, bool pressed,
					     obs_source_mute_monitor_getter getter,
					     obs_source_mute_monitor_setter setter)
{
	UNUSED_PARAMETER(getter);
	UNUSED_PARAMETER(setter);
	if (monitor)
		return target ? obs_source_hotkey_monitor_on(source, 0, NULL, pressed)
			      : obs_source_hotkey_monitor_off(source, 0, NULL, pressed);
	return target ? obs_source_hotkey_mute(source, 0, NULL, pressed)
		      : obs_source_hotkey_unmute(source, 0, NULL, pressed);
}
