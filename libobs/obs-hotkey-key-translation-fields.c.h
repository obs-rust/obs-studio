#include "libobs/build/libobs.pch.h"

/* Private adapters for the Rust key-translation query and store. The name
 * array, obs->hotkeys.translations[], and its bmalloc/bfree ownership stay in
 * C. As in the original, no lock is taken and the key is not range-checked. */
char **obs_hotkey_key_translation_slot(obs_key_t key);
char **obs_hotkey_key_translation_slot(obs_key_t key)
{
	return &obs->hotkeys.translations[key];
}

void obs_hotkey_key_translation_free(char *translation);
void obs_hotkey_key_translation_free(char *translation)
{
	bfree(translation);
}

char *obs_hotkey_key_translation_dup(const char *translation);
char *obs_hotkey_key_translation_dup(const char *translation)
{
	return bstrdup(translation);
}
