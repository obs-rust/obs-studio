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

#pragma once

#include <stdbool.h>
#include <stddef.h>

#include "util/dstr.h"

/* The string rules of obs-windows.c's debug symbol search path
 * (reset_win32_symbol_paths(), obs-windows-symbol-paths.c):
 * - the directory of a module's binary: the path with every '/' turned into
 *   '\\', cut at its last '\\';
 * - whether a directory is listed already, by astrcmpi();
 * - the search path: the listed directories joined with ';'.
 *
 * obs-windows.c keeps the DbgHelp loading, os_get_abs_path_ptr() for "." and
 * for each directory, the push or free of each entry, the wide conversion,
 * SymInitializeW() and SymSetSearchPathW() and sym_initialize_called, each at
 * its point. The functions take no Windows type, so this header and its file
 * build on every OS.
 *
 * Not EXPORT: obs-windows.c calls them inside libobs. */

#ifdef __cplusplus
extern "C" {
#endif

/* Copies bin_path into dir with every '/' turned into '\\' and cuts it at the
 * last '\\' (writing a NUL there; dir->len is left as it was). Returns false,
 * leaving the copy in dir, when the path has no '\\': the module adds no
 * directory. The caller frees dir either way. */
bool obs_windows_symbol_paths_dir(const char *bin_path, struct dstr *dir);

/* Whether abspath is one of the num paths already, compared by astrcmpi(). */
bool obs_windows_symbol_paths_listed(char *const *paths, size_t num, const char *abspath);

/* Appends the num paths to out, skipping NULL and empty ones, each but the one
 * at index 0 after a ';'. */
void obs_windows_symbol_paths_join(char *const *paths, size_t num, struct dstr *out);

#ifdef __cplusplus
}
#endif
