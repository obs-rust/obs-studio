//! `oracle_*` builds of `libobs/obs-source-transition-sample-time.c.h`
//! (oracle/source_transition_sample_time.c): the time of each audio sample
//! a transition mixes, renamed, once over the C clock and once over the
//! Rust clock. Everything crosses as plain values.

use core::ffi::c_int;

unsafe extern "C" {
    /// `obs_source_transition_sample_time`, asking the C clock
    /// (`oracle_obs_source_transition_clock_time`), as in
    /// libobs-c-reference. Divides by zero for a `sample_rate` of 0.
    pub fn oracle_obs_source_transition_sample_time(
        mode: c_int,
        manual_val: f32,
        start_time: u64,
        duration: u64,
        ts: u64,
        sample_rate: usize,
        sample: usize,
    ) -> f32;

    /// `obs_source_transition_sample_time`, asking the Rust clock shim
    /// (`obs_source_transition_clock_time`), as the hybrid libobs's clock.
    /// Divides by zero for a `sample_rate` of 0.
    pub fn oracle_obs_source_transition_sample_time_rust_clock(
        mode: c_int,
        manual_val: f32,
        start_time: u64,
        duration: u64,
        ts: u64,
        sample_rate: usize,
        sample: usize,
    ) -> f32;

    /// 1 where `util_mul_div64()` is MSVC's `_udiv128()` in the oracle's
    /// build, which faults on a quotient past 64 bits; 0 for clang-cl's cut
    /// `__int128` division and the split formula.
    pub fn oracle_source_transition_sample_time_mul_div_faults() -> usize;
}
