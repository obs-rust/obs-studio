#include "libobs/build/libobs.pch.h"

/* Original obs-source.c.h helper, isolated for the reference-only group. */
static inline bool source_replace_missing_file_data_valid(const struct obs_source *source, const char *f)
{
	return obs_source_valid(source, f) && source->context.data;
}

void obs_source_replace_missing_file(obs_missing_file_cb cb, obs_source_t *source, const char *new_path, void *data)
{
	if (!source_replace_missing_file_data_valid(source, "obs_source_replace_missing_file"))
		return;

	cb(source->context.data, new_path, data);
}
