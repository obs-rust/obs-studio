#include "libobs/build/libobs.pch.h"

extern bool obs_hotkey_seam_lock(void);
extern void obs_hotkey_seam_unlock(void);

bool obs_hotkey_background_error_lock(void);
bool obs_hotkey_background_error_lock(void)
{
	return obs_hotkey_seam_lock();
}

char *const *obs_hotkey_background_error_fields(void);
char *const *obs_hotkey_background_error_fields(void)
{
	return &obs->hotkeys.background.error;
}

void obs_hotkey_background_error_unlock(void);
void obs_hotkey_background_error_unlock(void)
{
	obs_hotkey_seam_unlock();
}
