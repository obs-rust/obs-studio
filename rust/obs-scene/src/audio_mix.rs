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
//! (the C's pointers may alias, and GCC at `-O2`, as libobs ships, leaves
//! its loops scalar). The loop form of [`mix_add`] depends on the target,
//! because LLVM unrolls the two forms differently there: 4-sample groups on
//! x86_64 (the zip loop was 18 % slower than GCC's vectorized C at `-O3`,
//! the grouped one 2 %), a plain zip loop on aarch64 (the grouped one was
//! 46 % slower than clang's C, the zip loop level with it); the choice is
//! `platform::GROUPED_AUDIO_LOOPS`. [`mix_with_buf`] is a zip loop on every
//! target: grouped, LLVM vectorizes across the groups with gathers, 4 times
//! slower on x86_64.
//!
//! No intentional differences.

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

/// `obs_scene_audio_mix_with_buf()` (`mix_audio_with_buf()`): `out[i] +=
/// input[i] * gain[i]` for every `i` of the shortest of the three slices.
/// The caller cuts `out` to `pos .. pos + count` and `input` and `gain` to
/// `count`.
pub fn mix_with_buf(out: &mut [f32], input: &[f32], gain: &[f32]) {
    for ((sample, &value), &g) in out.iter_mut().zip(input).zip(gain) {
        *sample += value * g;
    }
}

/// `obs_scene_audio_mix_add()` (`scene_mix_audio()`): `out[i] += input[i]`
/// for every `i` of the shorter slice. The caller cuts `out` to
/// `pos .. pos + count` and `input` to `count`. [`mix_add_grouped`] on
/// x86_64, [`mix_add_zipped`] elsewhere (see the module docs).
pub fn mix_add(out: &mut [f32], input: &[f32]) {
    if crate::platform::GROUPED_AUDIO_LOOPS {
        mix_add_grouped(out, input);
    } else {
        mix_add_zipped(out, input);
    }
}

/// [`mix_add`] as 4-sample groups, then the rest. Same result as
/// [`mix_add_zipped`] on every input; public only for the tests, which run
/// both forms on every host.
#[doc(hidden)]
pub fn mix_add_grouped(out: &mut [f32], input: &[f32]) {
    let n = out.len().min(input.len());
    let (Some(out), Some(input)) = (out.get_mut(..n), input.get(..n)) else {
        return;
    };
    let (out_groups, out_rest) = out.as_chunks_mut::<4>();
    let (input_groups, input_rest) = input.as_chunks::<4>();
    for (samples, values) in out_groups.iter_mut().zip(input_groups) {
        for (sample, &value) in samples.iter_mut().zip(values) {
            *sample += value;
        }
    }
    for (sample, &value) in out_rest.iter_mut().zip(input_rest) {
        *sample += value;
    }
}

/// [`mix_add`] as a plain zip loop. Same result as [`mix_add_grouped`] on
/// every input; public only for the tests.
#[doc(hidden)]
pub fn mix_add_zipped(out: &mut [f32], input: &[f32]) {
    for (sample, &value) in out.iter_mut().zip(input) {
        *sample += value;
    }
}
