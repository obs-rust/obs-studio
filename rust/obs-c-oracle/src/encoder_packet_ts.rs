//! `oracle_*` build of `libobs/obs-encoder-packet-ts.c.h`
//! (oracle/encoder_packet_ts.c): the encoder packet timestamp decisions,
//! renamed.
//!
//! The timebase comes back as [`OracleTimebase`];
//! [`oracle_encoder_packet_ts_layout`] says how the C sees it, the part of
//! `struct encoder_packet_time` the walk reads, and the "not found" value.

/// Index of each entry in `oracle_encoder_packet_ts_layout`, in the C's
/// order.
pub const LAYOUT_NAMES: [&str; 9] = [
    "sizeof(struct obs_encoder_packet_timebase)",
    "alignof(struct obs_encoder_packet_timebase)",
    "offsetof(struct obs_encoder_packet_timebase, num)",
    "offsetof(struct obs_encoder_packet_timebase, den)",
    "sizeof(struct encoder_packet_time)",
    "alignof(struct encoder_packet_time)",
    "offsetof(struct encoder_packet_time, pts)",
    "offsetof(struct encoder_packet_time, cts)",
    "OBS_ENCODER_PACKET_TS_NOT_FOUND",
];

/// `struct obs_encoder_packet_timebase` as the oracle returns it.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OracleTimebase {
    pub num: i32,
    pub den: i32,
}

/// `struct encoder_packet_time` as the oracle reads it.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OraclePacketTime {
    pub pts: i64,
    pub cts: u64,
    pub fer: u64,
    pub ferc: u64,
    pub pir: u64,
}

unsafe extern "C" {
    pub fn oracle_obs_encoder_packet_ts_timebase(
        timebase_num: u32,
        timebase_den: u32,
        frame_rate_divisor: u32,
    ) -> OracleTimebase;

    /// Undefined for a `timebase_den` of 0, `dts * 1000000` beyond `i64` and
    /// `i64::MIN / -1`: never call it with those.
    pub fn oracle_obs_encoder_packet_ts_dts_usec(dts: i64, timebase_den: i32) -> i64;

    pub fn oracle_obs_encoder_packet_ts_system(
        start_ts: u64,
        dts_usec: i64,
        offset_usec: i64,
    ) -> i64;

    pub fn oracle_obs_encoder_packet_ts_paused(sys_dts_usec: i64, pause_ts_offset: u64) -> i64;

    pub fn oracle_obs_encoder_packet_ts_find(
        times: *const OraclePacketTime,
        num: usize,
        pts: i64,
    ) -> usize;

    pub fn oracle_encoder_packet_ts_layout(which: usize) -> usize;
}
