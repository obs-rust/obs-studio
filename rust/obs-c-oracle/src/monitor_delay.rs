//! `oracle_*` build of `libobs/audio-monitoring/monitor-delay.c.h`
//! (oracle/monitor_delay.c).
//!
//! `enum audio_monitor_delay_verdict` (0 to 2) crosses as `unsigned int`.

use core::ffi::c_uint;

/// Index of each value in `oracle_monitor_delay_value`, in the C's order.
pub const VALUE_NAMES: [&str; 8] = [
    "AUDIO_MONITOR_DELAY_STALE_NS",
    "AUDIO_MONITOR_DELAY_MAX_DIFF_NS",
    "AUDIO_MONITOR_DELAY_SYNC_NS",
    "AUDIO_MONITOR_VIDEO_STALL_NS",
    "AUDIO_MONITOR_DELAY_PLAY",
    "AUDIO_MONITOR_DELAY_HOLD",
    "AUDIO_MONITOR_DELAY_DRAG",
    "sizeof(enum audio_monitor_delay_verdict)",
];

unsafe extern "C" {
    pub fn oracle_audio_monitor_delay_stale(last_recv_time: *mut u64, cur_time: u64) -> bool;
    pub fn oracle_audio_monitor_delay_diff(
        cur_ts: u64,
        pad_ns: u64,
        last_frame_ts: u64,
        front_ts: *mut u64,
    ) -> i64;
    pub fn oracle_audio_monitor_delay_unsynced(last_frame_ts: u64, diff: i64) -> bool;
    pub fn oracle_audio_monitor_delay_stalled(time_since_prev: u64) -> bool;
    pub fn oracle_audio_monitor_delay_judge_diff(bad_diff: bool, diff: i64) -> c_uint;
    /// The value at `which` in [`VALUE_NAMES`], or -1 past the end.
    pub fn oracle_monitor_delay_value(which: usize) -> i64;
}
