#include "libobs/build/libobs.pch.h"

extern char *find_libobs_data_file(const char *file);

char *obs_find_data_file(const char *file)
{
	struct dstr path = {0};

	/* The libobs data directory first, then each added path in order:
	 * the first hit wins (obs_core_setup_rules_data_file_found()). */
	char *result = find_libobs_data_file(file);
	if (obs_core_setup_rules_data_file_found(result != NULL))
		return result;

	for (size_t i = 0; i < core_module_paths.num; ++i) {
		if (obs_core_setup_rules_data_file_found(check_path(file, core_module_paths.array[i].array, &path)))
			return path.array;
	}

	blog(LOG_ERROR, "Failed to find file '%s' in libobs data directory", file);

	dstr_free(&path);
	return NULL;
}
