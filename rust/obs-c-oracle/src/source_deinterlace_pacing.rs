//! `oracle_*` build of `libobs/obs-source-deinterlace-pacing.c.h`
//! (oracle/source_deinterlace_pacing.c): the 2x deinterlace frame pacing,
//! renamed. The `bool` results are declared as `u8`, the byte the C
//! returns, so a parity test compares it without assuming it is 0 or 1.

/// Index of each entry in `oracle_source_deinterlace_pacing_constant`, in
/// the C's order.
pub const CONSTANT_NAMES: [&str; 3] = [
    "OBS_SOURCE_DEINTERLACE_PACING_FRAME_TOLERANCE",
    "OBS_SOURCE_DEINTERLACE_PACING_TWOX_TOLERANCE",
    "OBS_SOURCE_DEINTERLACE_PACING_TS_JUMP_THRESHOLD",
];

unsafe extern "C" {
    /// `bool obs_source_deinterlace_pacing_hold(...)`.
    pub fn oracle_obs_source_deinterlace_pacing_hold(
        frame_ts: u64,
        offset: u64,
        half_duration: u32,
        sys_time: u64,
    ) -> u8;
    /// `bool obs_source_deinterlace_pacing_wait_for_next(...)`.
    pub fn oracle_obs_source_deinterlace_pacing_wait_for_next(
        timestamp: u64,
        next_timestamp: u64,
        offset: u64,
        sys_time: u64,
    ) -> u8;
    pub fn oracle_obs_source_deinterlace_pacing_waiting_frame_ts(
        timestamp: u64,
        next_timestamp: u64,
    ) -> u64;
    pub fn oracle_obs_source_deinterlace_pacing_half_duration(
        timestamp: u64,
        prev_ts: u64,
        prev_half: u32,
        frame_interval_ns: u64,
    ) -> u32;
    pub fn oracle_obs_source_deinterlace_pacing_buffered_offset(
        video_time: u64,
        last_frame_ts: u64,
    ) -> u64;
    pub fn oracle_obs_source_deinterlace_pacing_reanchor(
        offset: u64,
        video_time: u64,
        frame_ts: u64,
        half_duration: u32,
        frame_interval_ns: u64,
    ) -> u64;
    /// The entry named `CONSTANT_NAMES[which]`; 0 out of range.
    pub fn oracle_source_deinterlace_pacing_constant(which: usize) -> u64;
}
