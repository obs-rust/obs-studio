#include "libobs/build/libobs.pch.h"

bool obs_hotkey_combination_is_background(obs_key_combination_t combo)
{
	if (obs_hotkeys_get_background_state(NULL) == OBS_HOTKEYS_BACKGROUND_NATIVE)
		return true;

	char *id = obs_hotkey_combination_get_background_id(combo);
	bool background = id != NULL;
	bfree(id);
	return background;
}
