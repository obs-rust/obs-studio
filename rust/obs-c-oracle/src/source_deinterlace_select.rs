//! `oracle_*` build of `libobs/obs-source-deinterlace-select.c.h`
//! (oracle/source_deinterlace_select.c): the deinterlace frame selection,
//! renamed, calling the C of `oracle/source_deinterlace_pacing.c`. The
//! results' `bool`s are declared as `u8`, the bytes the C stores or
//! returns, so a parity test compares them without assuming they are 0 or
//! 1; the enums are declared as `u32`, the width the C returns them in.

/// Index of each entry in `oracle_source_deinterlace_select_constant`, in
/// the C's order.
pub const CONSTANT_NAMES: [&str; 6] = [
    "OBS_SOURCE_DEINTERLACE_SELECT_MAX_TS_VAR",
    "OBS_SOURCE_DEINTERLACE_SELECT_OFFSET_BUFFERED",
    "OBS_SOURCE_DEINTERLACE_SELECT_OFFSET_REANCHOR",
    "OBS_SOURCE_DEINTERLACE_SELECT_UPDATE_NONE",
    "OBS_SOURCE_DEINTERLACE_SELECT_UPDATE_UPLOAD",
    "OBS_SOURCE_DEINTERLACE_SELECT_UPDATE_SWAP",
];

/// `struct obs_source_deinterlace_select_first_result`, bytes for bools.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FirstResult {
    pub first: u8,
    pub flag_front: u8,
}

/// `struct obs_source_deinterlace_select_walk_result`, bytes for bools.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WalkResult {
    pub drop_front: usize,
    pub last_frame_ts: u64,
    pub offset_reset: u8,
    pub pairs: u8,
    pub flag_front: u8,
    pub ready: u8,
}

/// `struct obs_source_deinterlace_select_pair_result`, bytes for bools.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PairResult {
    pub prev_frame: u8,
    pub set_frame_ts: u8,
    pub frame_ts: u64,
}

unsafe extern "C" {
    /// `obs_source_deinterlace_select_first(uint64_t last_frame_ts, size_t count)`.
    pub fn oracle_obs_source_deinterlace_select_first(
        last_frame_ts: u64,
        count: usize,
    ) -> FirstResult;
    /// `obs_source_deinterlace_select_walk(const uint64_t *timestamps, size_t count, uint64_t sys_time, uint64_t last_frame_ts, uint64_t last_sys_timestamp, bool async_unbuffered)`.
    pub fn oracle_obs_source_deinterlace_select_walk(
        timestamps: *const u64,
        count: usize,
        sys_time: u64,
        last_frame_ts: u64,
        last_sys_timestamp: u64,
        async_unbuffered: bool,
    ) -> WalkResult;
    /// `obs_source_deinterlace_select_unbuffered_pair(uint64_t timestamp, uint64_t next_timestamp, uint64_t offset, uint64_t sys_time)`.
    pub fn oracle_obs_source_deinterlace_select_unbuffered_pair(
        timestamp: u64,
        next_timestamp: u64,
        offset: u64,
        sys_time: u64,
    ) -> PairResult;
    /// `bool obs_source_deinterlace_select_take_reads_flag(size_t remaining)`.
    pub fn oracle_obs_source_deinterlace_select_take_reads_flag(remaining: usize) -> u8;
    /// `bool obs_source_deinterlace_select_take_pair(size_t remaining, bool prev_frame)`.
    pub fn oracle_obs_source_deinterlace_select_take_pair(remaining: usize, prev_frame: bool)
    -> u8;
    /// `bool obs_source_deinterlace_select_take_init_last(uint64_t last_frame_ts)`.
    pub fn oracle_obs_source_deinterlace_select_take_init_last(last_frame_ts: u64) -> u8;
    /// `enum obs_source_deinterlace_select_offset_kind obs_source_deinterlace_select_take_offset(bool async_unbuffered)`.
    pub fn oracle_obs_source_deinterlace_select_take_offset(async_unbuffered: bool) -> u32;
    /// `enum obs_source_deinterlace_select_update_step obs_source_deinterlace_select_update(bool has_prev, bool updated)`.
    pub fn oracle_obs_source_deinterlace_select_update(has_prev: bool, updated: bool) -> u32;
    /// The entry named `CONSTANT_NAMES[which]`; 0 out of range.
    pub fn oracle_source_deinterlace_select_constant(which: usize) -> u64;
    /// The layouts (see the C): 16 entries; 0 out of range.
    pub fn oracle_source_deinterlace_select_layout(which: usize) -> usize;
}
