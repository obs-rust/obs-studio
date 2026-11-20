#include "libobs/build/libobs.pch.h"

/* The key name store (obs->hotkeys.translations[]): obs_hotkeys_set_translations_s()
 * in obs-hotkey-translations.c.h calls it for each name it sets. Declared in
 * obs-internal.h; not exported. */
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
