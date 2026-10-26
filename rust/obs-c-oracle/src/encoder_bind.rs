//! `oracle_*` build of `libobs/obs-encoder-bind.c.h`
//! (oracle/encoder_bind.c): the encoder media binding and shutdown checks,
//! renamed.
//!
//! Every function takes plain values; the timebase comes back by value as
//! [`OracleTimebase`], whose layout [`oracle_encoder_bind_layout`] checks.
//! Nothing here divides.

use std::ffi::c_int;

/// Index of each entry in `oracle_encoder_bind_constant`, in the C's order.
pub const CONSTANT_NAMES: [&str; 1] = ["OBS_ENCODER_BIND_TYPE_AUDIO"];

/// Index of each entry in `oracle_encoder_bind_layout`, in the C's order.
pub const LAYOUT_NAMES: [&str; 4] = [
    "sizeof(struct obs_encoder_bind_timebase)",
    "alignof(struct obs_encoder_bind_timebase)",
    "offsetof(struct obs_encoder_bind_timebase, num)",
    "offsetof(struct obs_encoder_bind_timebase, den)",
];

/// `struct obs_encoder_bind_timebase`, as the C returns it.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OracleTimebase {
    pub num: u32,
    pub den: u32,
}

unsafe extern "C" {
    pub fn oracle_obs_encoder_bind_not_audio(encoder_type: c_int) -> bool;

    pub fn oracle_obs_encoder_bind_video_timebase(
        has_video: bool,
        fps_num: u32,
        fps_den: u32,
    ) -> OracleTimebase;

    pub fn oracle_obs_encoder_bind_audio_timebase(
        has_audio: bool,
        sample_rate: u32,
    ) -> OracleTimebase;

    pub fn oracle_obs_encoder_bind_divisor_override(frame_rate_divisor: u32) -> bool;

    pub fn oracle_obs_encoder_bind_locked(trylock_result: c_int) -> bool;

    pub fn oracle_obs_encoder_bind_shutdown_unstarted(initialized: bool, needed: bool) -> bool;

    /// The constant as the C sees it, by index into [`CONSTANT_NAMES`]; -1
    /// past the end.
    pub fn oracle_encoder_bind_constant(which: usize) -> i64;

    /// The struct's layout as the C sees it, by index into
    /// [`LAYOUT_NAMES`]; `usize::MAX` past the end.
    pub fn oracle_encoder_bind_layout(which: usize) -> usize;
}
