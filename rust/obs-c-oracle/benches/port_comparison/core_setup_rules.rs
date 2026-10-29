//! obs-core: the core setup rules (`libobs/obs-core-setup-rules.c.h`), as
//! the calls that ask them most make them: the audio buffering ticks of an
//! audio reset, the latest version compare of an input type lookup, and the
//! frame-count rule for an encoder-only mix of the main view, asked once
//! per mix each time the stats are read. Plain values: no allocation.

use criterion::Criterion;
use obs_c_oracle::core_setup_rules::{
    oracle_obs_core_setup_rules_buffer_ticks, oracle_obs_core_setup_rules_latest_newer,
    oracle_obs_core_setup_rules_mix_step,
};
use obs_core::ffi::core_setup_rules::{
    obs_core_setup_rules_buffer_ticks, obs_core_setup_rules_latest_newer,
    obs_core_setup_rules_mix_step,
};

/// `OBS_CORE_SETUP_RULES_MIX_MAIN`.
const MIX_MAIN: u32 = 0;

pub fn benchmarks(criterion: &mut Criterion) {
    super::pair(
        criterion,
        "core_setup_rules/buffer_ticks",
        || obs_core_setup_rules_buffer_ticks(1000, 48000),
        // SAFETY: plain values.
        || unsafe { oracle_obs_core_setup_rules_buffer_ticks(1000, 48000) },
    );
    super::pair(
        criterion,
        "core_setup_rules/latest_newer",
        || obs_core_setup_rules_latest_newer(2, 1),
        // SAFETY: plain values.
        || unsafe { oracle_obs_core_setup_rules_latest_newer(2, 1) },
    );
    super::pair(
        criterion,
        "core_setup_rules/mix_step",
        || {
            let mut step = obs_core_setup_rules_mix_step(MIX_MAIN, false);
            step = obs_core_setup_rules_mix_step(step, true);
            step = obs_core_setup_rules_mix_step(step, true);
            obs_core_setup_rules_mix_step(step, true)
        },
        // SAFETY: plain values.
        || unsafe {
            let mut step = oracle_obs_core_setup_rules_mix_step(MIX_MAIN, false);
            step = oracle_obs_core_setup_rules_mix_step(step, true);
            step = oracle_obs_core_setup_rules_mix_step(step, true);
            oracle_obs_core_setup_rules_mix_step(step, true)
        },
    );
}
