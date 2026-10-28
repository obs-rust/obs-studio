/* Test-only oracle: the original libobs/obs-module-load-rules.c.h,
 * unmodified, with its thirteen entry points renamed to oracle_* so they can
 * link next to the Rust shims. Also the constants of obs-module-load-rules.h,
 * obs-defs.h and obs.h, as the C sees them. */

#include <stddef.h>
#include <stdint.h>

#include "obs.h"

#define obs_module_load_rules_arg_missing oracle_obs_module_load_rules_arg_missing
#define obs_module_load_rules_export_missing oracle_obs_module_load_rules_export_missing
#define obs_module_load_rules_version_word oracle_obs_module_load_rules_version_word
#define obs_module_load_rules_version_ok oracle_obs_module_load_rules_version_ok
#define obs_module_load_rules_safe_list oracle_obs_module_load_rules_safe_list
#define obs_module_load_rules_disabled_list oracle_obs_module_load_rules_disabled_list
#define obs_module_load_rules_allow_disable oracle_obs_module_load_rules_allow_disable
#define obs_module_load_rules_is_core oracle_obs_module_load_rules_is_core
#define obs_module_load_rules_is_legacy oracle_obs_module_load_rules_is_legacy
#define obs_module_load_rules_open_route oracle_obs_module_load_rules_open_route
#define obs_module_load_rules_init_route oracle_obs_module_load_rules_init_route
#define obs_module_load_rules_count_failure oracle_obs_module_load_rules_count_failure
#define obs_module_load_rules_core_loaded oracle_obs_module_load_rules_core_loaded

#include "obs-module-load-rules.c.h"

#undef obs_module_load_rules_core_loaded
#undef obs_module_load_rules_count_failure
#undef obs_module_load_rules_init_route
#undef obs_module_load_rules_open_route
#undef obs_module_load_rules_is_legacy
#undef obs_module_load_rules_is_core
#undef obs_module_load_rules_allow_disable
#undef obs_module_load_rules_disabled_list
#undef obs_module_load_rules_safe_list
#undef obs_module_load_rules_version_ok
#undef obs_module_load_rules_version_word
#undef obs_module_load_rules_export_missing
#undef obs_module_load_rules_arg_missing

size_t oracle_module_load_rules_constant_count(void);
int64_t oracle_module_load_rules_constant(size_t i);

/* In the order of CONSTANT_NAMES (src/module_load_rules.rs). */
static const int64_t constants[] = {
	OBS_MODULE_LOAD_RULES_SUCCESS,
	OBS_MODULE_LOAD_RULES_ERROR,
	OBS_MODULE_LOAD_RULES_FAILED_TO_OPEN,
	OBS_MODULE_LOAD_RULES_MISSING_EXPORTS,
	OBS_MODULE_LOAD_RULES_INCOMPATIBLE_VER,
	OBS_MODULE_LOAD_RULES_HARDCODED_SKIP,
	MODULE_SUCCESS,
	MODULE_ERROR,
	MODULE_FAILED_TO_OPEN,
	MODULE_MISSING_EXPORTS,
	MODULE_INCOMPATIBLE_VER,
	MODULE_HARDCODED_SKIP,
	OBS_MODULE_LOAD_RULES_TYPE_CORE,
	OBS_MODULE_LOAD_RULES_TYPE_LEGACY_PLUGIN,
	OBS_MODULE_TYPE_UNDEFINED,
	OBS_MODULE_TYPE_CORE,
	OBS_MODULE_TYPE_PLUGIN,
	OBS_MODULE_TYPE_LEGACY_PLUGIN,
	OBS_MODULE_LOAD_RULES_STATE_INVALID,
	OBS_MODULE_LOAD_RULES_STATE_FAILED_TO_OPEN,
	OBS_MODULE_LOAD_RULES_STATE_FAILED_TO_INITIALIZE,
	OBS_MODULE_INVALID,
	OBS_MODULE_DISABLED,
	OBS_MODULE_DISABLED_SAFE,
	OBS_MODULE_FAILED_TO_OPEN,
	OBS_MODULE_FAILED_TO_INITIALIZE,
	OBS_MODULE_LOAD_RULES_LIST_NONE,
	OBS_MODULE_LOAD_RULES_LIST_ALL,
	OBS_MODULE_LOAD_RULES_LIST_MATCH,
	OBS_MODULE_LOAD_RULES_DISABLE,
	OBS_MODULE_LOAD_RULES_KEEP_TYPE,
	OBS_MODULE_LOAD_RULES_SET_TYPE,
	OBS_MODULE_LOAD_RULES_FREE,
	OBS_MODULE_LOAD_RULES_FAIL,
	OBS_MODULE_LOAD_RULES_STOP,
	OBS_MODULE_LOAD_RULES_CORE_CONTINUES,
	LIBOBS_API_VER,
	sizeof(enum obs_module_load_rules_list),
};

size_t oracle_module_load_rules_constant_count(void)
{
	return sizeof(constants) / sizeof(constants[0]);
}

int64_t oracle_module_load_rules_constant(size_t i)
{
	return i < sizeof(constants) / sizeof(constants[0]) ? constants[i] : -1;
}
