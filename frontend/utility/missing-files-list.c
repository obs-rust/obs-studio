#include "missing-files-list.h"

obs_missing_files_t *missing_files_list_create(bool check_disabled)
{
	return check_disabled ? NULL : obs_missing_files_create();
}

static void add_filter_files(obs_source_t *parent, obs_source_t *filter, void *data)
{
	obs_missing_files_t *files = data;
	obs_missing_files_t *filter_files = obs_source_get_missing_files(filter);

	(void)parent;
	obs_missing_files_append(files, filter_files);
	obs_missing_files_destroy(filter_files);
}

void missing_files_list_add_source(void *data, obs_source_t *source)
{
	obs_missing_files_t *files = data;
	obs_missing_files_t *source_files;

	if (!files)
		return;

	source_files = obs_source_get_missing_files(source);
	obs_missing_files_append(files, source_files);
	obs_missing_files_destroy(source_files);

	obs_source_enum_filters(source, add_filter_files, files);
}
