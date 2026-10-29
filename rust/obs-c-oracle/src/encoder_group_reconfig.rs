//! `oracle_*` build of `libobs/obs-encoder-group-reconfig.c.h`
//! (oracle/encoder_group_reconfig.c): the encoder group reconfigure rules,
//! renamed.
//!
//! The reconfigure fields cross as [`OracleGroup`] and [`OracleEncoder`];
//! [`oracle_encoder_group_reconfig_layout`] says how the C sees them.

/// Index of each entry in `oracle_encoder_group_reconfig_layout`, in the
/// C's order.
pub const LAYOUT_NAMES: [&str; 11] = [
    "sizeof(struct obs_encoder_group_reconfig)",
    "alignof(struct obs_encoder_group_reconfig)",
    "offsetof(struct obs_encoder_group_reconfig, reconfigure_request)",
    "offsetof(struct obs_encoder_group_reconfig, next_pts)",
    "offsetof(struct obs_encoder_group_reconfig, encoders_updated_next_pts)",
    "offsetof(struct obs_encoder_group_reconfig, encoders_reconfigured)",
    "offsetof(struct obs_encoder_group_reconfig, reconfigure_again)",
    "sizeof(struct obs_encoder_reconfig)",
    "alignof(struct obs_encoder_reconfig)",
    "offsetof(struct obs_encoder_reconfig, last_reconfigure_request)",
    "offsetof(struct obs_encoder_reconfig, last_handled_reconfigure_request)",
];

/// `struct obs_encoder_group_reconfig` as the oracle reads and writes it.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OracleGroup {
    pub reconfigure_request: u64,
    pub next_pts: i64,
    pub encoders_updated_next_pts: u32,
    pub encoders_reconfigured: u32,
    pub reconfigure_again: bool,
}

/// `struct obs_encoder_reconfig` as the oracle reads and writes it.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OracleEncoder {
    pub last_reconfigure_request: u64,
    pub last_handled_reconfigure_request: u64,
}

unsafe extern "C" {
    pub fn oracle_obs_encoder_group_reconfig_new_request(group: *mut OracleGroup);

    pub fn oracle_obs_encoder_group_reconfig_update(
        group: *mut OracleGroup,
        num_encoders: usize,
        last_handled_reconfigure_request: u64,
    );

    /// Undefined for a timestep of 0 (`timebase_num` or the LCM 0) and
    /// `next_pts` beyond `i64`: never call it with those.
    pub fn oracle_obs_encoder_group_reconfig_frame(
        group: *mut OracleGroup,
        num_encoders: usize,
        num_encoders_started: u32,
        frame_rate_divisors_lcm: u32,
        encoder: *mut OracleEncoder,
        cur_pts: i64,
        timebase_num: u32,
        frame_rate_divisor: u32,
    ) -> bool;

    /// Undefined for a 0, a value above `INT_MAX` and an LCM above
    /// `INT_MAX`: never call it with those.
    pub fn oracle_obs_encoder_group_reconfig_lcm(lcm: u32, frame_rate_divisor: u32) -> u32;

    pub fn oracle_obs_encoder_group_reconfig_reset(group: *mut OracleGroup);

    pub fn oracle_encoder_group_reconfig_layout(which: usize) -> usize;
}
