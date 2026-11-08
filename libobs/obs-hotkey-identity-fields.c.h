#include "libobs/build/libobs.pch.h"

struct obs_hotkey_identity_fields obs_hotkey_identity_fields(const obs_hotkey_t *key)
{
	return (struct obs_hotkey_identity_fields){
		.id = &key->id,
		.name = &key->name,
	};
}
