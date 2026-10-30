//! `oracle_*` build of `libobs/obs-output-pause.c.h`
//! (oracle/output_pause.c): the output pause plan, renamed.

unsafe extern "C" {
    pub fn oracle_obs_output_pause_allowed(flags: u32) -> bool;

    pub fn oracle_obs_output_pause_already(paused: bool, pause: bool) -> bool;

    /// `video` and `audio` point to 10 and 6 readable `uintptr_t`,
    /// `video_lock` and `audio_lock` to a writable `uint32_t` each.
    pub fn oracle_obs_output_pause_lock_set(
        video: *const [usize; 10],
        audio: *const [usize; 6],
        video_lock: *mut u32,
        audio_lock: *mut u32,
    );

    /// Undefined (a division by zero) for an `interval` of 0: never call it
    /// with that.
    pub fn oracle_obs_output_pause_closest_ts(last_video_ts: u64, interval: u64, now: u64) -> u64;

    pub fn oracle_obs_output_pause_started(ts_start: u64) -> bool;

    pub fn oracle_obs_output_pause_ended(ts_end: u64) -> bool;

    pub fn oracle_obs_output_pause_offset(ts_offset: u64, ts_start: u64, ts_end: u64) -> u64;
}
