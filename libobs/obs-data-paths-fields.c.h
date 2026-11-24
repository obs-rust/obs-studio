#include "libobs/build/libobs.pch.h"

/* The list of extra data paths and the private adapters of the Rust
 * obs_add_data_path() and obs_remove_data_path() (obs-data-paths.c.h,
 * #5397): the count, an entry's string, the copying push and the erase. The
 * list and its strings stay C, on the libobs allocator; Rust decides what is
 * pushed, compared and erased.
 *
 * Separate from actual context initialization since this can be set before
 * startup and persist after shutdown. obs_find_data_file() (obs.c.h, same
 * group) reads core_module_paths directly. */
static obs_data_path_list_t core_module_paths = {0};

obs_data_path_list_t *obs_data_paths_list(void)
{
	return &core_module_paths;
}

size_t obs_data_paths_count(void);
size_t obs_data_paths_count(void)
{
	return core_module_paths.num;
}

/* NULL for an entry added as NULL or "" (dstr_init_copy() keeps none). */
const char *obs_data_paths_entry(size_t index);
const char *obs_data_paths_entry(size_t index)
{
	return core_module_paths.array[index].array;
}

void obs_data_paths_push_copy(const char *path);
void obs_data_paths_push_copy(const char *path)
{
	struct dstr *new_path = da_push_back_new(core_module_paths);
	dstr_init_copy(new_path, path);
}

void obs_data_paths_erase(size_t index);
void obs_data_paths_erase(size_t index)
{
	dstr_free(&core_module_paths.array[index]);
	da_erase(core_module_paths, index);
}
