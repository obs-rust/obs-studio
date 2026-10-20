//! `oracle_*` build of
//! `libobs/audio-monitoring/pulse/pulseaudio-stream-flags.c.h`
//! (oracle/pulse_stream_flags.c).

use core::ffi::{c_char, c_int, c_uint};

/// `struct pulseaudio_monitor_backlog`, as the oracle sees it.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct OracleBacklog {
    pub window_start: u64,
    pub window_min: u64,
}

/// Index of each value in `oracle_pulse_def_value`, in the C's order.
pub const DEF_NAMES: [&str; 13] = [
    "PA_STREAM_START_CORKED",
    "PA_STREAM_INTERPOLATE_TIMING",
    "PA_STREAM_AUTO_TIMING_UPDATE",
    "PA_STREAM_DONT_MOVE",
    "PA_SUBSCRIPTION_EVENT_SINK",
    "PA_SUBSCRIPTION_EVENT_SOURCE",
    "PA_SUBSCRIPTION_EVENT_SINK_INPUT",
    "PA_SUBSCRIPTION_EVENT_FACILITY_MASK",
    "PA_SUBSCRIPTION_EVENT_NEW",
    "PA_SUBSCRIPTION_EVENT_CHANGE",
    "PA_SUBSCRIPTION_EVENT_REMOVE",
    "PA_SUBSCRIPTION_EVENT_TYPE_MASK",
    "PA_INVALID_INDEX",
];

unsafe extern "C" {
    pub fn oracle_pulseaudio_monitor_stream_flags(device_id: *const c_char) -> c_uint;
    pub fn oracle_pulseaudio_monitor_reset_on_sink_event(
        event: c_uint,
        event_idx: u32,
        sink_idx: u32,
        fallback: bool,
    ) -> bool;
    pub fn oracle_pulseaudio_monitor_backlog_excess(
        backlog: *mut OracleBacklog,
        now_ns: u64,
        queued_usec: u64,
        target_usec: u64,
    ) -> u64;
    /// The value named `DEF_NAMES[which]`.
    pub fn oracle_pulse_def_value(which: usize) -> u32;
    pub fn oracle_pulse_backlog_window() -> u64;
    pub fn oracle_pulse_backlog_slack() -> u64;
    /// 0 size, 1 align, 2 offset of `window_start`, 3 offset of `window_min`.
    pub fn oracle_pulse_backlog_layout(which: usize) -> usize;
    /// 1 when the oracle was built against libpulse's own `<pulse/def.h>`.
    pub fn oracle_pulse_def_is_real() -> c_int;
}
