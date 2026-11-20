#include "libobs/build/libobs.pch.h"

void obs_hotkeys_store_key_translation(obs_key_t key, const char *translation)
{
	bfree(obs->hotkeys.translations[key]);
	obs->hotkeys.translations[key] = NULL;

	if (translation)
		obs->hotkeys.translations[key] = bstrdup(translation);
}

const char *obs_get_hotkey_translation(obs_key_t key, const char *def)
{
	if (key == OBS_KEY_NONE) {
		return NULL;
	}

	return obs->hotkeys.translations[key] ? obs->hotkeys.translations[key] : def;
}
