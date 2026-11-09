#include "libobs/build/libobs.pch.h"

extern bool obs_hotkey_seam_lock(void);
extern void obs_hotkey_seam_unlock(void);

bool obs_hotkey_background_app_id_lock(void)
{
	return obs_hotkey_seam_lock();
}

const char *const *obs_hotkey_background_app_id_fields(void)
{
	return &obs->hotkeys.background.app_id;
}

void obs_hotkey_background_app_id_unlock(void)
{
	obs_hotkey_seam_unlock();
}
