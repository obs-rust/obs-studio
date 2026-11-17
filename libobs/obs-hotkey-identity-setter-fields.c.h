#include "libobs/build/libobs.pch.h"

/* The same integer hash as the unported registry. This seam only locates a
 * field; Rust owns replacement order and allocation. No new lock is added. */
#undef HASH_FUNCTION
#define HASH_FUNCTION(s, len, hashv) (hashv) = (unsigned int)(*s % UINT_MAX)

char **obs_hotkey_identity_slot(obs_hotkey_id id, bool description);
char **obs_hotkey_identity_slot(obs_hotkey_id id, bool description)
{
	obs_hotkey_t *key;
	HASH_FIND(hh, obs->hotkeys.hotkeys, &id, sizeof(size_t), key);
	if (!key)
		return NULL;
	return description ? &key->description : &key->name;
}

#undef HASH_FUNCTION
#define HASH_FUNCTION HASH_SFH
