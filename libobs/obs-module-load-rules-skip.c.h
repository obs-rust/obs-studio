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

/* The skip ladder of load_all_callback() in obs-module.c: the checks it
 * makes before it opens a module, one read at a time. Pure: no libobs
 * state. obs-module.c keeps every read (get_plugin_info(), the safe and
 * disabled lists, the core module list, obs_get_module()), both
 * obs_create_disabled_module() calls and every log message, at the same
 * points and in the same order, and passes each value in. The type decides
 * which reads happen at all: the disabled list is read only for a type
 * that is not core, and the core module list only for a legacy plugin.
 * Declared in obs-module-load-rules.h, beside the other load rules of
 * obs-module-load-rules.c. */

enum obs_module_load_rules_skip obs_module_load_rules_skip_step(enum obs_module_load_rules_skip step, uint32_t type,
								bool value)
{
	switch (step) {
	case OBS_MODULE_LOAD_RULES_SKIP_START:
		return OBS_MODULE_LOAD_RULES_SKIP_PLUGIN;
	case OBS_MODULE_LOAD_RULES_SKIP_PLUGIN:
		return value ? OBS_MODULE_LOAD_RULES_SKIP_SAFE : OBS_MODULE_LOAD_RULES_SKIP_NOT_PLUGIN;
	case OBS_MODULE_LOAD_RULES_SKIP_SAFE:
		/* Not looking at the type: a core module left out of a safe
		 * list is disabled too. */
		if (!value)
			return OBS_MODULE_LOAD_RULES_SKIP_DISABLE_SAFE;
		if (type != OBS_MODULE_LOAD_RULES_TYPE_CORE)
			return OBS_MODULE_LOAD_RULES_SKIP_DISABLED;
		return OBS_MODULE_LOAD_RULES_SKIP_LOADED;
	case OBS_MODULE_LOAD_RULES_SKIP_DISABLED:
		if (value)
			return OBS_MODULE_LOAD_RULES_SKIP_DISABLE;
		/* This is a necessary check until legacy plugin loading is
		 * removed in a future update. It covers portable installs where
		 * the old core modules still live in `obs-plugins`. */
		if (type == OBS_MODULE_LOAD_RULES_TYPE_LEGACY_PLUGIN)
			return OBS_MODULE_LOAD_RULES_SKIP_CORE_NAME;
		return OBS_MODULE_LOAD_RULES_SKIP_LOADED;
	case OBS_MODULE_LOAD_RULES_SKIP_CORE_NAME:
		return value ? OBS_MODULE_LOAD_RULES_SKIP_LEGACY_CORE : OBS_MODULE_LOAD_RULES_SKIP_LOADED;
	case OBS_MODULE_LOAD_RULES_SKIP_LOADED:
		if (!value)
			return OBS_MODULE_LOAD_RULES_SKIP_LOAD;
		if (type == OBS_MODULE_LOAD_RULES_TYPE_CORE)
			return OBS_MODULE_LOAD_RULES_SKIP_LOADED_CORE;
		if (type == OBS_MODULE_LOAD_RULES_TYPE_LEGACY_PLUGIN)
			return OBS_MODULE_LOAD_RULES_SKIP_LOADED_LEGACY;
		return OBS_MODULE_LOAD_RULES_SKIP_LOADED_PLUGIN;
	case OBS_MODULE_LOAD_RULES_SKIP_NOT_PLUGIN:
	case OBS_MODULE_LOAD_RULES_SKIP_DISABLE_SAFE:
	case OBS_MODULE_LOAD_RULES_SKIP_DISABLE:
	case OBS_MODULE_LOAD_RULES_SKIP_LEGACY_CORE:
	case OBS_MODULE_LOAD_RULES_SKIP_LOADED_CORE:
	case OBS_MODULE_LOAD_RULES_SKIP_LOADED_LEGACY:
	case OBS_MODULE_LOAD_RULES_SKIP_LOADED_PLUGIN:
	case OBS_MODULE_LOAD_RULES_SKIP_LOAD:
		break;
	}
	return step;
}
