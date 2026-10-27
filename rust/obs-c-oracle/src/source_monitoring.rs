//! `oracle_*` build of `libobs/obs-source-monitoring.c.h`
//! (oracle/source_monitoring.c): the monitoring, monitor-only mute, balance,
//! mixer, flag, push-to and audio active rules of the sources, renamed. The
//! `bool` results are declared as `u8`, the byte the C returns, so a parity
//! test compares it without assuming it is 0 or 1; the monitor, route and
//! active outcomes as `u32` (the C's enums, an `int` or `unsigned int`; every
//! value fits both), the monitoring types as [`c_int`], the mixers a handler
//! leaves as `i64` and the balance as `f32`.

use std::os::raw::c_int;

/// Index of each entry in `oracle_source_monitoring_value`, in the C's
/// order.
pub const VALUE_NAMES: [&str; 20] = [
    "OBS_SOURCE_MONITORING_AUDIO",
    "OBS_SOURCE_MONITORING_TYPE_NONE",
    "OBS_SOURCE_MONITORING_TYPE_MONITOR_ONLY",
    "OBS_SOURCE_MONITORING_TYPE_MONITOR_AND_OUTPUT",
    "OBS_SOURCE_MONITORING_MONITOR_KEEP",
    "OBS_SOURCE_MONITORING_MONITOR_CREATE",
    "OBS_SOURCE_MONITORING_MONITOR_DESTROY",
    "OBS_SOURCE_MONITORING_ROUTE_OFF",
    "OBS_SOURCE_MONITORING_ROUTE_MONITOR_ONLY",
    "OBS_SOURCE_MONITORING_ROUTE_MONITOR_AND_OUTPUT",
    "OBS_SOURCE_MONITORING_ROUTE_ON",
    "OBS_SOURCE_MONITORING_ACTIVE_NONE",
    "OBS_SOURCE_MONITORING_ACTIVE_ACTIVATE",
    "OBS_SOURCE_MONITORING_ACTIVE_DEACTIVATE",
    "sizeof(enum obs_source_monitoring_monitor)",
    "sizeof(enum obs_source_monitoring_route)",
    "sizeof(enum obs_source_monitoring_active)",
    "sizeof(bool)",
    "sizeof(int)",
    "sizeof(float)",
];

unsafe extern "C" {
    /// `bool obs_source_monitoring_changes(uint32_t cur, uint32_t value)`.
    pub fn oracle_obs_source_monitoring_changes(cur: u32, value: u32) -> u8;
    /// `bool obs_source_monitoring_has_audio(uint32_t output_flags)`.
    pub fn oracle_obs_source_monitoring_has_audio(output_flags: u32) -> u8;
    /// `uint32_t obs_source_monitoring_mixers_store(int64_t mixers)`.
    pub fn oracle_obs_source_monitoring_mixers_store(mixers: i64) -> u32;
    /// `bool obs_source_monitoring_push_to_logs(uint32_t output_flags, bool
    /// changed)`.
    pub fn oracle_obs_source_monitoring_push_to_logs(output_flags: u32, changed: bool) -> u8;
    /// `int obs_source_monitoring_on_type(bool muted)`.
    pub fn oracle_obs_source_monitoring_on_type(muted: bool) -> c_int;
    /// `int obs_source_monitoring_stored_type(bool enabled)`.
    pub fn oracle_obs_source_monitoring_stored_type(enabled: bool) -> c_int;
    /// `enum obs_source_monitoring_monitor obs_source_monitoring_monitor(bool
    /// was_on, bool enabled)`.
    pub fn oracle_obs_source_monitoring_monitor(was_on: bool, enabled: bool) -> u32;
    /// `enum obs_source_monitoring_route obs_source_monitoring_route(int
    /// type)`.
    pub fn oracle_obs_source_monitoring_route(monitoring_type: c_int) -> u32;
    /// `bool obs_source_monitoring_off_reads_mute(bool
    /// muted_for_monitor_only)`.
    pub fn oracle_obs_source_monitoring_off_reads_mute(muted_for_monitor_only: bool) -> u8;
    /// `bool obs_source_monitoring_off_unmutes(bool muted)`.
    pub fn oracle_obs_source_monitoring_off_unmutes(muted: bool) -> u8;
    /// `bool obs_source_monitoring_only_mutes(bool muted)`.
    pub fn oracle_obs_source_monitoring_only_mutes(muted: bool) -> u8;
    /// `float obs_source_monitoring_balance(float balance)`.
    pub fn oracle_obs_source_monitoring_balance(balance: f32) -> f32;
    /// `enum obs_source_monitoring_active
    /// obs_source_monitoring_audio_active(bool prev, bool active)`.
    pub fn oracle_obs_source_monitoring_audio_active(prev: bool, active: bool) -> u32;
    /// The header's values and sizes, by index into [`VALUE_NAMES`];
    /// `INT64_MIN` past the end.
    pub fn oracle_source_monitoring_value(which: usize) -> i64;
}
