//! `oracle_*` build of `libobs/obs-encoder-lifecycle.c.h`
//! (oracle/encoder_lifecycle.c): the encoder callback and connection
//! lifecycle decisions, renamed.
//!
//! Every function takes and returns plain values. Nothing here divides.

use std::ffi::c_int;

/// Index of each entry in `oracle_encoder_lifecycle_constant`, in the C's
/// order.
pub const CONSTANT_NAMES: [&str; 2] = [
    "OBS_ENCODER_LIFECYCLE_CAP_PASS_TEXTURE",
    "OBS_ENCODER_LIFECYCLE_NOT_FOUND",
];

unsafe extern "C" {
    pub fn oracle_obs_encoder_lifecycle_first(num_callbacks: usize) -> bool;

    pub fn oracle_obs_encoder_lifecycle_found(idx: usize) -> bool;

    pub fn oracle_obs_encoder_lifecycle_last(num_callbacks: usize) -> bool;

    pub fn oracle_obs_encoder_lifecycle_pass_texture(caps: u32) -> bool;

    pub fn oracle_obs_encoder_lifecycle_lcm_unset(lcm: u32) -> bool;

    pub fn oracle_obs_encoder_lifecycle_started_inc(started: u32) -> u32;

    pub fn oracle_obs_encoder_lifecycle_group_ready(started: u32, num_encoders: usize) -> bool;

    pub fn oracle_obs_encoder_lifecycle_started_dec(started: u32) -> u32;

    pub fn oracle_obs_encoder_lifecycle_group_idle(started: u32) -> bool;

    pub fn oracle_obs_encoder_lifecycle_prepend_sei(got: bool, has_sei: bool, size: usize) -> bool;

    /// The constant as the C sees it, by index into [`CONSTANT_NAMES`];
    /// `*ok` is 0 past the end.
    pub fn oracle_encoder_lifecycle_constant(which: usize, ok: *mut c_int) -> u64;
}
