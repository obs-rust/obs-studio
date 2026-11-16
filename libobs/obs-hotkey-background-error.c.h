#include "libobs/build/libobs.pch.h"

extern bool obs_hotkey_seam_lock(void);
extern void obs_hotkey_seam_unlock(void);

/* Preserve the extracted body while using the existing private mutex seam. */
#define lock obs_hotkey_seam_lock
#define unlock obs_hotkey_seam_unlock

char *obs_hotkeys_get_background_error(void)
{
	if (!lock())
		return NULL;

	char *error = obs->hotkeys.background.error ? bstrdup(obs->hotkeys.background.error) : NULL;
	unlock();
	return error;
}

#undef lock
#undef unlock
