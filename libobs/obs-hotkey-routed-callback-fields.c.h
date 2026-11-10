#include "libobs/build/libobs.pch.h"

extern bool obs_hotkey_seam_lock(void);
extern void obs_hotkey_seam_unlock(void);
extern obs_hotkey_t *obs_hotkey_seam_find(obs_hotkey_id id);

bool obs_hotkey_routed_callback_lock(void);
void obs_hotkey_routed_callback_unlock(void);
bool obs_hotkey_routed_callback_enabled(void);
obs_hotkey_t *obs_hotkey_routed_callback_find(obs_hotkey_id id);
void obs_hotkey_routed_callback_call(obs_hotkey_t *hotkey, obs_hotkey_id id, bool pressed);

bool obs_hotkey_routed_callback_lock(void)
{
	return obs_hotkey_seam_lock();
}

void obs_hotkey_routed_callback_unlock(void)
{
	obs_hotkey_seam_unlock();
}

bool obs_hotkey_routed_callback_enabled(void)
{
	return obs->hotkeys.reroute_hotkeys;
}

obs_hotkey_t *obs_hotkey_routed_callback_find(obs_hotkey_id id)
{
	return obs_hotkey_seam_find(id);
}

void obs_hotkey_routed_callback_call(obs_hotkey_t *hotkey, obs_hotkey_id id, bool pressed)
{
	hotkey->func(hotkey->data, id, hotkey, pressed);
}
