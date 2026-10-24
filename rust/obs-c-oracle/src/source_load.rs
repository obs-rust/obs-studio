//! `oracle_*` build of `libobs/obs-source-load.c.h` (oracle/source_load.c):
//! the source load migration and value rules of `obs_load_source_type()`,
//! renamed.
//!
//! The monitoring plan comes back as [`OracleMonitoring`];
//! [`oracle_source_load_layout`] says how the C lays it out.

/// Index of each entry in `oracle_source_load_constant`, in the C's order.
pub const CONSTANT_NAMES: [&str; 7] = [
    "OBS_SOURCE_LOAD_MONITOR_ONLY_BEFORE",
    "OBS_SOURCE_LOAD_MONITORING_ENABLED_BEFORE",
    "OBS_SOURCE_LOAD_MONITOR_BY_DEFAULT",
    "OBS_SOURCE_LOAD_MONITORING_NONE",
    "OBS_SOURCE_LOAD_MONITORING_MONITOR_ONLY",
    "OBS_SOURCE_LOAD_ALL_MIXERS",
    "OBS_SOURCE_LOAD_DEINTERLACE_DISABLE",
];

/// Index of each entry in `oracle_source_load_layout`, in the C's order.
pub const LAYOUT_NAMES: [&str; 6] = [
    "sizeof(struct obs_source_load_monitoring)",
    "alignof(struct obs_source_load_monitoring)",
    "offsetof(struct obs_source_load_monitoring, set_mixers)",
    "offsetof(struct obs_source_load_monitoring, mixers)",
    "offsetof(struct obs_source_load_monitoring, store_enabled)",
    "offsetof(struct obs_source_load_monitoring, enabled)",
];

/// `struct obs_source_load_monitoring`, as the C returns it.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OracleMonitoring {
    pub set_mixers: bool,
    pub mixers: u32,
    pub store_enabled: bool,
    pub enabled: bool,
}

unsafe extern "C" {
    pub fn oracle_obs_source_load_prev_ver(stored: i64) -> u32;

    pub fn oracle_obs_source_load_volume(volume: f64) -> f32;

    pub fn oracle_obs_source_load_balance(balance: f64) -> f32;

    pub fn oracle_obs_source_load_delay(delay: i64) -> u64;

    pub fn oracle_obs_source_load_deinterlace_mode(mode: i64) -> core::ffi::c_int;

    pub fn oracle_obs_source_load_monitoring_migration(
        prev_ver: u32,
        caps: u32,
        monitoring_type: i64,
    ) -> OracleMonitoring;

    /// The entry named `CONSTANT_NAMES[which]`; -1 out of range.
    pub fn oracle_source_load_constant(which: usize) -> i64;

    /// The entry named `LAYOUT_NAMES[which]`; `usize::MAX` out of range.
    pub fn oracle_source_load_layout(which: usize) -> usize;
}
