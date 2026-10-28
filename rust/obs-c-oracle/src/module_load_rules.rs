//! `oracle_*` build of `libobs/obs-module-load-rules.c.h`
//! (oracle/module_load_rules.c).
//!
//! The list enum is a 32-bit integer, returned as `u32` as the shims do.
//! The constants of `obs-module-load-rules.h`, `obs-defs.h` and `obs.h`, and
//! `LIBOBS_API_VER`, come from the same C file.

use core::ffi::c_int;

/// Index of each entry in `oracle_module_load_rules_constant`, in the C's
/// order.
pub const CONSTANT_NAMES: [&str; 38] = [
    "OBS_MODULE_LOAD_RULES_SUCCESS",
    "OBS_MODULE_LOAD_RULES_ERROR",
    "OBS_MODULE_LOAD_RULES_FAILED_TO_OPEN",
    "OBS_MODULE_LOAD_RULES_MISSING_EXPORTS",
    "OBS_MODULE_LOAD_RULES_INCOMPATIBLE_VER",
    "OBS_MODULE_LOAD_RULES_HARDCODED_SKIP",
    "MODULE_SUCCESS",
    "MODULE_ERROR",
    "MODULE_FAILED_TO_OPEN",
    "MODULE_MISSING_EXPORTS",
    "MODULE_INCOMPATIBLE_VER",
    "MODULE_HARDCODED_SKIP",
    "OBS_MODULE_LOAD_RULES_TYPE_CORE",
    "OBS_MODULE_LOAD_RULES_TYPE_LEGACY_PLUGIN",
    "OBS_MODULE_TYPE_UNDEFINED",
    "OBS_MODULE_TYPE_CORE",
    "OBS_MODULE_TYPE_PLUGIN",
    "OBS_MODULE_TYPE_LEGACY_PLUGIN",
    "OBS_MODULE_LOAD_RULES_STATE_INVALID",
    "OBS_MODULE_LOAD_RULES_STATE_FAILED_TO_OPEN",
    "OBS_MODULE_LOAD_RULES_STATE_FAILED_TO_INITIALIZE",
    "OBS_MODULE_INVALID",
    "OBS_MODULE_DISABLED",
    "OBS_MODULE_DISABLED_SAFE",
    "OBS_MODULE_FAILED_TO_OPEN",
    "OBS_MODULE_FAILED_TO_INITIALIZE",
    "OBS_MODULE_LOAD_RULES_LIST_NONE",
    "OBS_MODULE_LOAD_RULES_LIST_ALL",
    "OBS_MODULE_LOAD_RULES_LIST_MATCH",
    "OBS_MODULE_LOAD_RULES_DISABLE",
    "OBS_MODULE_LOAD_RULES_KEEP_TYPE",
    "OBS_MODULE_LOAD_RULES_SET_TYPE",
    "OBS_MODULE_LOAD_RULES_FREE",
    "OBS_MODULE_LOAD_RULES_FAIL",
    "OBS_MODULE_LOAD_RULES_STOP",
    "OBS_MODULE_LOAD_RULES_CORE_CONTINUES",
    "LIBOBS_API_VER",
    "sizeof(enum obs_module_load_rules_list)",
];

unsafe extern "C" {
    pub fn oracle_obs_module_load_rules_arg_missing(present: bool) -> bool;
    pub fn oracle_obs_module_load_rules_export_missing(found: bool) -> bool;
    pub fn oracle_obs_module_load_rules_version_word(ver: u32) -> u32;
    pub fn oracle_obs_module_load_rules_version_ok(ver_word: u32, api_ver: u32) -> bool;
    pub fn oracle_obs_module_load_rules_safe_list(num: usize) -> u32;
    pub fn oracle_obs_module_load_rules_disabled_list(num: usize) -> u32;
    pub fn oracle_obs_module_load_rules_allow_disable(core_name: bool) -> bool;
    pub fn oracle_obs_module_load_rules_is_core(module_type: u32) -> bool;
    pub fn oracle_obs_module_load_rules_is_legacy(module_type: u32) -> bool;
    pub fn oracle_obs_module_load_rules_open_route(code: c_int, state: *mut u32) -> u32;
    pub fn oracle_obs_module_load_rules_init_route(ok: bool, state: *mut u32) -> u32;
    pub fn oracle_obs_module_load_rules_count_failure(has_fail_info: bool) -> bool;
    pub fn oracle_obs_module_load_rules_core_loaded(fail_count: usize) -> bool;
    pub fn oracle_module_load_rules_constant_count() -> usize;
    pub fn oracle_module_load_rules_constant(i: usize) -> i64;
}
