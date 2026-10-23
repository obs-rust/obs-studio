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

#include "libobs/build/libobs-ported-module-names.pch.h"

/* Module names and files, moved from obs-module.c: how a module is named
 * from its binary, where its config file and its manifest are, whether a
 * name is on a module list, and how a core module is found. Pure: no libobs
 * state. obs-module.c keeps the module lists (obs->safe_modules,
 * obs->disabled_modules, the module list), struct obs_module, the config
 * path (obs->module_config_path) and the loading, and passes in what these
 * need: the paths, the names, the list and the module file extension
 * (get_module_extension()). Declared in obs-internal.h. Ported to
 * rust/obs-core (module_names): the hybrid libobs drops this file. */

const char *obs_module_names_file_name(const char *path)
{
	const char *file = strrchr(path, '/');
	return (!file) ? path : (file + 1);
}

char *obs_module_names_module_name(const char *file, const char *ext)
{
	struct dstr name = {0};

	dstr_copy(&name, file);
	dstr_resize(&name, name.len - strlen(ext));
	return name.array;
}

char *obs_module_names_config_path(const char *config_path, const char *module_name, const char *file)
{
	struct dstr output = {0};

	dstr_copy(&output, config_path);
	if (!dstr_is_empty(&output) && dstr_end(&output) != '/')
		dstr_cat_ch(&output, '/');
	dstr_cat(&output, module_name);
	dstr_cat_ch(&output, '/');
	dstr_cat(&output, file);

	return output.array;
}

char *obs_module_names_manifest_path(const char *data_path)
{
	struct dstr path = {0};

	dstr_copy(&path, data_path);
	if (!dstr_is_empty(&path) && dstr_end(&path) != '/') {
		dstr_cat_ch(&path, '/');
	}
	dstr_cat(&path, "manifest.json");

	return path.array;
}

bool obs_module_names_listed(char *const *names, size_t count, const char *name)
{
	for (size_t i = 0; i < count; i++) {
		if (strcmp(name, names[i]) == 0)
			return true;
	}

	return false;
}

bool obs_module_names_find_core(const char *binary, const char *name, const char *data,
				enum obs_runtime_module_type type, const char *ext,
				obs_find_module_callback2_t callback, void *param)
{
	bool found = false;

	struct dstr module_path;
	dstr_init_copy(&module_path, binary);
	dstr_cat(&module_path, ext);

	char *parsed_data_directory;
	parsed_data_directory = obs_module_paths_data_dir(name, data);

	if (os_file_exists(module_path.array)) {
		struct obs_module_info2 callback_info = {.bin_path = module_path.array,
							 .data_path = parsed_data_directory,
							 .name = name,
							 .type = type};

		callback(param, &callback_info);
		found = true;
	}

	bfree(parsed_data_directory);
	dstr_free(&module_path);

	return found;
}
