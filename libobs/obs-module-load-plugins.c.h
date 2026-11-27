#include "libobs/build/libobs.pch.h"

extern const char *get_module_extension(void);

void obs_load_plugins(struct obs_runtime_module_info *info, struct obs_module_failure_info *error)
{
	struct fail_info failure = {0};

	obs_module_paths_find(info->path_info.binary, info->path_info.data, info->type, get_module_extension(),
			      obs_host_add_module_extension, load_all_callback, &failure);

	error->count = failure.fail_count;
	error->failed_modules = strlist_split(failure.fail_modules.array, ';', false);
	dstr_free(&failure.fail_modules);
}
