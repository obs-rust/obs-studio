/* Original setter and real private layout; no behavior exclusions. */
#include <stdlib.h>
#include <string.h>
#define obs_source_set_async_rotation oracle_obs_source_set_async_rotation
#include "obs-source-async-rotation.c.h"
#undef obs_source_set_async_rotation
#include "source_async_rotation_fields.inc"

void *oracle_source_async_rotation_new(long initial);
void *oracle_source_async_rotation_new(long initial)
{
	obs_source_t *source = calloc(1, sizeof(*source));
	if (source)
		source->async_rotation = initial;
	return source;
}

long oracle_source_async_rotation_value(const obs_source_t *source);
long oracle_source_async_rotation_value(const obs_source_t *source)
{
	return source->async_rotation;
}

bool oracle_source_async_rotation_other_fields_unchanged(const obs_source_t *source);
bool oracle_source_async_rotation_other_fields_unchanged(const obs_source_t *source)
{
	struct obs_source actual, expected;
	memcpy(&actual, source, sizeof(actual));
	memset(&expected, 0, sizeof(expected));
	actual.async_rotation = 0;
	return memcmp(&actual, &expected, sizeof(actual)) == 0;
}

void oracle_source_async_rotation_free(void *source);
void oracle_source_async_rotation_free(void *source)
{
	free(source);
}
