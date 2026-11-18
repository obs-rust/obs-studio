#include "libobs/build/libobs.pch.h"

/* Defined in obs-source.c.h later in this sorted unity group. */
static inline bool data_valid(const struct obs_source *source, const char *f);

obs_missing_files_t *obs_source_get_missing_files(const obs_source_t *source)
{
	if (!data_valid(source, "obs_source_get_missing_files"))
		return obs_missing_files_create();

	if (source->info.missing_files) {
		return source->info.missing_files(source->context.data);
	}

	return obs_missing_files_create();
}
