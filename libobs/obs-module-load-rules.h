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
#include <stdint.h>

/* The module load rules of obs-module.c, on plain values
 * (obs-module-load-rules.c). Each one is made where the C made it, from
 * values the caller reads at that point, so nothing is looked up or called
 * that was not before:
 * - obs_module_load_rules_arg_missing(): an argument obs_open_module()
 *   needs (the module pointer, the path, the core) is missing; asked for
 *   each in turn, as the C's || chain reads them;
 * - obs_module_load_rules_export_missing(): a required export (load,
 *   set_pointer, ver, in that order) was not found, so the module has
 *   missing exports; asked after each lookup;
 * - obs_module_load_rules_version_word(): the version a module reports,
 *   without its patch number (the low 16 bits);
 * - obs_module_load_rules_version_ok(): that word is not above the libobs
 *   the module runs in;
 * - obs_module_load_rules_safe_list(), obs_module_load_rules_disabled_list():
 *   from the length of the safe and disabled lists, whether every module is
 *   on the list, none is, or the name decides (obs_module_names_listed());
 * - obs_module_load_rules_allow_disable(): a module can be disabled unless
 *   its name is a core module's;
 * - obs_module_load_rules_is_core(), obs_module_load_rules_is_legacy(): a
 *   module type is the core or the legacy plugin type. Core modules are never
 *   disabled, refuse the plugin loader's find_core_module(), skip quietly
 *   when already loaded and carry on when one fails to open; a legacy plugin
 *   with a core module's name is skipped, and an already loaded legacy
 *   plugin gets its own message;
 * - obs_module_load_rules_open_route(): what load_all_callback() does with
 *   the code obs_open_module() returned (the OBS_MODULE_LOAD_RULES_* bits
 *   below, and the state of the disabled module it makes). It does not take
 *   the module type: the C reads it only for CORE_CONTINUES, after making
 *   the disabled module, as before;
 * - obs_module_load_rules_init_route(): the same, for obs_init_module()'s
 *   result;
 * - obs_module_load_rules_count_failure(): a failure is counted only when the
 *   caller passed somewhere to count it;
 * - obs_module_load_rules_core_loaded(): the core modules loaded when no
 *   failure was counted.
 *
 * obs-module.c keeps get_plugin_info(), obs_get_module(),
 * obs_host_skip_module(), os_dlopen(), os_dlsym(), the ver() call, the module
 * set-up, obs_create_disabled_module(), obs_init_module(), free_module(), the
 * failure list, the module walks and every log message. Nothing here takes a
 * lock, calls into libobs or keeps state.
 *
 * The constants are copied below, so this header needs no core header;
 * obs-module.c checks them against obs-defs.h and obs.h.
 *
 * Hidden in libobs: not EXPORT. */

/* MODULE_SUCCESS, MODULE_ERROR, MODULE_FAILED_TO_OPEN,
 * MODULE_MISSING_EXPORTS, MODULE_INCOMPATIBLE_VER, MODULE_HARDCODED_SKIP */
#define OBS_MODULE_LOAD_RULES_SUCCESS 0
#define OBS_MODULE_LOAD_RULES_ERROR -1
#define OBS_MODULE_LOAD_RULES_FAILED_TO_OPEN -2
#define OBS_MODULE_LOAD_RULES_MISSING_EXPORTS -3
#define OBS_MODULE_LOAD_RULES_INCOMPATIBLE_VER -4
#define OBS_MODULE_LOAD_RULES_HARDCODED_SKIP -5
/* OBS_MODULE_TYPE_CORE, OBS_MODULE_TYPE_LEGACY_PLUGIN
 * (enum obs_runtime_module_type) */
#define OBS_MODULE_LOAD_RULES_TYPE_CORE 1
#define OBS_MODULE_LOAD_RULES_TYPE_LEGACY_PLUGIN 3
/* OBS_MODULE_INVALID, OBS_MODULE_FAILED_TO_OPEN,
 * OBS_MODULE_FAILED_TO_INITIALIZE (enum obs_module_load_state) */
#define OBS_MODULE_LOAD_RULES_STATE_INVALID 0
#define OBS_MODULE_LOAD_RULES_STATE_FAILED_TO_OPEN 5
#define OBS_MODULE_LOAD_RULES_STATE_FAILED_TO_INITIALIZE 6

/* What the safe or disabled list says about a module before its name is
 * looked at. */
enum obs_module_load_rules_list {
	OBS_MODULE_LOAD_RULES_LIST_NONE,
	OBS_MODULE_LOAD_RULES_LIST_ALL,
	OBS_MODULE_LOAD_RULES_LIST_MATCH,
};

/* The route bits:
 * - DISABLE: make a disabled module with the state given back;
 * - KEEP_TYPE: give that disabled module the module's type;
 * - CORE_CONTINUES: for a core module (obs_module_load_rules_is_core()),
 *   give that disabled module its type and stop instead of counting a
 *   failure;
 * - SET_TYPE: give the loaded module its type;
 * - FREE: free the module first (a failed obs_init_module());
 * - FAIL: count a failure;
 * - STOP: go no further (no obs_init_module()). */
#define OBS_MODULE_LOAD_RULES_DISABLE (1u << 0)
#define OBS_MODULE_LOAD_RULES_KEEP_TYPE (1u << 1)
#define OBS_MODULE_LOAD_RULES_SET_TYPE (1u << 2)
#define OBS_MODULE_LOAD_RULES_FREE (1u << 3)
#define OBS_MODULE_LOAD_RULES_FAIL (1u << 4)
#define OBS_MODULE_LOAD_RULES_STOP (1u << 5)
#define OBS_MODULE_LOAD_RULES_CORE_CONTINUES (1u << 6)

bool obs_module_load_rules_arg_missing(bool present);
bool obs_module_load_rules_export_missing(bool found);
uint32_t obs_module_load_rules_version_word(uint32_t ver);
bool obs_module_load_rules_version_ok(uint32_t ver_word, uint32_t api_ver);
enum obs_module_load_rules_list obs_module_load_rules_safe_list(size_t num);
enum obs_module_load_rules_list obs_module_load_rules_disabled_list(size_t num);
bool obs_module_load_rules_allow_disable(bool core_name);
bool obs_module_load_rules_is_core(uint32_t type);
bool obs_module_load_rules_is_legacy(uint32_t type);
uint32_t obs_module_load_rules_open_route(int code, uint32_t *state);
uint32_t obs_module_load_rules_init_route(bool ok, uint32_t *state);
bool obs_module_load_rules_count_failure(bool has_fail_info);
bool obs_module_load_rules_core_loaded(size_t fail_count);
