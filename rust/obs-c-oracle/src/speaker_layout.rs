//! `oracle_*` build of `libobs/audio-monitoring/monitor-speaker-layout.c.h`
//! (oracle/speaker_layout.c).
//!
//! `enum speaker_layout` crosses as `c_int`.

use core::ffi::c_int;

/// Index of each value in `oracle_monitor_speaker_layout_value`, in the C's order.
pub const VALUE_NAMES: [&str; 6] = [
    "AUDIO_MONITOR_SPEAKER_MASK_2POINT1",
    "AUDIO_MONITOR_SPEAKER_MASK_SURROUND",
    "AUDIO_MONITOR_SPEAKER_MASK_4POINT1",
    "AUDIO_MONITOR_SPEAKER_MASK_5POINT1",
    "AUDIO_MONITOR_SPEAKER_MASK_7POINT1",
    "sizeof(enum speaker_layout)",
];

unsafe extern "C" {
    pub fn oracle_audio_monitor_speaker_layout(mask: u32, channels: u32) -> c_int;
    /// The value at `which` in [`VALUE_NAMES`], or -1 past the end.
    pub fn oracle_monitor_speaker_layout_value(which: usize) -> i64;
}
