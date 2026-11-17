/* Original extracted setters and the real production lookup adapter. */
#include "libobs/build/libobs.pch.h"
#include <stddef.h>
#include <string.h>

static _Thread_local struct obs_core *identity_setter_core;
void oracle_obs_hotkey_set_name(obs_hotkey_id id, const char *name);
void oracle_obs_hotkey_set_description(obs_hotkey_id id, const char *description);
#define obs identity_setter_core
#define obs_hotkey_set_name oracle_obs_hotkey_set_name
#define obs_hotkey_set_description oracle_obs_hotkey_set_description
#include "libobs/obs-hotkey-identity-setters.c.h"
#undef obs_hotkey_set_name
#undef obs_hotkey_set_description
#define obs_hotkey_identity_slot oracle_obs_hotkey_identity_slot
#include "hotkey_identity_setter_fields.inc"
#undef obs_hotkey_identity_slot
#undef obs

/* Match the registry hash for real, collision-capable uthash lookups. */
#undef HASH_FUNCTION
#define HASH_FUNCTION(s, len, hashv) (hashv) = (unsigned int)(*s % UINT_MAX)

bool oracle_hotkey_identity_setter_check(size_t id, bool description, const char *input,
					 void (*setter)(obs_hotkey_id, const char *));
bool oracle_hotkey_identity_setter_check(size_t id, bool description, const char *input,
					 void (*setter)(obs_hotkey_id, const char *))
{
	struct obs_core *cores = bzalloc(2 * sizeof(*cores));
	struct obs_hotkey keys[2][2] = {0};
	struct obs_hotkey before[2][2];
	for (size_t w = 0; w < 2; w++) {
		for (size_t i = 0; i < 2; i++) {
			struct obs_hotkey *key = &keys[w][i];
			key->id = i == 0 ? 0 : 17;
			key->name = bstrdup("old.name");
			key->description = bstrdup("Old");
			key->pressed = 3;
			key->pair_partner_id = 19;
			HASH_ADD(hh, cores[w].hotkeys.hotkeys, id, sizeof(size_t), key);
		}
	}
	memcpy(before, keys, sizeof(keys));
	identity_setter_core = &cores[0];
	if (description)
		oracle_obs_hotkey_set_description(id, input);
	else
		oracle_obs_hotkey_set_name(id, input);
	identity_setter_core = &cores[1];
	setter(id, input);
	bool equal = true;
	for (size_t i = 0; i < 2; i++) {
		const char *left[] = {keys[0][i].name, keys[0][i].description};
		const char *right[] = {keys[1][i].name, keys[1][i].description};
		for (size_t f = 0; f < 2; f++) {
			if (!left[f] || !right[f])
				equal = equal && left[f] == right[f];
			else
				equal = equal && strcmp(left[f], right[f]) == 0;
		}
		for (size_t w = 0; w < 2; w++) {
			struct obs_hotkey after;
			memcpy(&after, &keys[w][i], sizeof(after));
			after.name = before[w][i].name;
			after.description = before[w][i].description;
			equal = equal && memcmp(&after, &before[w][i], sizeof(after)) == 0;
		}
	}
	for (size_t w = 0; w < 2; w++) {
		for (size_t i = 0; i < 2; i++) {
			HASH_DEL(cores[w].hotkeys.hotkeys, &keys[w][i]);
			bfree(keys[w][i].name);
			bfree(keys[w][i].description);
		}
	}
	identity_setter_core = NULL;
	bfree(cores);
	return equal;
}

#undef HASH_FUNCTION
#define HASH_FUNCTION HASH_SFH
