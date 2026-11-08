/* Original query and discovered field adapter over the real private layout. */
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#define obs_weak_object_references_object oracle_obs_weak_object_references_object
#include "obs-weak-object-identity.c.h"
#undef obs_weak_object_references_object
#include "weak_object_identity_fields.inc"

struct weak_object_identity_fixture {
	struct obs_weak_object weak;
	struct obs_weak_object before;
	struct obs_context_data objects[2];
};

void *oracle_weak_object_identity_new(unsigned stored);
void *oracle_weak_object_identity_new(unsigned stored)
{
	struct weak_object_identity_fixture *fixture = calloc(1, sizeof(*fixture));
	if (fixture) {
		fixture->weak.object = stored ? &fixture->objects[stored - 1] : NULL;
		memcpy(&fixture->before, &fixture->weak, sizeof(fixture->weak));
	}
	return fixture;
}
void *oracle_weak_object_identity_target(void *handle, unsigned target);
void *oracle_weak_object_identity_target(void *handle, unsigned target)
{
	struct weak_object_identity_fixture *fixture = handle;
	return target ? &fixture->objects[target - 1] : NULL;
}
bool oracle_weak_object_identity_unchanged(const void *handle);
bool oracle_weak_object_identity_unchanged(const void *handle)
{
	const struct weak_object_identity_fixture *fixture = handle;
	return memcmp(&fixture->weak, &fixture->before, sizeof(fixture->weak)) == 0;
}
void oracle_weak_object_identity_free(void *handle);
void oracle_weak_object_identity_free(void *handle)
{
	free(handle);
}
bool oracle_weak_object_identity_addresses(const obs_weak_object_t *weak);
bool oracle_weak_object_identity_addresses(const obs_weak_object_t *weak)
{
	return obs_weak_object_identity_fields(weak).object == &weak->object;
}
void oracle_weak_object_identity_layout(size_t *layout);
void oracle_weak_object_identity_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_weak_object_identity_fields);
	layout[1] = _Alignof(struct obs_weak_object_identity_fields);
	layout[2] = offsetof(struct obs_weak_object_identity_fields, object);
}
