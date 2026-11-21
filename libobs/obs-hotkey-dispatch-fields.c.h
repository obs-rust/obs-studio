#include "libobs/build/libobs.pch.h"

/* Private adapters for the Rust hotkey dispatch (obs-hotkey-dispatch.c.h).
 * The rerouting flag, the router slot and the callback ABI stay in C. The
 * flag and router adapters never touch the hotkey; the two calls are reached
 * only on a delivered path. */
bool obs_hotkey_dispatch_rerouting(void);
bool obs_hotkey_dispatch_rerouting(void)
{
	return obs->hotkeys.reroute_hotkeys;
}

bool obs_hotkey_dispatch_has_router(void);
bool obs_hotkey_dispatch_has_router(void)
{
	return obs->hotkeys.router_func != NULL;
}

void obs_hotkey_dispatch_call(obs_hotkey_t *hotkey, bool pressed);
void obs_hotkey_dispatch_call(obs_hotkey_t *hotkey, bool pressed)
{
	hotkey->func(hotkey->data, hotkey->id, hotkey, pressed);
}

void obs_hotkey_dispatch_route(obs_hotkey_t *hotkey, bool pressed);
void obs_hotkey_dispatch_route(obs_hotkey_t *hotkey, bool pressed)
{
	obs->hotkeys.router_func(obs->hotkeys.router_func_data, hotkey->id, pressed);
}
