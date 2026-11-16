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

#include "obs-internal.h"

/* Private adapter, not a new public API: the seam between the module list
 * calls (obs-module-lists.c) and the fields they read in struct obs_core and
 * struct obs_module (obs-module-lists-seam.c).
 *
 * - The core: whether obs is set; first_module; module_config_path; and the
 *   struct darray inside module_paths (of struct obs_module_path),
 *   safe_modules and disabled_modules (of char *).
 * - A module: next, file, data_path and mod_name; and the struct darray
 *   inside sources, outputs, encoders and services (of char *).
 *
 * Each is a single read or field address, copied from the line of
 * obs-module-lists.c it wraps. A darray address is the one da_push_back()
 * takes there: push onto it with darray_push_back() and the element size of
 * the typed list. None checks its arguments or takes a lock: the module list
 * calls take none today, and a caller keeps it that way. The core reads
 * other than obs_module_lists_seam_has_core() need obs set.
 *
 * Hidden in libobs: not EXPORT. */

/* The core. */
bool obs_module_lists_seam_has_core(void);
obs_module_t *obs_module_lists_seam_first_module(void);
const char *obs_module_lists_seam_module_config_path(void);
struct darray *obs_module_lists_seam_module_paths(void);
struct darray *obs_module_lists_seam_safe_modules(void);
struct darray *obs_module_lists_seam_disabled_modules(void);

/* A module. */
obs_module_t *obs_module_lists_seam_module_next(const obs_module_t *module);
const char *obs_module_lists_seam_module_file(const obs_module_t *module);
const char *obs_module_lists_seam_module_data_path(const obs_module_t *module);
const char *obs_module_lists_seam_module_name(const obs_module_t *module);
struct darray *obs_module_lists_seam_module_sources(obs_module_t *module);
struct darray *obs_module_lists_seam_module_outputs(obs_module_t *module);
struct darray *obs_module_lists_seam_module_encoders(obs_module_t *module);
struct darray *obs_module_lists_seam_module_services(obs_module_t *module);
