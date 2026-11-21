#include "libobs/build/libobs.pch.h"

/* The call of a hotkey event: the hotkey's own callback, or the router when
 * callback rerouting is on (nothing when it is on and no router is set). The
 * callback ABI and the router (the frontend's) stay here. */
void obs_hotkey_dispatch(obs_hotkey_t *hotkey, bool pressed)
{
	if (!obs->hotkeys.reroute_hotkeys)
		hotkey->func(hotkey->data, hotkey->id, hotkey, pressed);
	else if (obs->hotkeys.router_func)
		obs->hotkeys.router_func(obs->hotkeys.router_func_data, hotkey->id, pressed);
}
