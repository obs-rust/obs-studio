/* Real C getter and centered-texture setter (#5599) bodies and automatically
 * discovered private field adapter. NULL is excluded: the original C getters
 * and setter dereference their source directly. */
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#define obs_source_get_texcoords_centered oracle_obs_source_get_texcoords_centered
#define obs_source_is_hidden oracle_obs_source_is_hidden
#define obs_source_set_texcoords_centered oracle_obs_source_set_texcoords_centered
#include "obs-source-boolean-queries.c.h"
#include "obs-source-texcoords-centered-setter.c.h"
#undef obs_source_get_texcoords_centered
#undef obs_source_is_hidden
#undef obs_source_set_texcoords_centered
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

void oracle_source_boolean_offset(size_t *offset);
void oracle_source_boolean_offset(size_t *offset)
{
	offset[0] = offsetof(struct obs_source, texcoords_centered);
	offset[1] = sizeof(struct obs_source);
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

/* Every byte of the fixture holds fill, then the four fields hold their
 * values, and the snapshot follows, so a setter writing any other byte shows
 * in oracle_source_boolean_stored(). */
void *oracle_source_boolean_new_filled(unsigned char fill, bool centered, bool hidden, bool enabled);
void *oracle_source_boolean_new_filled(unsigned char fill, bool centered, bool hidden, bool enabled)
{
	struct source_boolean_fixture *fixture = malloc(sizeof(*fixture));
	if (fixture) {
		memset(&fixture->source, fill, sizeof(fixture->source));
		fixture->source.texcoords_centered = centered;
		fixture->source.temp_removed = hidden;
		fixture->source.enabled = enabled;
		memcpy(fixture->snapshot, &fixture->source, sizeof(fixture->source));
	}
	return fixture;
}

/* The fixture changed exactly in its centered-texture byte, which now holds
 * expected; the snapshot follows, so a later call compares against it. */
bool oracle_source_boolean_stored(void *source, bool expected);
bool oracle_source_boolean_stored(void *source, bool expected)
{
	struct source_boolean_fixture *fixture = source;
	memcpy(fixture->snapshot + offsetof(struct obs_source, texcoords_centered), &expected, sizeof(expected));
	return fixture->source.texcoords_centered == expected &&
	       memcmp(&fixture->source, fixture->snapshot, sizeof(fixture->source)) == 0;
}

/* Byte-for-byte equality of two fixtures' sources. */
bool oracle_source_boolean_equal(const void *left, const void *right);
bool oracle_source_boolean_equal(const void *left, const void *right)
{
	const struct source_boolean_fixture *l = left, *r = right;
	return memcmp(&l->source, &r->source, sizeof(l->source)) == 0;
}
