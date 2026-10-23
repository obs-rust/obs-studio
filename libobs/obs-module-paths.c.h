/******************************************************************************
    Copyright (C) 2023 by Lain Bailey <lain@obsproject.com>

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 2 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <http://www.gnu.org/licenses/>.
******************************************************************************/

#include "libobs/build/libobs-ported-module-paths.pch.h"

/* Module search paths, moved from obs-module.c: how a module's binary and
 * data paths are made from a search path, and where a module's file is.
 * Pure path logic: no libobs state. obs-module.c keeps the search path list
 * (obs->module_paths), the module list and the loading, and passes in what
 * these need: the paths, the module type, the module file extension
 * (get_module_extension()) and the host's binary extension hook
 * (obs_host_add_module_extension()). Declared in obs-internal.h. Ported to
 * rust/obs-core (module_paths): the hybrid libobs drops this file. */

static inline void make_data_dir(struct dstr *parsed_data_dir, const char *data_dir, const char *name)
{
	dstr_copy(parsed_data_dir, data_dir);
	dstr_replace(parsed_data_dir, "%module%", name);
	if (dstr_end(parsed_data_dir) == '/')
		dstr_resize(parsed_data_dir, parsed_data_dir->len - 1);
}

char *obs_module_paths_data_dir(const char *module_name, const char *data_dir)
{
	struct dstr parsed_data_dir = {0};
	bool found = false;

	make_data_dir(&parsed_data_dir, data_dir, module_name);

	found = os_file_exists(parsed_data_dir.array);

	if (!found && astrcmpi_n(module_name, "lib", 3) == 0)
		make_data_dir(&parsed_data_dir, data_dir, module_name + 3);

	return parsed_data_dir.array;
}

static bool parse_binary_from_directory(struct dstr *parsed_bin_path, const char *bin_path, const char *file,
					const char *ext, obs_module_add_extension_t add_extension)
{
	struct dstr directory = {0};
	bool found = true;

	dstr_copy(&directory, bin_path);
	dstr_replace(&directory, "%module%", file);
	if (dstr_end(&directory) != '/')
		dstr_cat_ch(&directory, '/');

	dstr_copy_dstr(parsed_bin_path, &directory);
	dstr_cat(parsed_bin_path, file);
	add_extension(parsed_bin_path);

	if (!os_file_exists(parsed_bin_path->array)) {
		/* Legacy fallback: Check for plugin with .so suffix*/
		dstr_cat(parsed_bin_path, ".so");
		/* if the file doesn't exist, check with 'lib' prefix */
		dstr_copy_dstr(parsed_bin_path, &directory);
		dstr_cat(parsed_bin_path, "lib");
		dstr_cat(parsed_bin_path, file);
		dstr_cat(parsed_bin_path, ext);

		/* if neither exist, don't include this as a library */
		if (!os_file_exists(parsed_bin_path->array)) {
			dstr_free(parsed_bin_path);
			found = false;
		}
	}

	dstr_free(&directory);
	return found;
}

static void process_found_module(const char *bin, const char *data, enum obs_runtime_module_type type, const char *ext,
				 obs_module_add_extension_t add_extension, const char *path, bool directory,
				 obs_find_module_callback2_t callback, void *param)
{
	struct obs_module_info2 info;
	struct dstr name = {0};
	struct dstr parsed_bin_path = {0};
	const char *file;
	char *parsed_data_dir;
	bool bin_found = true;

	file = strrchr(path, '/');
	file = file ? (file + 1) : path;

	if (strcmp(file, ".") == 0 || strcmp(file, "..") == 0)
		return;

	dstr_copy(&name, file);
	char *ext_start = strrchr(name.array, '.');
	if (ext_start)
		dstr_resize(&name, (size_t)(ext_start - name.array));

	if (!directory) {
		dstr_copy(&parsed_bin_path, path);
	} else {
		bin_found = parse_binary_from_directory(&parsed_bin_path, bin, name.array, ext, add_extension);
	}

	parsed_data_dir = obs_module_paths_data_dir(name.array, data);

	if (parsed_data_dir && bin_found) {
		info.bin_path = parsed_bin_path.array;
		info.data_path = parsed_data_dir;
		info.name = name.array;
		info.type = type;
		callback(param, &info);
	}

	bfree(parsed_data_dir);
	dstr_free(&name);
	dstr_free(&parsed_bin_path);
}

void obs_module_paths_find(const char *bin, const char *data, enum obs_runtime_module_type type, const char *ext,
			   obs_module_add_extension_t add_extension, obs_find_module_callback2_t callback, void *param)
{
	struct dstr search_path = {0};
	char *module_start;
	bool search_directories = false;
	os_glob_t *gi;

	dstr_copy(&search_path, bin);
	module_start = strstr(search_path.array, "%module%");
	if (module_start) {
		dstr_resize(&search_path, (size_t)(module_start - search_path.array));
		search_directories = true;
	}

	if (!dstr_is_empty(&search_path) && dstr_end(&search_path) != '/')
		dstr_cat_ch(&search_path, '/');

	dstr_cat_ch(&search_path, '*');
	if (!search_directories)
		dstr_cat(&search_path, ext);

	if (os_glob(search_path.array, 0, &gi) == 0) {
		for (size_t i = 0; i < gi->gl_pathc; i++) {
			if (search_directories == gi->gl_pathv[i].directory)
				process_found_module(bin, data, type, ext, add_extension, gi->gl_pathv[i].path,
						     search_directories, callback, param);
		}

		os_globfree(gi);
	}

	dstr_free(&search_path);
}

char *obs_module_paths_find_file(const char *data_path, const char *file)
{
	struct dstr output = {0};

	if (!file)
		file = "";

	dstr_copy(&output, data_path);
	if (!dstr_is_empty(&output) && dstr_end(&output) != '/' && *file)
		dstr_cat_ch(&output, '/');
	dstr_cat(&output, file);

	if (!os_file_exists(output.array))
		dstr_free(&output);
	return output.array;
}
