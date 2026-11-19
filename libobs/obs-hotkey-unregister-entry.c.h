#include "libobs/build/libobs.pch.h"

extern void obs_hotkey_unregister_one(obs_hotkey_id id);
void obs_hotkey_unregister_pair_call(obs_hotkey_pair_id id);

/* Reuse the registry lock and existing removers; keep the public bodies verbatim. */
#define lock obs_hotkey_seam_lock
#define unlock obs_hotkey_seam_unlock
#define unregister_hotkey obs_hotkey_unregister_one
#define unregister_hotkey_pair obs_hotkey_unregister_pair_call

void obs_hotkey_unregister(obs_hotkey_id id)
{
	if (!lock())
		return;

	unregister_hotkey(id);
	unlock();
}

void obs_hotkey_pair_unregister(obs_hotkey_pair_id id)
{
	if (!lock())
		return;

	unregister_hotkey_pair(id);
	unlock();
}

#undef lock
#undef unlock
#undef unregister_hotkey
#undef unregister_hotkey_pair
