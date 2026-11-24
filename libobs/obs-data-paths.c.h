#include "libobs/build/libobs.pch.h"

/* Separate from actual context initialization
 * since this can be set before startup and persist
 * after shutdown. */
static DARRAY(struct dstr) core_module_paths = {0};

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
