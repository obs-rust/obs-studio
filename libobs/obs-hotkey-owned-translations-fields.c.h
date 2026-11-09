#include "libobs/build/libobs.pch.h"

char **obs_hotkey_owned_translation_field(size_t slot);

char **obs_hotkey_owned_translation_field(size_t slot)
{
	char **fields[] = {&obs->hotkeys.mute,           &obs->hotkeys.unmute,        &obs->hotkeys.push_to_mute,
			   &obs->hotkeys.push_to_talk,   &obs->hotkeys.monitor_on,    &obs->hotkeys.monitor_off,
			   &obs->hotkeys.sceneitem_show, &obs->hotkeys.sceneitem_hide};
	return fields[slot];
}
