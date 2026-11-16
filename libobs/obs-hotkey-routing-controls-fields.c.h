#include "libobs/build/libobs.pch.h"

extern bool obs_hotkey_seam_lock(void);
extern void obs_hotkey_seam_unlock(void);

bool obs_hotkey_routing_lock(void);
obs_hotkey_callback_router_func *obs_hotkey_routing_callback_field(void);
void **obs_hotkey_routing_data_field(void);
bool *obs_hotkey_routing_enabled_field(void);
void obs_hotkey_routing_unlock(void);

bool obs_hotkey_routing_lock(void)
{
	return obs_hotkey_seam_lock();
}

obs_hotkey_callback_router_func *obs_hotkey_routing_callback_field(void)
{
	return &obs->hotkeys.router_func;
}

void **obs_hotkey_routing_data_field(void)
{
	return &obs->hotkeys.router_func_data;
}

bool *obs_hotkey_routing_enabled_field(void)
{
	return &obs->hotkeys.reroute_hotkeys;
}

void obs_hotkey_routing_unlock(void)
{
	obs_hotkey_seam_unlock();
}
