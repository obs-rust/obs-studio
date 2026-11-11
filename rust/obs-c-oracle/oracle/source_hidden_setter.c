/* Original setter and the real private layout. NULL is outside the C contract. */
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#define obs_source_set_hidden oracle_obs_source_set_hidden
#include "obs-source-hidden-setter.c.h"
#undef obs_source_set_hidden
#include "source_hidden_setter_fields.inc"

void *oracle_source_hidden_new(bool centered, bool hidden, bool enabled, uint32_t flags);
void *oracle_source_hidden_new(bool centered, bool hidden, bool enabled, uint32_t flags)
{
	struct obs_source *source = calloc(1, sizeof(*source));
	if (source) {
		source->texcoords_centered = centered;
		source->temp_removed = hidden;
		source->enabled = enabled;
		source->flags = flags;
	}
	return source;
}

void oracle_source_hidden_free(void *source);
void oracle_source_hidden_free(void *source)
{
	free(source);
}

bool oracle_source_hidden_equal(const void *left, const void *right);
bool oracle_source_hidden_equal(const void *left, const void *right)
{
	return memcmp(left, right, sizeof(struct obs_source)) == 0;
}

bool oracle_source_hidden_value(const void *source);
bool oracle_source_hidden_value(const void *source)
{
	return ((const struct obs_source *)source)->temp_removed;
}

void oracle_source_hidden_bool_layout(size_t *layout);
void oracle_source_hidden_bool_layout(size_t *layout)
{
	layout[0] = sizeof(bool);
	layout[1] = _Alignof(bool);
}
