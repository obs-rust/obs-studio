//! Safe core of `libobs/obs-encoder-group-reconfig.c.h`: the encoder group
//! reconfigure rules of `obs-encoder.c`.
//!
//! When an active encoder of a group gets new settings, every encoder of
//! the group applies them on the same frame, so that their new keyframes
//! line up (the multitrack video output's encoders, at their frame rate
//! divisors). `obs-encoder.c` keeps `obs_encoder_valid()`,
//! `obs_data_apply()`, the `context.data`, `info.update` and active tests,
//! the group mutex, every read and write of the encoder and the group (it
//! copies the reconfigure fields in and out around each call),
//! `info.update()` in `do_encode()` and the GPU encode thread,
//! `start_timestamp` and the LCM reset to 1, `obs_encoder_set_group()` and
//! the group create and destroy. These keep no state, take no lock and
//! call nothing.
//!
//! - [`update`] (`obs_encoder_update()`, the group branch): start a new
//!   request, or hold a second one back ([`Group::reconfigure_again`]);
//! - [`new_request`] (`encoder_group_new_reconfigure_request()`);
//! - [`frame`] (`handle_encoder_group_reconfigure_request()`, for each video
//!   frame of a grouped encoder): apply the settings on this frame or not,
//!   and the group's next shared frame ([`next_shared_pts`]);
//! - [`lcm`] (`add_connection()`): the least common multiple of the group's
//!   frame rate divisors;
//! - [`reset`] (`remove_connection()`, once no encoder of the group is
//!   started).
//!
//! No input comes from outside OBS (the divisors and the timebase from the
//! frontend's video settings, the pts from libobs' own count), but nothing
//! here can panic on any input. The C's unsigned counters and `uint32_t`
//! products wrap, and so do these.
//!
//! Intentional differences, unreachable (undefined in C): [`lcm`] answers 0
//! for a 0 on either side (C divides by 0), wraps a product above
//! `INT_MAX` (C overflows `int`) and takes a value above `INT_MAX` as the
//! unsigned number it is (C's `gcd()` divides by 0 or does not end);
//! [`next_shared_pts`] wraps beyond `i64` and leaves the pts unrounded for
//! a timestep of 0 (C divides by 0).

#![deny(
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::unreachable
)]

/// The reconfigure fields of `struct obs_encoder_group` (`struct
/// obs_encoder_group_reconfig`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Group {
    /// The request counter; 0 is "no request yet".
    pub reconfigure_request: u64,
    /// The pts on which the encoders apply the current request.
    pub next_pts: i64,
    /// The encoders that took the current request.
    pub encoders_updated_next_pts: u32,
    /// The encoders that applied the current request.
    pub encoders_reconfigured: u32,
    /// An update came for an encoder that already applied the current
    /// request: start a new one once all applied it.
    pub reconfigure_again: bool,
}

/// The reconfigure fields of `struct obs_encoder` (`struct
/// obs_encoder_reconfig`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Encoder {
    /// The request this encoder took.
    pub last_reconfigure_request: u64,
    /// The request this encoder applied.
    pub last_handled_reconfigure_request: u64,
}

/// What [`frame`] reads of the group and the encoder besides the
/// reconfigure fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FrameInput {
    /// The group's `encoders.num`.
    pub num_encoders: usize,
    /// The group's `num_encoders_started`.
    pub num_encoders_started: u32,
    /// The group's `frame_rate_divisors_lcm`.
    pub frame_rate_divisors_lcm: u32,
    /// The encoder's `cur_pts`, the pts of the frame it encodes next.
    pub cur_pts: i64,
    /// The encoder's `timebase_num`.
    pub timebase_num: u32,
    /// The encoder's `frame_rate_divisor`.
    pub frame_rate_divisor: u32,
}

/// `encoder_group_new_reconfigure_request()`: the request counter steps by
/// one (wrapping), and the counts of this request and
/// [`Group::reconfigure_again`] start again.
pub fn new_request(group: &mut Group) {
    let _ = group;
    todo!()
}

/// `obs_encoder_update()` for an active encoder in a group:
/// `num_encoders` is the group's `encoders.num`, and
/// `last_handled_reconfigure_request` the encoder's. The first request, or
/// one after every encoder applied the last, starts a new request; an
/// encoder that already applied the current request sets
/// [`Group::reconfigure_again`].
pub fn update(group: &mut Group, num_encoders: usize, last_handled_reconfigure_request: u64) {
    let _ = (group, num_encoders, last_handled_reconfigure_request);
    todo!()
}

/// `handle_encoder_group_reconfigure_request()`, for each video frame of an
/// encoder in a group. True: the encoder applies its settings on this frame
/// (the caller sets `reconfigure_requested`).
#[must_use]
pub fn frame(group: &mut Group, encoder: &mut Encoder, input: &FrameInput) -> bool {
    let _ = (group, encoder, input);
    todo!()
}

/// The next shared frame `handle_encoder_group_reconfigure_request()`
/// proposes: `cur_pts + timebase_num * frame_rate_divisor` (the product as
/// `uint32_t`), rounded up to a multiple of `timebase_num *
/// frame_rate_divisors_lcm` (also as `uint32_t`), truncating as C's `/` and
/// `%` do.
#[must_use]
pub fn next_shared_pts(
    cur_pts: i64,
    timebase_num: u32,
    frame_rate_divisor: u32,
    frame_rate_divisors_lcm: u32,
) -> i64 {
    let _ = (
        cur_pts,
        timebase_num,
        frame_rate_divisor,
        frame_rate_divisors_lcm,
    );
    todo!()
}

/// `add_connection()`: the group's new `frame_rate_divisors_lcm`,
/// `lcm(lcm, frame_rate_divisor)` as C's `gcd()` and `lcm()` compute it in
/// `int`, stored as `uint32_t`.
#[must_use]
pub fn lcm(lcm: u32, frame_rate_divisor: u32) -> u32 {
    let _ = (lcm, frame_rate_divisor);
    todo!()
}

/// `remove_connection()`, once no encoder of the group is started: every
/// reconfigure field back to 0 (false).
pub fn reset(group: &mut Group) {
    let _ = group;
    todo!()
}
