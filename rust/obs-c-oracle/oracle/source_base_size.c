/* Original C traversal and independently compiled routing; no behavior exclusions. */
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#define obs_source_size_route oracle_source_base_route
#define obs_source_size_entry oracle_source_base_entry
#include "obs-source-size.c.h"
#define obs_source_get_base_width oracle_obs_source_get_base_width
#define obs_source_get_base_height oracle_obs_source_get_base_height
#define obs_source_base_width oracle_obs_source_base_width
#define obs_source_base_height oracle_obs_source_base_height
#include "obs-source-base-size.c.h"
#undef obs_source_get_base_width
#undef obs_source_get_base_height
#undef obs_source_base_width
#undef obs_source_base_height
#undef obs_source_size_route
#undef obs_source_size_entry
#include "source_base_size_fields.inc"

struct base_size_fixture {
	struct obs_source source[3];
	struct obs_source before[3];
	uint32_t dimensions[3][2];
};
static uint32_t base_size_width(void *data)
{
	return ((uint32_t *)data)[0];
}
static uint32_t base_size_height(void *data)
{
	return ((uint32_t *)data)[1];
}

void *oracle_source_base_size_new(const uint32_t *values);
void *oracle_source_base_size_new(const uint32_t *values)
{
	struct base_size_fixture *fixture = calloc(1, sizeof(*fixture));
	if (!fixture)
		return NULL;
	for (size_t i = 0; i < 3; i++) {
		const uint32_t *v = values + i * 8;
		struct obs_source *s = &fixture->source[i];
		s->info.type = (enum obs_source_type)(v[0] % 3);
		s->enabled = (v[1] & 1) != 0;
		s->info.get_width = (v[1] & 2) ? base_size_width : NULL;
		s->info.get_height = (v[1] & 4) ? base_size_height : NULL;
		s->context.data = (v[1] & 8) ? fixture->dimensions[i] : NULL;
		s->filter_parent = (v[1] & 16) ? &fixture->source[0] : NULL;
		s->async_active = (v[1] & 32) != 0;
		s->filter_target = (v[1] & 64) && i < 2 ? &fixture->source[i + 1] : NULL;
		s->async_width = v[2];
		s->async_height = v[3];
		s->transition_actual_cx = (long)(int32_t)v[4];
		s->transition_actual_cy = (long)(int32_t)v[5];
		s->async_rotation = (long)(int32_t)v[6];
		fixture->dimensions[i][0] = v[7];
		fixture->dimensions[i][1] = v[2];
	}
	memcpy(fixture->before, fixture->source, sizeof(fixture->source));
	return fixture;
}
bool oracle_source_base_size_unchanged(const void *handle);
bool oracle_source_base_size_unchanged(const void *handle)
{
	const struct base_size_fixture *fixture = handle;
	return memcmp(fixture->source, fixture->before, sizeof(fixture->source)) == 0;
}
void oracle_source_base_size_free(void *handle);
void oracle_source_base_size_free(void *handle)
{
	free(handle);
}
void oracle_source_base_size_layout(size_t *layout);
void oracle_source_base_size_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_source_base_size_fields);
	layout[1] = _Alignof(struct obs_source_base_size_fields);
	layout[2] = offsetof(struct obs_source_base_size_fields, source_type);
	layout[3] = offsetof(struct obs_source_base_size_fields, data);
	layout[4] = offsetof(struct obs_source_base_size_fields, get_width);
	layout[5] = offsetof(struct obs_source_base_size_fields, get_height);
	layout[6] = offsetof(struct obs_source_base_size_fields, filter_parent);
	layout[7] = offsetof(struct obs_source_base_size_fields, filter_target);
	layout[8] = offsetof(struct obs_source_base_size_fields, enabled);
	layout[9] = offsetof(struct obs_source_base_size_fields, async_active);
	layout[10] = offsetof(struct obs_source_base_size_fields, async_rotation);
	layout[11] = offsetof(struct obs_source_base_size_fields, transition_width);
	layout[12] = offsetof(struct obs_source_base_size_fields, transition_height);
	layout[13] = offsetof(struct obs_source_base_size_fields, async_width);
	layout[14] = offsetof(struct obs_source_base_size_fields, async_height);
}
