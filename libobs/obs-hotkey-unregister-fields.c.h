#include "libobs/build/libobs.pch.h"

/* Private storage operations. The caller already holds the recursive hotkey lock. */
size_t obs_hotkey_unregister_next_id(void);
obs_hotkey_t *obs_hotkey_unregister_find(size_t id);
void obs_hotkey_unregister_remove(obs_hotkey_t *key);
int obs_hotkey_unregister_category(obs_hotkey_t *key);
void *obs_hotkey_unregister_owner(obs_hotkey_t *key);
void obs_hotkey_unregister_clear_owner(obs_hotkey_t *key);
void *obs_hotkey_unregister_name(obs_hotkey_t *key);
void *obs_hotkey_unregister_description(obs_hotkey_t *key);

#undef HASH_FUNCTION
#define HASH_FUNCTION(s, len, hashv) (hashv) = (unsigned int)(*s % UINT_MAX)

size_t obs_hotkey_unregister_next_id(void)
{
	return obs ? obs->hotkeys.next_id : 0;
}

obs_hotkey_t *obs_hotkey_unregister_find(size_t id)
{
	obs_hotkey_t *key;
	HASH_FIND(hh, obs->hotkeys.hotkeys, &id, sizeof(size_t), key);
	return key;
}

void obs_hotkey_unregister_remove(obs_hotkey_t *key)
{
	HASH_DEL(obs->hotkeys.hotkeys, key);
}

int obs_hotkey_unregister_category(obs_hotkey_t *key)
{
	return key->registerer_type;
}

void *obs_hotkey_unregister_owner(obs_hotkey_t *key)
{
	return key->registerer;
}

void obs_hotkey_unregister_clear_owner(obs_hotkey_t *key)
{
	key->registerer = NULL;
}

void *obs_hotkey_unregister_name(obs_hotkey_t *key)
{
	return key->name;
}

void *obs_hotkey_unregister_description(obs_hotkey_t *key)
{
	return key->description;
}

#undef HASH_FUNCTION
#define HASH_FUNCTION HASH_SFH
