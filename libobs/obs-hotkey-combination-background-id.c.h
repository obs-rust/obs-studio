#include "libobs/build/libobs.pch.h"

/* Defined in obs-hotkey.c.h, later in the same unity group. */
static inline bool lock(void);
static inline void unlock(void);

char *obs_hotkey_combination_get_background_id(obs_key_combination_t combo)
{
	if (!lock())
		return NULL;

	struct dstr id = {0};
	bool (*shortcut_id)(obs_key_combination_t, struct dstr *) = obs->hotkeys.background.shortcut_id;
	unlock();

	if (!shortcut_id || !shortcut_id(combo, &id)) {
		dstr_free(&id);
		return NULL;
	}
	return id.array;
}
