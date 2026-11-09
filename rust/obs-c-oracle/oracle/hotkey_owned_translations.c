/* Unchanged setter bodies and real private field adapter over a test-owned core. */
#include "libobs/build/libobs.pch.h"

static _Thread_local struct obs_core owned_translations_core;
#define obs (&owned_translations_core)
#define obs_hotkeys_set_audio_hotkeys_translations oracle_obs_hotkeys_set_audio_hotkeys_translations
#define obs_hotkeys_set_sceneitem_hotkeys_translations oracle_obs_hotkeys_set_sceneitem_hotkeys_translations
void oracle_obs_hotkeys_set_audio_hotkeys_translations(const char *, const char *, const char *, const char *,
						       const char *, const char *);
void oracle_obs_hotkeys_set_sceneitem_hotkeys_translations(const char *, const char *);
#include "libobs/obs-hotkey-owned-translations.c.h"
#undef obs_hotkeys_set_sceneitem_hotkeys_translations
#undef obs_hotkeys_set_audio_hotkeys_translations
#include "hotkey_owned_translations_fields.inc"
#undef obs

void oracle_hotkey_owned_translations_layout(size_t *out);
void oracle_hotkey_owned_translations_layout(size_t *out)
{
	out[0] = sizeof(owned_translations_core.hotkeys.mute);
	out[1] = _Alignof(char *);
	out[2] = sizeof(char **);
	out[3] = _Alignof(char **);
	out[4] = sizeof(size_t);
	out[5] = _Alignof(size_t);
}

/* Keep fixture cleanup identical to the real core's bfree ownership. */
void oracle_hotkey_owned_translations_clear(void);
void oracle_hotkey_owned_translations_clear(void)
{
	for (size_t i = 0; i < 8; i++) {
		char **field = obs_hotkey_owned_translation_field(i);
		bfree(*field);
		*field = NULL;
	}
}
