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

#include "libobs/build/libobs-ported-windows-symbol-paths.pch.h"

/* The string rules of the debug symbol search path, moved from
 * obs-windows.c's reset_win32_symbol_paths(): the directory of a module's
 * binary, the duplicate check and the join. obs-windows.c keeps the DbgHelp
 * calls, os_get_abs_path_ptr(), the list and the frees. Declared in
 * obs-windows-symbol-paths.h without EXPORT. Ported to rust/obs-core
 * (win_symbol_paths): the hybrid libobs drops this file. */

bool obs_windows_symbol_paths_dir(const char *bin_path, struct dstr *dir)
{
	char *path_end;

	dstr_copy(dir, bin_path);
	dstr_replace(dir, "/", "\\");

	path_end = strrchr(dir->array, '\\');
	if (!path_end)
		return false;

	*path_end = 0;
	return true;
}

bool obs_windows_symbol_paths_listed(char *const *paths, size_t num, const char *abspath)
{
	for (size_t i = 0; i < num; i++) {
		const char *existing_path = paths[i];
		if (astrcmpi(abspath, existing_path) == 0)
			return true;
	}

	return false;
}

void obs_windows_symbol_paths_join(char *const *paths, size_t num, struct dstr *out)
{
	for (size_t i = 0; i < num; i++) {
		const char *path = paths[i];
		if (path && *path) {
			if (i != 0)
				dstr_cat(out, ";");
			dstr_cat(out, paths[i]);
		}
	}
}
