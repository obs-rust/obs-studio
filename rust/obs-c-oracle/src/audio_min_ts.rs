//! `oracle_*` build of `libobs/obs-audio-min-ts.c.h`
//! (oracle/audio_min_ts.c). The C enum comes back as `int`.

/// Index of each entry in `oracle_audio_min_ts_layout`, in the C's order.
pub const LAYOUT_NAMES: [&str; 4] = [
    "sizeof(enum obs_audio_buffering_choice)",
    "OBS_AUDIO_BUFFERING_NONE",
    "OBS_AUDIO_BUFFERING_FIXED",
    "OBS_AUDIO_BUFFERING_ADD",
];

unsafe extern "C" {
    pub fn oracle_obs_audio_min_ts_earlier(pending: bool, audio_ts: u64, min_ts: *mut u64) -> bool;
    pub fn oracle_obs_audio_min_ts_insufficient(
        audio_render: bool,
        pending: *mut bool,
        audio_ts: u64,
        buffered: usize,
        sample_rate: usize,
        min_ts: u64,
    ) -> bool;
    pub fn oracle_obs_audio_min_ts_buffering(
        fixed_buffer: bool,
        maxed: bool,
        min_ts: u64,
        tick_start: u64,
    ) -> i32;
    /// The entry named `LAYOUT_NAMES[which]`; 0 out of range.
    pub fn oracle_audio_min_ts_layout(which: usize) -> usize;
}
