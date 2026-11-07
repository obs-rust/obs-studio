/* Original C getters, real hotkeys and the production field adapter.
 * Production getters are only invoked through callbacks supplied by Rust. */
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#define obs_hotkey_get_id oracle_obs_hotkey_get_id
#define obs_hotkey_get_name oracle_obs_hotkey_get_name
#include "obs-hotkey-identity-getters.c.h"
#undef obs_hotkey_get_id
#undef obs_hotkey_get_name
struct obs_hotkey_identity_fields oracle_obs_hotkey_identity_fields(const obs_hotkey_t *key);
#define obs_hotkey_identity_fields(key) oracle_obs_hotkey_identity_fields(key)
#include "hotkey_identity_fields.inc"
#undef obs_hotkey_identity_fields

struct hotkey_identity_fixture {
	struct obs_hotkey key;
	struct obs_hotkey before;
};

void oracle_hotkey_identity_layout(size_t *values);
void oracle_hotkey_identity_layout(size_t *values)
{
	const size_t layout[] = {
		sizeof(struct obs_hotkey_identity_fields),
		_Alignof(struct obs_hotkey_identity_fields),
		offsetof(struct obs_hotkey_identity_fields, id),
		offsetof(struct obs_hotkey_identity_fields, name),
		sizeof(obs_hotkey_id),
		_Alignof(obs_hotkey_id),
	};
	memcpy(values, layout, sizeof(layout));
}

void *oracle_hotkey_identity_new(size_t id, char *name);
void *oracle_hotkey_identity_new(size_t id, char *name)
{
	struct hotkey_identity_fixture *fixture = calloc(1, sizeof(*fixture));
	if (!fixture)
		return NULL;
	fixture->key.id = id;
	fixture->key.name = name;
	fixture->key.pressed = 3;
	fixture->key.pair_partner_id = 19;
	memcpy(&fixture->before, &fixture->key, sizeof(fixture->key));
	return fixture;
}

bool oracle_hotkey_identity_probe(const void *handle, obs_hotkey_id (*get_id)(const obs_hotkey_t *),
				const char *(*get_name)(const obs_hotkey_t *));
bool oracle_hotkey_identity_probe(const void *handle, obs_hotkey_id (*get_id)(const obs_hotkey_t *),
				const char *(*get_name)(const obs_hotkey_t *))
{
	const struct hotkey_identity_fixture *fixture = handle;
	return get_id(&fixture->key) == oracle_obs_hotkey_get_id(&fixture->key) &&
	       get_name(&fixture->key) == oracle_obs_hotkey_get_name(&fixture->key) &&
	       memcmp(&fixture->key, &fixture->before, sizeof(fixture->key)) == 0;
}

void oracle_hotkey_identity_free(void *handle);
void oracle_hotkey_identity_free(void *handle)
{
	free(handle);
}
