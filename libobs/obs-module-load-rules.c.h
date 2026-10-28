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

#include "libobs/build/libobs-ported-module-load-rules.pch.h"

/* Module load rules, moved from obs_open_module(), load_module_exports(),
 * is_safe_module(), is_disabled_module(), obs_get_module_allow_disable(),
 * load_all_callback(), obs_load_core_modules(), find_core_module(),
 * obs_is_core_module() and obs_is_legacy_module() in obs-module.c: the open
 * gates, the required exports, the version rule, the safe and disabled list
 * rules, the core and legacy types, and the routing of each open and init
 * result. Pure: no libobs state. obs-module.c keeps every lookup, call, list
 * and log message, in the same place and under the same condition, and
 * passes what it read in. Declared in obs-module-load-rules.h. Ported to
 * rust/obs-core (module_load_rules): the hybrid libobs drops this file. */

bool obs_module_load_rules_arg_missing(bool present)
{
	return !present;
}

bool obs_module_load_rules_export_missing(bool found)
{
	return !found;
}

uint32_t obs_module_load_rules_version_word(uint32_t ver)
{
	/* Patch version (lower 16-bit) is ignored. */
	return ver & 0xFFFF0000;
}

bool obs_module_load_rules_version_ok(uint32_t ver_word, uint32_t api_ver)
{
	/* Reject plugins compiled with a newer libobs. */
	return !(ver_word > api_ver);
}

enum obs_module_load_rules_list obs_module_load_rules_safe_list(size_t num)
{
	if (!num)
		return OBS_MODULE_LOAD_RULES_LIST_ALL;

	return OBS_MODULE_LOAD_RULES_LIST_MATCH;
}

enum obs_module_load_rules_list obs_module_load_rules_disabled_list(size_t num)
{
	if (num == 0)
		return OBS_MODULE_LOAD_RULES_LIST_NONE;

	return OBS_MODULE_LOAD_RULES_LIST_MATCH;
}

bool obs_module_load_rules_allow_disable(bool core_name)
{
	return !core_name;
}

bool obs_module_load_rules_is_core(uint32_t type)
{
	return type == OBS_MODULE_LOAD_RULES_TYPE_CORE;
}

bool obs_module_load_rules_is_legacy(uint32_t type)
{
	return type == OBS_MODULE_LOAD_RULES_TYPE_LEGACY_PLUGIN;
}

uint32_t obs_module_load_rules_open_route(int code, uint32_t *state)
{
	*state = OBS_MODULE_LOAD_RULES_STATE_INVALID;

	switch (code) {
	case OBS_MODULE_LOAD_RULES_MISSING_EXPORTS:
		return OBS_MODULE_LOAD_RULES_STOP;
	case OBS_MODULE_LOAD_RULES_FAILED_TO_OPEN:
		*state = OBS_MODULE_LOAD_RULES_STATE_FAILED_TO_OPEN;
		/* Most often a library the module needs is not installed
		 * (libcef.so for obs-browser). A core module carries on without
		 * it, as a plugin does, instead of failing the core module load
		 * and with it OBS startup. */
		return OBS_MODULE_LOAD_RULES_DISABLE | OBS_MODULE_LOAD_RULES_CORE_CONTINUES |
		       OBS_MODULE_LOAD_RULES_FAIL | OBS_MODULE_LOAD_RULES_STOP;
	case OBS_MODULE_LOAD_RULES_ERROR:
		return OBS_MODULE_LOAD_RULES_FAIL | OBS_MODULE_LOAD_RULES_STOP;
	case OBS_MODULE_LOAD_RULES_INCOMPATIBLE_VER:
		*state = OBS_MODULE_LOAD_RULES_STATE_FAILED_TO_OPEN;
		return OBS_MODULE_LOAD_RULES_DISABLE | OBS_MODULE_LOAD_RULES_FAIL | OBS_MODULE_LOAD_RULES_STOP;
	case OBS_MODULE_LOAD_RULES_HARDCODED_SKIP:
		return OBS_MODULE_LOAD_RULES_STOP;
	}

	/* Any other code (MODULE_SUCCESS, or one obs_open_module() never
	 * returns) goes on to obs_init_module(). */
	return 0;
}

uint32_t obs_module_load_rules_init_route(bool ok, uint32_t *state)
{
	if (!ok) {
		*state = OBS_MODULE_LOAD_RULES_STATE_FAILED_TO_INITIALIZE;
		return OBS_MODULE_LOAD_RULES_FREE | OBS_MODULE_LOAD_RULES_DISABLE | OBS_MODULE_LOAD_RULES_KEEP_TYPE;
	}

	*state = OBS_MODULE_LOAD_RULES_STATE_INVALID;
	return OBS_MODULE_LOAD_RULES_SET_TYPE;
}

bool obs_module_load_rules_count_failure(bool has_fail_info)
{
	return has_fail_info;
}

bool obs_module_load_rules_core_loaded(size_t fail_count)
{
	return fail_count == 0;
}
