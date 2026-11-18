#include "libobs/build/libobs.pch.h"

/* Original obs-source.c.h helper, isolated for the reference-only group. */
static inline bool source_missing_files_data_valid(const struct obs_source *source, const char *f)
{
	return obs_source_valid(source, f) && source->context.data;
}

obs_missing_files_t *obs_source_get_missing_files(const obs_source_t *source)
{
	if (!source_missing_files_data_valid(source, "obs_source_get_missing_files"))
		return obs_missing_files_create();

	if (source->info.missing_files) {
		return source->info.missing_files(source->context.data);
	}

	return obs_missing_files_create();
}
