#include "libobs/build/libobs.pch.h"

/* Preserve the registry lock behavior while this unit is extracted for Rust. */
#define lock obs_hotkey_seam_lock
#define unlock obs_hotkey_seam_unlock

void obs_hotkey_set_callback_routing_func(obs_hotkey_callback_router_func func, void *data)
{
	if (!lock())
		return;

	obs->hotkeys.router_func = func;
	obs->hotkeys.router_func_data = data;
	unlock();
}

void obs_hotkey_enable_callback_rerouting(bool enable)
{
	if (!lock())
		return;

	obs->hotkeys.reroute_hotkeys = enable;
	unlock();
}

#undef unlock
#undef lock
