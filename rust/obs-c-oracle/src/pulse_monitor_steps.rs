//! `oracle_*` build of
//! `libobs/audio-monitoring/pulse/pulseaudio-monitor-steps.c.h`
//! (oracle/pulse_monitor_steps.c).

use core::ffi::{c_char, c_int, c_long, c_uint};

/// Index of each value in `oracle_pulse_monitor_value`, in the C's order.
pub const VALUE_NAMES: [&str; 14] = [
    "PULSEAUDIO_MONITOR_DO_NOT_SELF_MONITOR",
    "PULSEAUDIO_MONITOR_SINK_INFO",
    "PULSEAUDIO_MONITOR_SINK_END",
    "PULSEAUDIO_MONITOR_SINK_ERROR",
    "PA_CONTEXT_UNCONNECTED",
    "PA_CONTEXT_CONNECTING",
    "PA_CONTEXT_AUTHORIZING",
    "PA_CONTEXT_SETTING_NAME",
    "PA_CONTEXT_READY",
    "PA_CONTEXT_FAILED",
    "PA_CONTEXT_TERMINATED",
    "PA_SAMPLE_INVALID",
    "sizeof(uint_fast32_t)",
    "sizeof(long)",
];

unsafe extern "C" {
    pub fn oracle_pulseaudio_monitor_grow(queued: usize, tlength: u32) -> bool;
    pub fn oracle_pulseaudio_monitor_grown_tlength(queued: usize) -> u32;
    pub fn oracle_pulseaudio_monitor_uncork(queued: usize, tlength: u32) -> bool;
    pub fn oracle_pulseaudio_monitor_write_more(queued: usize) -> bool;
    pub fn oracle_pulseaudio_monitor_write_short(asked: usize, queued: usize) -> bool;
    pub fn oracle_pulseaudio_monitor_playback_idle(activate_refs: c_long) -> bool;
    /// `pulseaudio_monitor_playback_bytes`, the frame size converted to
    /// `uint_fast32_t` as a C caller's would be.
    pub fn oracle_pulse_monitor_playback_bytes(bytes_per_frame: u64, frames: u32) -> usize;
    pub fn oracle_pulseaudio_monitor_needs_volume(vol: f32) -> bool;
    pub fn oracle_pulseaudio_monitor_sink_result(eol: c_int) -> c_uint;
    pub fn oracle_pulseaudio_monitor_has_device(id: *const c_char) -> bool;
    pub fn oracle_pulseaudio_monitor_self_monitor_check(output_flags: u32) -> bool;
    pub fn oracle_pulseaudio_monitor_is_default(id: *const c_char) -> bool;
    pub fn oracle_pulseaudio_monitor_fallback(format: c_int, id: *const c_char) -> bool;
    pub fn oracle_pulseaudio_monitor_sink_named(device: *const c_char) -> bool;
    pub fn oracle_pulseaudio_monitor_format_invalid(format: c_int) -> bool;
    pub fn oracle_pulseaudio_monitor_tlength_usec() -> u64;
    pub fn oracle_pulseaudio_monitor_open_flags(fallback: bool, id: *const c_char) -> c_uint;
    pub fn oracle_pulseaudio_monitor_event_queued(event: c_uint) -> bool;
    pub fn oracle_pulseaudio_monitor_context_good(state: c_uint) -> bool;
    pub fn oracle_pulseaudio_monitor_last(num: usize) -> bool;
    /// The value named `VALUE_NAMES[which]`.
    pub fn oracle_pulse_monitor_value(which: usize) -> i64;
}
