#include "libobs/build/libobs.pch.h"

#define lock obs_hotkey_seam_lock
#define unlock obs_hotkey_seam_unlock

extern bool obs_hotkey_seam_lock(void);
extern void obs_hotkey_seam_unlock(void);

const char *obs_hotkeys_get_background_app_id(void)
{
	if (!lock())
		return NULL;

	const char *app_id = obs->hotkeys.background.app_id;
	unlock();
	return app_id;
}

#undef lock
#undef unlock
