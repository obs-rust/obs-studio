//! Safe core of `libobs/obs-scene-audio-mix.c.h`: the two mix kernels of
//! `scene_audio_render()` in `obs-scene.c.h`.
//!
//! Every audio tick, a scene adds each audible item's audio into each of
//! its mixes, one channel at a time, from the frame the item's audio starts
//! on (the window of [`crate::audio_window::frames`]). `obs-scene.c.h`
//! keeps the item loop, `audio_lock`, `apply_scene_item_volume()`,
//! `obs_source_get_audio_mix()`, the window and mix decisions and the mix
//! and channel loops, and passes each channel's buffers here, already cut to
//! the window:
//! - [`mix_with_buf`]: the item's samples scaled by its 0/1 gain buffer, for
//!   an item whose visibility change falls in the tick;
//! - [`mix_add`]: the item's samples as they are.
//!
//! Both are a separate IEEE multiply and add per sample, in the C's order,
//! and never fused: the C oracle is built without contraction. The loops
//! run over slices, which cannot overlap, so the compiler vectorizes them
//! (the C's pointers may alias, and GCC leaves its loops scalar).
//!
//! No intentional differences.

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

/// `obs_scene_audio_mix_with_buf()` (`mix_audio_with_buf()`): `out[i] +=
/// input[i] * gain[i]` for every `i` of the shortest of the three slices.
/// The caller cuts `out` to `pos .. pos + count` and `input` and `gain` to
/// `count`.
pub fn mix_with_buf(out: &mut [f32], input: &[f32], gain: &[f32]) {
    let _ = (out, input, gain);
    todo!()
}

/// `obs_scene_audio_mix_add()` (`scene_mix_audio()`): `out[i] += input[i]`
/// for every `i` of the shorter slice. The caller cuts `out` to
/// `pos .. pos + count` and `input` to `count`.
pub fn mix_add(out: &mut [f32], input: &[f32]) {
    let _ = (out, input);
    todo!()
}
