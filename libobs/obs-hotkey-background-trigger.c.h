#include "libobs/build/libobs.pch.h"

extern bool obs_hotkey_seam_lock(void);
extern void obs_hotkey_seam_unlock(void);

/* Preserve the extracted body while using the existing private mutex seam. */
#define lock obs_hotkey_seam_lock
#define unlock obs_hotkey_seam_unlock

char *obs_hotkey_combination_get_background_trigger(obs_key_combination_t combo)
{
	if (!lock())
		return NULL;

	struct dstr trigger = {0};
	struct obs_hotkeys_background *background = &obs->hotkeys.background;
	bool found = background->shortcut_trigger &&
		     background->shortcut_trigger(background->wake_data, combo, &trigger);
	unlock();

	if (!found) {
		dstr_free(&trigger);
		return NULL;
	}
	return trigger.array;
}

#undef lock
#undef unlock
