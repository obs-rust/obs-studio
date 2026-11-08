/* Original query and automatically discovered real-layout adapter. */
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#define obs_obj_invalid oracle_obs_obj_invalid
#include "obs-object-validity-query.c.h"
#undef obs_obj_invalid
#include "object_validity_fields.inc"

struct object_validity_fixture {
	struct obs_context_data context;
	struct obs_context_data before;
	uint64_t payload;
	uint64_t payload_before;
};

void oracle_object_validity_layout(size_t *layout);
void oracle_object_validity_layout(size_t *layout)
{
	layout[0] = sizeof(void *);
	layout[1] = _Alignof(void *);
	layout[2] = sizeof(void *const *);
	layout[3] = _Alignof(void *const *);
	layout[4] = sizeof(bool);
	layout[5] = _Alignof(bool);
}

void *oracle_object_validity_new(uint32_t type, bool privacy, bool populated, uint64_t payload);
void *oracle_object_validity_new(uint32_t type, bool privacy, bool populated, uint64_t payload)
{
	struct object_validity_fixture *fixture = calloc(1, sizeof(*fixture));
	if (!fixture)
		return NULL;
	fixture->context.type = (enum obs_obj_type)type;
	fixture->context.private = privacy;
	fixture->context.name = "object validity sentinel";
	fixture->payload = payload;
	fixture->payload_before = payload;
	fixture->context.data = populated ? &fixture->payload : NULL;
	memcpy(&fixture->before, &fixture->context, sizeof(fixture->context));
	return fixture;
}

bool oracle_object_validity_unchanged(const void *handle);
bool oracle_object_validity_unchanged(const void *handle)
{
	const struct object_validity_fixture *fixture = handle;
	return memcmp(&fixture->context, &fixture->before, sizeof(fixture->context)) == 0 &&
	       fixture->payload == fixture->payload_before;
}

void oracle_object_validity_free(void *handle);
void oracle_object_validity_free(void *handle)
{
	free(handle);
}
