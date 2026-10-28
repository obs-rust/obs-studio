//! obs-core: the module load rules (`libobs/obs-module-load-rules.c.h`), as
//! `load_all_callback()` makes them for one plugin that loads: the safe and
//! disabled list answers, the core and legacy type checks, the version rule
//! of `obs_open_module()`, the open route for `MODULE_SUCCESS` and the init
//! route for a module that loaded. Each runs once per module found, at
//! startup and when plugins load.

use criterion::Criterion;
use obs_c_oracle::module_load_rules::{
    oracle_obs_module_load_rules_disabled_list, oracle_obs_module_load_rules_init_route,
    oracle_obs_module_load_rules_is_core, oracle_obs_module_load_rules_is_legacy,
    oracle_obs_module_load_rules_open_route, oracle_obs_module_load_rules_safe_list,
    oracle_obs_module_load_rules_version_ok, oracle_obs_module_load_rules_version_word,
};
use obs_core::ffi::module_load_rules::{
    obs_module_load_rules_disabled_list, obs_module_load_rules_init_route,
    obs_module_load_rules_is_core, obs_module_load_rules_is_legacy,
    obs_module_load_rules_open_route, obs_module_load_rules_safe_list,
    obs_module_load_rules_version_ok, obs_module_load_rules_version_word,
};
use obs_core::module_load_rules::{MODULE_SUCCESS, TYPE_PLUGIN};

/// `LIBOBS_API_VER` 33.0.0, and a plugin built for it at patch 2.
const API_VER: u32 = 33 << 24;
const PLUGIN_VER: u32 = API_VER | 2;

pub fn benchmarks(criterion: &mut Criterion) {
    super::pair(
        criterion,
        "module_load_rules/lists",
        || {
            (
                obs_module_load_rules_safe_list(0),
                obs_module_load_rules_disabled_list(3),
            )
        },
        // SAFETY: plain values.
        || unsafe {
            (
                oracle_obs_module_load_rules_safe_list(0),
                oracle_obs_module_load_rules_disabled_list(3),
            )
        },
    );
    super::pair(
        criterion,
        "module_load_rules/types",
        || {
            (
                obs_module_load_rules_is_core(TYPE_PLUGIN),
                obs_module_load_rules_is_legacy(TYPE_PLUGIN),
            )
        },
        // SAFETY: plain values.
        || unsafe {
            (
                oracle_obs_module_load_rules_is_core(TYPE_PLUGIN),
                oracle_obs_module_load_rules_is_legacy(TYPE_PLUGIN),
            )
        },
    );
    super::pair(
        criterion,
        "module_load_rules/version",
        || obs_module_load_rules_version_ok(obs_module_load_rules_version_word(PLUGIN_VER), API_VER),
        // SAFETY: plain values.
        || unsafe {
            oracle_obs_module_load_rules_version_ok(
                oracle_obs_module_load_rules_version_word(PLUGIN_VER),
                API_VER,
            )
        },
    );
    super::pair(
        criterion,
        "module_load_rules/routes",
        || {
            let mut state = 0;
            // SAFETY: a valid state pointer.
            unsafe {
                (
                    obs_module_load_rules_open_route(MODULE_SUCCESS, &raw mut state),
                    obs_module_load_rules_init_route(true, &raw mut state),
                    state,
                )
            }
        },
        || {
            let mut state = 0;
            // SAFETY: a valid state pointer.
            unsafe {
                (
                    oracle_obs_module_load_rules_open_route(MODULE_SUCCESS, &raw mut state),
                    oracle_obs_module_load_rules_init_route(true, &raw mut state),
                    state,
                )
            }
        },
    );
}
