#include "libobs/build/libobs.pch.h"

/* Keep the extracted bodies verbatim; dependencies stay in obs-hotkey.c.h. */
#define lock obs_hotkey_seam_lock
#define unlock obs_hotkey_seam_unlock
#define enum_bindings obs_hotkey_seam_enum_bindings

void obs_enum_hotkeys(obs_hotkey_enum_func func, void *data)
{
	if (!lock())
		return;

	obs_hotkey_t *hk, *tmp;
	HASH_ITER (hh, obs->hotkeys.hotkeys, hk, tmp) {
		if (!func(data, hk->id, hk))
			break;
	}

	unlock();
}

void obs_enum_hotkey_bindings(obs_hotkey_binding_enum_func func, void *data)
{
	if (!lock())
		return;

	enum_bindings(func, data);
	unlock();
}

#undef enum_bindings
#undef unlock
#undef lock
