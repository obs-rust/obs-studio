#include "libobs/build/libobs.pch.h"

/* Preserve the integer-key hash used by obs-hotkey.c.h. */
#undef HASH_FUNCTION
#define HASH_FUNCTION(s, len, hashv) (hashv) = (unsigned int)(*s % UINT_MAX)
#define HASH_FIND_HKEY(head, id, out) HASH_FIND(hh, head, &(id), sizeof(size_t), out)

void obs_hotkey_pair_set_names(obs_hotkey_pair_id id, const char *name0, const char *name1)
{
	obs_hotkey_pair_t *pair;

	HASH_FIND_HKEY(obs->hotkeys.hotkey_pairs, id, pair);
	if (!pair)
		return;

	obs_hotkey_set_name(pair->id[0], name0);
	obs_hotkey_set_name(pair->id[1], name1);
}

void obs_hotkey_pair_set_descriptions(obs_hotkey_pair_id id, const char *desc0, const char *desc1)
{
	obs_hotkey_pair_t *pair;

	HASH_FIND_HKEY(obs->hotkeys.hotkey_pairs, id, pair);
	if (!pair)
		return;

	obs_hotkey_set_description(pair->id[0], desc0);
	obs_hotkey_set_description(pair->id[1], desc1);
}

#undef HASH_FIND_HKEY
#undef HASH_FUNCTION
#define HASH_FUNCTION HASH_SFH
