//! Safe core of `libobs/obs-encoder-lifecycle.c.h`: the encoder callback
//! and connection lifecycle decisions of `obs-encoder.c`.
//!
//! One function per decision, each asked where the C makes it.
//! `obs-encoder.c` keeps `init_mutex`, `callbacks_mutex`, `outputs_mutex`,
//! the group mutex and every section they guard, the darrays (the callback
//! push and erase, the SEI copy), `get_callback_idx()` (a walk; its result
//! is passed), the `context.data`, `media`, `request_keyframe`,
//! `destroy_on_stop`, `keyframe`, `sent_first_packet`, mix and texture flag
//! tests (each a single field, tested where it always was, in its `&&` or
//! `||` order), `lcm()`, the connects and disconnects, the GPU and raw
//! starts and stops, `get_mix_for_video()`, `add_ready_encoder_group()`,
//! the group destroy, `obs_encoder_shutdown()`, `set_encoder_active()`,
//! `pause_reset()`, `request_keyframe()`, `get_sei()`,
//! `obs_output_active()`, the force stop and the NULL packet, the callbacks
//! and the profiler, and every write. It passes their values here. These
//! keep no state and call nothing in libobs.
//!
//! - `obs_encoder_start_internal()`: [`first`] and [`found`] under
//!   `callbacks_mutex`; the keyframe request asks [`not_video`],
//!   re-exported from [`crate::encoder_audio_start`], negated;
//! - `obs_encoder_stop()`: [`found`] and [`last`] under `callbacks_mutex`,
//!   [`group_idle`] (after `destroy_on_stop`) under the group mutex;
//! - `add_connection()` and `remove_connection()`: the route asks
//!   [`not_audio`], re-exported from [`crate::encoder_bind`], negated;
//!   under the group mutex [`lcm_unset`], [`started_inc`] and
//!   [`group_ready`], or [`started_dec`] and [`group_idle`];
//! - `gpu_encode_available()`: [`pass_texture`];
//! - `send_first_video_packet()`: [`prepend_sei`]; `send_packet()` asks
//!   [`not_video`], negated.
//!
//! No input comes from outside OBS (the counts and indexes from the
//! encoder's own lists, the caps and the SEI from the encoder plugin), and
//! nothing here can panic on any input. Nothing here divides: `lcm()` stays
//! C.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable
)]

pub use crate::encoder_audio_start::not_video;
pub use crate::encoder_bind::not_audio;

/// `OBS_ENCODER_CAP_PASS_TEXTURE`.
pub const OBS_ENCODER_CAP_PASS_TEXTURE: u32 = 1 << 1;

/// `DARRAY_INVALID`, `get_callback_idx()`'s "not found".
pub const NOT_FOUND: usize = usize::MAX;

/// `obs_encoder_start_internal()` under `callbacks_mutex`, before the add:
/// no callback yet, so this one connects the encoder.
#[must_use]
pub fn first(num_callbacks: usize) -> bool {
    num_callbacks == 0
}

/// `obs_encoder_start_internal()` and `obs_encoder_stop()`: `idx`,
/// `get_callback_idx()`'s result, is not [`NOT_FOUND`]. The start adds the
/// callback only when it is not found; the stop erases it only when it is.
#[must_use]
pub fn found(idx: usize) -> bool {
    idx != NOT_FOUND
}

/// `obs_encoder_stop()` after the erase: no callback is left, so the
/// encoder disconnects.
#[must_use]
pub fn last(num_callbacks: usize) -> bool {
    num_callbacks == 0
}

/// `gpu_encode_available()` with a mix: `caps` has
/// [`OBS_ENCODER_CAP_PASS_TEXTURE`]. Only then does the C test the mix's
/// texture flags.
#[must_use]
pub fn pass_texture(caps: u32) -> bool {
    (caps & OBS_ENCODER_CAP_PASS_TEXTURE) != 0
}

/// `add_connection()` under the group mutex: the divisor LCM is 0, so the C
/// sets it to 1 before `lcm()`.
#[must_use]
pub fn lcm_unset(lcm: u32) -> bool {
    lcm == 0
}

/// `add_connection()`: `num_encoders_started` plus one, wrapping as the C's
/// `uint32_t` does.
#[must_use]
pub fn started_inc(started: u32) -> u32 {
    started.wrapping_add(1)
}

/// `add_connection()` after the count: `started` is at least
/// `num_encoders` (`encoders.num`), so the group is ready.
#[must_use]
pub fn group_ready(started: u32, num_encoders: usize) -> bool {
    // a count past usize (no host has one) is past every encoder count
    usize::try_from(started).map_or(true, |started| started >= num_encoders)
}

/// `remove_connection()`: `num_encoders_started` minus one, wrapping as the
/// C's `uint32_t` does.
#[must_use]
pub fn started_dec(started: u32) -> u32 {
    started.wrapping_sub(1)
}

/// `remove_connection()` after the count, and `obs_encoder_stop()` after
/// `destroy_on_stop`: no encoder of the group is started.
#[must_use]
pub fn group_idle(started: u32) -> bool {
    started == 0
}

/// `send_first_video_packet()` on the first keyframe: `get_sei()` returned
/// true (`got`) with a pointer (`has_sei`) and a size that is not 0, so the
/// SEI goes in front of the packet.
#[must_use]
pub fn prepend_sei(got: bool, has_sei: bool, size: usize) -> bool {
    got && has_sei && size != 0
}
