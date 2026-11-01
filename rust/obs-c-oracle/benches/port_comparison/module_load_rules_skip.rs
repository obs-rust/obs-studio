//! obs-core: the module loader's skip ladder
//! (`libobs/obs-module-load-rules-skip.c.h`), as `load_all_callback()` walks
//! it for a plugin that passes every check: START, then the plugin check, the
//! safe list, the disabled list and the loaded check, to LOAD. It runs once
//! per module found, at startup and when plugins load.

use criterion::Criterion;
use obs_c_oracle::module_load_rules_skip::oracle_obs_module_load_rules_skip_step;
use obs_core::ffi::module_load_rules::obs_module_load_rules_skip_step;
use obs_core::module_load_rules::TYPE_PLUGIN;

/// The value passed with each step, in order: none for START, then a plugin
/// on the safe list, not disabled and not loaded yet.
const ANSWERS: [bool; 5] = [false, true, true, false, false];

pub fn benchmarks(criterion: &mut Criterion) {
    super::pair(
        criterion,
        "module_load_rules/skip",
        || {
            let mut step = 0;
            for value in ANSWERS {
                step = obs_module_load_rules_skip_step(step, TYPE_PLUGIN, value);
            }
            step
        },
        || {
            let mut step = 0;
            for value in ANSWERS {
                // SAFETY: plain values.
                step = unsafe { oracle_obs_module_load_rules_skip_step(step, TYPE_PLUGIN, value) };
            }
            step
        },
    );
}
