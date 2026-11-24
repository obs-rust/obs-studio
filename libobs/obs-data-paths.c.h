#include "libobs/build/libobs.pch.h"

/* The list is obs-data-paths-fields.c.h's (#5397), shared with
 * obs_find_data_file(); these original wrappers reach it through its
 * accessor. */
#define core_module_paths (*obs_data_paths_list())

void obs_add_data_path(const char *path)
{
	struct dstr *new_path = da_push_back_new(core_module_paths);
	dstr_init_copy(new_path, path);
}

bool obs_remove_data_path(const char *path)
{
	for (size_t i = 0; i < core_module_paths.num; ++i) {
		int result = dstr_cmp(&core_module_paths.array[i], path);

		if (obs_core_setup_rules_data_path_match(result)) {
			dstr_free(&core_module_paths.array[i]);
			da_erase(core_module_paths, i);
			return true;
		}
	}

	return false;
}

#undef core_module_paths
