/* Original extracted C getters and the real private layout. No stand-in hotkey. */
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#define obs_hotkey_get_description oracle_obs_hotkey_get_description
#define obs_hotkey_get_registerer_type oracle_obs_hotkey_get_registerer_type
#define obs_hotkey_get_registerer oracle_obs_hotkey_get_registerer
#define obs_hotkey_get_pair_partner_id oracle_obs_hotkey_get_pair_partner_id
#include "obs-hotkey-object-getters.c.h"
#undef obs_hotkey_get_description
#undef obs_hotkey_get_registerer_type
#undef obs_hotkey_get_registerer
#undef obs_hotkey_get_pair_partner_id
#include "hotkey_object_fields.inc"

struct hotkey_object_fixture {
	struct obs_hotkey key;
	struct obs_hotkey before;
};

void oracle_hotkey_object_layout(size_t *values);
void oracle_hotkey_object_layout(size_t *values)
{
	const size_t layout[] = {
		sizeof(struct obs_hotkey_object_fields),
		_Alignof(struct obs_hotkey_object_fields),
		offsetof(struct obs_hotkey_object_fields, description),
		offsetof(struct obs_hotkey_object_fields, registerer_type),
		offsetof(struct obs_hotkey_object_fields, registerer),
		offsetof(struct obs_hotkey_object_fields, pair_partner_id),
		sizeof(obs_hotkey_registerer_t),
		_Alignof(obs_hotkey_registerer_t),
		OBS_HOTKEY_REGISTERER_FRONTEND,
		OBS_HOTKEY_REGISTERER_SOURCE,
		OBS_HOTKEY_REGISTERER_OUTPUT,
		OBS_HOTKEY_REGISTERER_ENCODER,
		OBS_HOTKEY_REGISTERER_SERVICE,
	};
	memcpy(values, layout, sizeof(layout));
}

void *oracle_hotkey_object_new(char *description, obs_hotkey_registerer_t kind, void *registerer, size_t partner);
void *oracle_hotkey_object_new(char *description, obs_hotkey_registerer_t kind, void *registerer, size_t partner)
{
	struct hotkey_object_fixture *fixture = calloc(1, sizeof(*fixture));
	if (!fixture)
		return NULL;
	fixture->key.description = description;
	fixture->key.registerer_type = kind;
	fixture->key.registerer = registerer;
	fixture->key.pair_partner_id = partner;
	fixture->key.id = 19;
	fixture->key.pressed = 3;
	memcpy(&fixture->before, &fixture->key, sizeof(fixture->key));
	return fixture;
}

bool oracle_hotkey_object_unchanged(const void *handle);
bool oracle_hotkey_object_unchanged(const void *handle)
{
	const struct hotkey_object_fixture *fixture = handle;
	return memcmp(&fixture->key, &fixture->before, sizeof(fixture->key)) == 0;
}

void oracle_hotkey_object_free(void *handle);
void oracle_hotkey_object_free(void *handle)
{
	free(handle);
}
