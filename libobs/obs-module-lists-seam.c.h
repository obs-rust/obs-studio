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

#include "libobs/build/libobs.pch.h"

/* Private adapter, not a new public API (obs-module-lists-seam.h). Each body
 * is copied from the line of obs-module-lists.c named beside it. */

/* ------------------------------------------------------------------------- */
/* The core */

/* obs_add_module_path(), obs_add_safe_module(), obs_add_disabled_module():
 * !obs */
bool obs_module_lists_seam_has_core(void)
{
	return false;
}

/* obs_log_loaded_modules(): obs->first_module */
obs_module_t *obs_module_lists_seam_first_module(void)
{
	return NULL;
}

/* obs_module_get_config_path(): obs->module_config_path */
const char *obs_module_lists_seam_module_config_path(void)
{
	return NULL;
}

/* obs_add_module_path(): da_push_back(obs->module_paths, ..) */
struct darray *obs_module_lists_seam_module_paths(void)
{
	return NULL;
}

/* obs_add_safe_module(): da_push_back(obs->safe_modules, ..) */
struct darray *obs_module_lists_seam_safe_modules(void)
{
	return NULL;
}

/* obs_add_disabled_module(): da_push_back(obs->disabled_modules, ..) */
struct darray *obs_module_lists_seam_disabled_modules(void)
{
	return NULL;
}

/* ------------------------------------------------------------------------- */
/* A module */

/* obs_log_loaded_modules(): mod->next */
obs_module_t *obs_module_lists_seam_module_next(const obs_module_t *module)
{
	UNUSED_PARAMETER(module);
	return NULL;
}

/* obs_log_loaded_modules(): mod->file */
const char *obs_module_lists_seam_module_file(const obs_module_t *module)
{
	UNUSED_PARAMETER(module);
	return NULL;
}

/* obs_find_module_file(): module->data_path */
const char *obs_module_lists_seam_module_data_path(const obs_module_t *module)
{
	UNUSED_PARAMETER(module);
	return NULL;
}

/* obs_module_get_config_path(): module->mod_name */
const char *obs_module_lists_seam_module_name(const obs_module_t *module)
{
	UNUSED_PARAMETER(module);
	return NULL;
}

/* obs_module_add_source(): da_push_back(module->sources, ..) */
struct darray *obs_module_lists_seam_module_sources(obs_module_t *module)
{
	UNUSED_PARAMETER(module);
	return NULL;
}

/* obs_module_add_output(): da_push_back(module->outputs, ..) */
struct darray *obs_module_lists_seam_module_outputs(obs_module_t *module)
{
	UNUSED_PARAMETER(module);
	return NULL;
}

/* obs_module_add_encoder(): da_push_back(module->encoders, ..) */
struct darray *obs_module_lists_seam_module_encoders(obs_module_t *module)
{
	UNUSED_PARAMETER(module);
	return NULL;
}

/* obs_module_add_service(): da_push_back(module->services, ..) */
struct darray *obs_module_lists_seam_module_services(obs_module_t *module)
{
	UNUSED_PARAMETER(module);
	return NULL;
}
