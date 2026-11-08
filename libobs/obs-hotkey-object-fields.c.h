#include "libobs/build/libobs.pch.h"

struct obs_hotkey_object_fields obs_hotkey_object_fields(const obs_hotkey_t *key)
{
	return (struct obs_hotkey_object_fields){
		.description = &key->description,
		.registerer_type = &key->registerer_type,
		.registerer = &key->registerer,
		.pair_partner_id = &key->pair_partner_id,
	};
}
