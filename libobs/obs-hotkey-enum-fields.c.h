#include "libobs/build/libobs.pch.h"

obs_hotkey_t *obs_hotkey_enum_first(void)
{
	return obs->hotkeys.hotkeys;
}

obs_hotkey_t *obs_hotkey_enum_next(obs_hotkey_t *hotkey)
{
	return hotkey->hh.next;
}

obs_hotkey_id obs_hotkey_enum_context_id(const struct obs_context_data *context, size_t index)
{
	return context->hotkeys.array[index];
}
