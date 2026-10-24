//! `oracle_*` build of `libobs/obs-source-async-pacing.c.h`
//! (oracle/source_async_pacing.c): the async video frame pacing, renamed.
//! The result's `bool` is declared as `u8`, the byte the C stores, so a
//! parity test compares it without assuming it is 0 or 1.

/// Index of each entry in `oracle_source_async_pacing_constant`, in the
/// C's order.
pub const CONSTANT_NAMES: [&str; 2] = [
    "OBS_SOURCE_ASYNC_PACING_MAX_TS_VAR",
    "OBS_SOURCE_ASYNC_PACING_FRAME_TOLERANCE",
];

/// `struct obs_source_async_pacing_result`, with `take_first` as its byte.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AsyncPacingResult {
    pub drop_front: usize,
    pub last_frame_ts: u64,
    pub take_first: u8,
}

unsafe extern "C" {
    /// `struct obs_source_async_pacing_result obs_source_async_pacing(...)`.
    pub fn oracle_obs_source_async_pacing(
        timestamps: *const u64,
        count: usize,
        sys_time: u64,
        last_frame_ts: u64,
        last_sys_timestamp: u64,
        async_unbuffered: bool,
    ) -> AsyncPacingResult;
    /// The entry named `CONSTANT_NAMES[which]`; 0 out of range.
    pub fn oracle_source_async_pacing_constant(which: usize) -> u64;
    /// The result's size, then the offsets of `drop_front`,
    /// `last_frame_ts` and `take_first`; 0 out of range.
    pub fn oracle_source_async_pacing_result_layout(which: usize) -> usize;
}
