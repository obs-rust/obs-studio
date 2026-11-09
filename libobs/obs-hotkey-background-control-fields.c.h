#include "libobs/build/libobs.pch.h"

extern bool obs_hotkey_seam_lock(void);
extern void obs_hotkey_seam_unlock(void);

bool obs_hotkey_background_control_lock(void);
bool *obs_hotkey_background_control_retry(void);
bool *obs_hotkey_background_control_configure(void);
bool *obs_hotkey_background_control_probe(void);
uint32_t *obs_hotkey_background_control_flags(void);
void obs_hotkey_background_control_wake(void);
void obs_hotkey_background_control_unlock(void);

bool obs_hotkey_background_control_lock(void)
{
	return obs_hotkey_seam_lock();
}

bool *obs_hotkey_background_control_retry(void)
{
	return &obs->hotkeys.background.retry;
}

bool *obs_hotkey_background_control_configure(void)
{
	return &obs->hotkeys.background.configure;
}

bool *obs_hotkey_background_control_probe(void)
{
	return &obs->hotkeys.background.probe;
}

uint32_t *obs_hotkey_background_control_flags(void)
{
	return &obs->hotkeys.background.flags;
}

void obs_hotkey_background_control_wake(void)
{
	if (obs->hotkeys.background.wake)
		obs->hotkeys.background.wake(obs->hotkeys.background.wake_data);
}

void obs_hotkey_background_control_unlock(void)
{
	obs_hotkey_seam_unlock();
}
