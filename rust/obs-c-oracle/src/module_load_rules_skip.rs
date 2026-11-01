//! `oracle_*` build of `libobs/obs-module-load-rules-skip.c.h`
//! (oracle/module_load_rules_skip.c).
//!
//! `enum obs_module_load_rules_skip` is a 32-bit integer, passed as `u32` as
//! the shim does. Its values come from the same C file.

/// Index of each entry in `oracle_module_load_rules_skip_constant`, in the
/// C's order.
pub const SKIP_CONSTANT_NAMES: [&str; 15] = [
    "OBS_MODULE_LOAD_RULES_SKIP_START",
    "OBS_MODULE_LOAD_RULES_SKIP_PLUGIN",
    "OBS_MODULE_LOAD_RULES_SKIP_SAFE",
    "OBS_MODULE_LOAD_RULES_SKIP_DISABLED",
    "OBS_MODULE_LOAD_RULES_SKIP_CORE_NAME",
    "OBS_MODULE_LOAD_RULES_SKIP_LOADED",
    "OBS_MODULE_LOAD_RULES_SKIP_NOT_PLUGIN",
    "OBS_MODULE_LOAD_RULES_SKIP_DISABLE_SAFE",
    "OBS_MODULE_LOAD_RULES_SKIP_DISABLE",
    "OBS_MODULE_LOAD_RULES_SKIP_LEGACY_CORE",
    "OBS_MODULE_LOAD_RULES_SKIP_LOADED_CORE",
    "OBS_MODULE_LOAD_RULES_SKIP_LOADED_LEGACY",
    "OBS_MODULE_LOAD_RULES_SKIP_LOADED_PLUGIN",
    "OBS_MODULE_LOAD_RULES_SKIP_LOAD",
    "sizeof(enum obs_module_load_rules_skip)",
];

unsafe extern "C" {
    pub fn oracle_obs_module_load_rules_skip_step(step: u32, module_type: u32, value: bool) -> u32;
    pub fn oracle_module_load_rules_skip_constant_count() -> usize;
    pub fn oracle_module_load_rules_skip_constant(i: usize) -> i64;
}
