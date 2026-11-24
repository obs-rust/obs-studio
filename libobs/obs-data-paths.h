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

#include "util/darray.h"
#include "util/dstr.h"

/* The process-global list of extra data paths that obs_add_data_path() and
 * obs_remove_data_path() change and obs_find_data_file() searches (#5397).
 * It is defined once, in obs-data-paths-fields.c.h beside obs.c.h, so the
 * original wrappers in their reference-only group and obs_find_data_file()
 * share it. Private: declared without EXPORT. */
typedef DARRAY(struct dstr) obs_data_path_list_t;

obs_data_path_list_t *obs_data_paths_list(void);
