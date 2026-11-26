#include "libobs/build/libobs.pch.h"

/* obs-source.c.h's data_valid() is static there; this extraction keeps its
 * one-line body under its own name, mapped onto the original name and
 * scoped to this file, so the bodies stay verbatim and the file also builds
 * in a translation unit of its own. */
static inline bool obs_source_get_size_data_valid(const struct obs_source *source, const char *f)
{
	return obs_source_valid(source, f) && source->context.data;
}
#define data_valid obs_source_get_size_data_valid

/* Where obs_source_get_width() and obs_source_get_height() start a source
 * that is not a filter. The caller holds its filter_mutex. */
static const obs_source_t *size_start(const obs_source_t *source)
{
	switch (obs_source_size_entry((int)source->info.type, source->filters.num != 0)) {
	case OBS_SOURCE_SIZE_ENTRY_FIRST_FILTER:
		return source->filters.array[0];
	case OBS_SOURCE_SIZE_ENTRY_SELF:
		break;
	}

	return source;
}

static uint32_t get_recurse_width(obs_source_t *source)
{
	uint32_t width;

	pthread_mutex_lock(&source->filter_mutex);

	width = obs_source_base_width(size_start(source));

	pthread_mutex_unlock(&source->filter_mutex);

	return width;
}

static uint32_t get_recurse_height(obs_source_t *source)
{
	uint32_t height;

	pthread_mutex_lock(&source->filter_mutex);

	height = obs_source_base_height(size_start(source));

	pthread_mutex_unlock(&source->filter_mutex);

	return height;
}

uint32_t obs_source_get_width(obs_source_t *source)
{
	if (!data_valid(source, "obs_source_get_width"))
		return 0;

	/* a filter starts at itself (obs_source_size_entry()) and takes no
	 * lock: its filter_mutex guards filters it never reads */
	return (source->info.type != OBS_SOURCE_TYPE_FILTER) ? get_recurse_width(source)
							     : obs_source_base_width(source);
}

uint32_t obs_source_get_height(obs_source_t *source)
{
	if (!data_valid(source, "obs_source_get_height"))
		return 0;

	/* as obs_source_get_width() */
	return (source->info.type != OBS_SOURCE_TYPE_FILTER) ? get_recurse_height(source)
							     : obs_source_base_height(source);
}

#undef data_valid
