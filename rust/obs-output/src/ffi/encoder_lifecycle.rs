//! C ABI for `libobs/obs-encoder-lifecycle.c.h`, the encoder callback and
//! connection lifecycle decisions of `obs-encoder.c`, declared in
//! `libobs/obs-encoder-lifecycle.h` without `EXPORT`.
//!
//! `obs-encoder.c.h` (still C) takes `init_mutex`, `callbacks_mutex`,
//! `outputs_mutex` and the group mutex where it always did, walks the
//! callbacks (`get_callback_idx()`, passing its result), pushes and erases
//! them, tests `context.data`, `media`, `request_keyframe`,
//! `destroy_on_stop`, the packet's `keyframe`, `sent_first_packet`, the mix
//! and its texture flags and each output's `obs_output_active()` in their
//! order, calls `lcm()`, the connects and disconnects, the GPU and raw
//! starts and stops, `get_sei()` and the rest, and writes every answer back.
//! So `struct obs_encoder` and its group are never mirrored: only counts,
//! an index, the type, the caps and three SEI values cross, by value. These
//! shims take no lock, call nothing in libobs and dereference no pointer.
//! They stay hidden (`rust-hidden.txt`). The type tests are the
//! `obs_encoder_bind_not_audio` and `obs_encoder_audio_start_not_video`
//! shims in [`super::encoder_bind`] and [`super::encoder_audio_start`],
//! which the callers ask too.
//!
//! No intentional differences.

use crate::encoder_lifecycle as core;

/// `bool obs_encoder_lifecycle_first(size_t num_callbacks)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_lifecycle_first(num_callbacks: usize) -> bool {
    core::first(num_callbacks)
}

/// `bool obs_encoder_lifecycle_found(size_t idx)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_lifecycle_found(idx: usize) -> bool {
    core::found(idx)
}

/// `bool obs_encoder_lifecycle_last(size_t num_callbacks)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_lifecycle_last(num_callbacks: usize) -> bool {
    core::last(num_callbacks)
}

/// `bool obs_encoder_lifecycle_pass_texture(uint32_t caps)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_lifecycle_pass_texture(caps: u32) -> bool {
    core::pass_texture(caps)
}

/// `bool obs_encoder_lifecycle_lcm_unset(uint32_t lcm)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_lifecycle_lcm_unset(lcm: u32) -> bool {
    core::lcm_unset(lcm)
}

/// `uint32_t obs_encoder_lifecycle_started_inc(uint32_t started)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_lifecycle_started_inc(started: u32) -> u32 {
    core::started_inc(started)
}

/// `bool obs_encoder_lifecycle_group_ready(uint32_t started, size_t
/// num_encoders)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_lifecycle_group_ready(started: u32, num_encoders: usize) -> bool {
    core::group_ready(started, num_encoders)
}

/// `uint32_t obs_encoder_lifecycle_started_dec(uint32_t started)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_lifecycle_started_dec(started: u32) -> u32 {
    core::started_dec(started)
}

/// `bool obs_encoder_lifecycle_group_idle(uint32_t started)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_lifecycle_group_idle(started: u32) -> bool {
    core::group_idle(started)
}

/// `bool obs_encoder_lifecycle_prepend_sei(bool got, bool has_sei, size_t
/// size)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_lifecycle_prepend_sei(got: bool, has_sei: bool, size: usize) -> bool {
    core::prepend_sei(got, has_sei, size)
}
