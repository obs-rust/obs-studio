/* Real C getter bodies and automatically discovered private field adapter.
 * NULL is excluded: the original C getters dereference their source directly. */
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#define obs_source_get_texcoords_centered oracle_obs_source_get_texcoords_centered
#define obs_source_is_hidden oracle_obs_source_is_hidden
#include "obs-source-boolean-queries.c.h"
#undef obs_source_get_texcoords_centered
#undef obs_source_is_hidden
#include "source_boolean_query_fields.inc"

struct source_boolean_fixture {
	struct obs_source source;
	unsigned char snapshot[sizeof(struct obs_source)];
};

void oracle_source_boolean_layout(size_t *layout);
void oracle_source_boolean_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_source_boolean_query_fields);
	layout[1] = _Alignof(struct obs_source_boolean_query_fields);
	layout[2] = offsetof(struct obs_source_boolean_query_fields, texcoords_centered);
	layout[3] = offsetof(struct obs_source_boolean_query_fields, hidden);
	layout[4] = sizeof(bool);
	layout[5] = _Alignof(bool);
}

void *oracle_source_boolean_new(bool centered, bool hidden, bool enabled, uint32_t flags);
void *oracle_source_boolean_new(bool centered, bool hidden, bool enabled, uint32_t flags)
{
	struct source_boolean_fixture *fixture = calloc(1, sizeof(*fixture));
	if (fixture) {
		fixture->source.texcoords_centered = centered;
		fixture->source.temp_removed = hidden;
		fixture->source.enabled = enabled;
		fixture->source.flags = flags;
		memcpy(fixture->snapshot, &fixture->source, sizeof(fixture->source));
	}
	return fixture;
}

bool oracle_source_boolean_unchanged(const void *source);
bool oracle_source_boolean_unchanged(const void *source)
{
	const struct source_boolean_fixture *fixture = source;
	return memcmp(&fixture->source, fixture->snapshot, sizeof(fixture->source)) == 0;
}

void oracle_source_boolean_free(void *source);
void oracle_source_boolean_free(void *source)
{
	free(source);
}
