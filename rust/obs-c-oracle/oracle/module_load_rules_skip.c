/* Test-only oracle: the original libobs/obs-module-load-rules-skip.c.h,
 * unmodified, with its entry point renamed to oracle_* so it can link next
 * to the Rust shim. Also the values of enum obs_module_load_rules_skip, as
 * the C sees them. */

#include <stddef.h>
#include <stdint.h>

#define obs_module_load_rules_skip_step oracle_obs_module_load_rules_skip_step

#include "obs-module-load-rules-skip.c.h"

#undef obs_module_load_rules_skip_step

size_t oracle_module_load_rules_skip_constant_count(void);
int64_t oracle_module_load_rules_skip_constant(size_t i);

/* In the order of SKIP_CONSTANT_NAMES (src/module_load_rules_skip.rs). */
static const int64_t constants[] = {
	OBS_MODULE_LOAD_RULES_SKIP_START,         OBS_MODULE_LOAD_RULES_SKIP_PLUGIN,
	OBS_MODULE_LOAD_RULES_SKIP_SAFE,          OBS_MODULE_LOAD_RULES_SKIP_DISABLED,
	OBS_MODULE_LOAD_RULES_SKIP_CORE_NAME,     OBS_MODULE_LOAD_RULES_SKIP_LOADED,
	OBS_MODULE_LOAD_RULES_SKIP_NOT_PLUGIN,    OBS_MODULE_LOAD_RULES_SKIP_DISABLE_SAFE,
	OBS_MODULE_LOAD_RULES_SKIP_DISABLE,       OBS_MODULE_LOAD_RULES_SKIP_LEGACY_CORE,
	OBS_MODULE_LOAD_RULES_SKIP_LOADED_CORE,   OBS_MODULE_LOAD_RULES_SKIP_LOADED_LEGACY,
	OBS_MODULE_LOAD_RULES_SKIP_LOADED_PLUGIN, OBS_MODULE_LOAD_RULES_SKIP_LOAD,
	sizeof(enum obs_module_load_rules_skip),
};

size_t oracle_module_load_rules_skip_constant_count(void)
{
	return sizeof(constants) / sizeof(constants[0]);
}

int64_t oracle_module_load_rules_skip_constant(size_t i)
{
	return i < sizeof(constants) / sizeof(constants[0]) ? constants[i] : INT64_MIN;
}
