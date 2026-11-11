#include "libobs/build/libobs.pch.h"

/* Keep the registry's integer-key hash when compiling this unit alone. */
#undef HASH_FUNCTION
#define HASH_FUNCTION(s, len, hashv) (hashv) = (unsigned int)(*s % UINT_MAX)
#define HASH_FIND_HKEY(head, id, out) HASH_FIND(hh, head, &(id), sizeof(size_t), out)

#define lock obs_hotkey_seam_lock
#define unlock obs_hotkey_seam_unlock

extern bool obs_hotkey_seam_lock(void);
extern void obs_hotkey_seam_unlock(void);

void obs_hotkey_trigger_routed_callback(obs_hotkey_id id, bool pressed)
{
	if (!lock())
		return;

	if (!obs->hotkeys.reroute_hotkeys)
		goto unlock;

	obs_hotkey_t *hotkey;
	HASH_FIND_HKEY(obs->hotkeys.hotkeys, id, hotkey);
	if (!hotkey)
		goto unlock;

	hotkey->func(hotkey->data, id, hotkey, pressed);

unlock:
	unlock();
}

#undef lock
#undef unlock
#undef HASH_FIND_HKEY
#undef HASH_FUNCTION
#define HASH_FUNCTION HASH_SFH
