#include "libobs/build/libobs.pch.h"

extern bool obs_hotkey_seam_lock(void);
extern void obs_hotkey_seam_unlock(void);

/* Private live access for the background shortcut enumeration; nothing here
 * is EXPORT. The list is read only while the hotkey mutex is held. */
bool obs_hotkey_background_enum_lock(void);
bool obs_hotkey_background_enum_lock(void)
{
	return obs_hotkey_seam_lock();
}

const struct darray *obs_hotkey_background_enum_fields(void);
const struct darray *obs_hotkey_background_enum_fields(void)
{
	return &obs->hotkeys.background.shortcuts.da;
}

void obs_hotkey_background_enum_unlock(void);
void obs_hotkey_background_enum_unlock(void)
{
	obs_hotkey_seam_unlock();
}
