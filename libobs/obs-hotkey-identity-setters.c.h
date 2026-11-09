#include "libobs/build/libobs.pch.h"

/* Match the registry's integer-key hash when this unit is compiled alone. */
#undef HASH_FUNCTION
#define HASH_FUNCTION(s, len, hashv) (hashv) = (unsigned int)(*s % UINT_MAX)
#define HASH_FIND_HKEY(head, id, out) HASH_FIND(hh, head, &(id), sizeof(size_t), out)

void obs_hotkey_set_name(obs_hotkey_id id, const char *name)
{
	obs_hotkey_t *hotkey;
	HASH_FIND_HKEY(obs->hotkeys.hotkeys, id, hotkey);
	if (!hotkey)
		return;

	bfree(hotkey->name);
	hotkey->name = bstrdup(name);
}

void obs_hotkey_set_description(obs_hotkey_id id, const char *desc)
{
	obs_hotkey_t *hotkey;
	HASH_FIND_HKEY(obs->hotkeys.hotkeys, id, hotkey);
	if (!hotkey)
		return;

	bfree(hotkey->description);
	hotkey->description = bstrdup(desc);
}

#undef HASH_FIND_HKEY
#undef HASH_FUNCTION
#define HASH_FUNCTION HASH_SFH
