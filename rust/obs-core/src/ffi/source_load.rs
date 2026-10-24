//! C ABI for `libobs/obs-source-load.c.h`, the source load migration and
//! value rules of `obs_load_source_type()` (`obs.c`), declared in
//! `libobs/obs-source-load.h` without `EXPORT`.
//!
//! These only compute values. `obs.c` (still C) reads each saved value with
//! `obs_data_*()`, passes it here and hands the answer to the setter, in the
//! same order and the same number of calls as before; it also creates the
//! source, looks up the canvas and loads the filters. The monitoring plan
//! crosses as a `#[repr(C)]` struct whose layout Tier 3 checks against the
//! C. No pointer comes here. All stay hidden (`rust-hidden.txt`).
//!
//! No intentional differences.

use core::ffi::c_int;

use crate::source_load::{self, MonitoringPlan};

/// `struct obs_source_load_monitoring` (`obs-source-load.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(non_camel_case_types)]
pub struct obs_source_load_monitoring {
    pub set_mixers: bool,
    pub mixers: u32,
    pub store_enabled: bool,
    pub enabled: bool,
}

impl From<MonitoringPlan> for obs_source_load_monitoring {
    fn from(plan: MonitoringPlan) -> Self {
        Self {
            set_mixers: plan.set_mixers,
            mixers: plan.mixers,
            store_enabled: plan.store_enabled,
            enabled: plan.enabled,
        }
    }
}

/// `uint32_t obs_source_load_prev_ver(int64_t stored)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_load_prev_ver(stored: i64) -> u32 {
    source_load::prev_ver(stored)
}

/// `float obs_source_load_volume(double volume)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_load_volume(volume: f64) -> f32 {
    source_load::volume(volume)
}

/// `float obs_source_load_balance(double balance)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_load_balance(balance: f64) -> f32 {
    source_load::balance(balance)
}

/// `uint64_t obs_source_load_delay(int64_t delay)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_load_delay(delay: i64) -> u64 {
    source_load::delay(delay)
}

/// `int obs_source_load_deinterlace_mode(int64_t mode)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_load_deinterlace_mode(mode: i64) -> c_int {
    source_load::deinterlace_mode(mode)
}

/// `struct obs_source_load_monitoring obs_source_load_monitoring_migration(
/// uint32_t prev_ver, uint32_t caps, int64_t monitoring_type)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_load_monitoring_migration(
    prev_ver: u32,
    caps: u32,
    monitoring_type: i64,
) -> obs_source_load_monitoring {
    source_load::monitoring_migration(prev_ver, caps, monitoring_type).into()
}
