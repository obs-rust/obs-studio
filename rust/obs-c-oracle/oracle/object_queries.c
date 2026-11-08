/* Original public queries and automatically discovered real-layout adapter. */
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#define obs_obj_get_type oracle_obs_obj_get_type
#define obs_obj_is_private oracle_obs_obj_is_private
#include "obs-object-queries.c.h"
#undef obs_obj_get_type
#undef obs_obj_is_private
#include "object_query_fields.inc"

struct object_query_fixture {
	struct obs_context_data context;
	struct obs_context_data before;
};

void oracle_object_query_layout(size_t *layout);
void oracle_object_query_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_object_query_fields);
	layout[1] = _Alignof(struct obs_object_query_fields);
	layout[2] = offsetof(struct obs_object_query_fields, type);
	layout[3] = offsetof(struct obs_object_query_fields, private);
	layout[4] = sizeof(enum obs_obj_type);
	layout[5] = _Alignof(enum obs_obj_type);
	layout[6] = sizeof(bool);
	layout[7] = _Alignof(bool);
	layout[8] = OBS_OBJ_TYPE_INVALID;
	layout[9] = OBS_OBJ_TYPE_CANVAS;
}

void *oracle_object_query_new(uint32_t type, bool privacy);
void *oracle_object_query_new(uint32_t type, bool privacy)
{
	struct object_query_fixture *fixture = calloc(1, sizeof(*fixture));
	if (!fixture)
		return NULL;
	fixture->context.type = (enum obs_obj_type)type;
	fixture->context.private = privacy;
	fixture->context.name = "object query sentinel";
	fixture->context.data = &fixture->before;
	memcpy(&fixture->before, &fixture->context, sizeof(fixture->context));
	return fixture;
}

bool oracle_object_query_unchanged(const void *handle);
bool oracle_object_query_unchanged(const void *handle)
{
	const struct object_query_fixture *fixture = handle;
	return memcmp(&fixture->context, &fixture->before, sizeof(fixture->context)) == 0;
}

void oracle_object_query_free(void *handle);
void oracle_object_query_free(void *handle)
{
	free(handle);
}
