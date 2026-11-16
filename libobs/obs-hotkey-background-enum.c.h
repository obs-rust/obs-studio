#include "libobs/build/libobs.pch.h"

/* Preserve the C body verbatim through the existing private mutex seam. */
#define lock obs_hotkey_seam_lock
#define unlock obs_hotkey_seam_unlock

void obs_hotkeys_enum_background_shortcuts(obs_hotkey_background_enum_func func, void *data)
{
	if (!func || !lock())
		return;

	struct obs_hotkeys_background *background = &obs->hotkeys.background;
	for (size_t i = 0; i < background->shortcuts.num; i++) {
		struct obs_hotkey_background_shortcut *shortcut = &background->shortcuts.array[i];
		if (!func(data, shortcut->combo, shortcut->trigger, shortcut->bound))
			break;
	}
	unlock();
}

#undef unlock
#undef lock
