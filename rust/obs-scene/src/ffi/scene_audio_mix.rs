//! C ABI for `libobs/obs-scene-audio-mix.c.h`, the two mix kernels of
//! `scene_audio_render()` in `obs-scene.c`, declared in
//! `libobs/obs-scene-audio-mix.h` without `EXPORT`.
//!
//! `obs-scene.c.h` (still C) keeps the item, mix and channel loops and
//! passes one channel's buffers at a time: the scene's mix channel, the
//! item's channel, its gain buffer, the window's first frame and its length.
//! These cut them to the window and call the safe core; they allocate
//! nothing and call nothing in libobs. Both stay hidden (`rust-hidden.txt`).
//!
//! No intentional differences.

use core::slice;

use crate::audio_mix;

/// `void obs_scene_audio_mix_with_buf(float *out, const float *in, const
/// float *buf, size_t pos, size_t count)`.
///
/// # Safety
///
/// As the C: `out` is writable from `pos` for `count` floats, and `input`
/// and `buf` are readable for `count` floats, none of them overlapping
/// `out`'s window. `scene_audio_render()` passes a scene mix channel of
/// `AUDIO_OUTPUT_FRAMES`, the item's channel and its gain buffer, and a
/// window inside the tick.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_scene_audio_mix_with_buf(
    out: *mut f32,
    input: *const f32,
    buf: *const f32,
    pos: usize,
    count: usize,
) {
    if count == 0 {
        return;
    }
    // SAFETY: the caller's contract.
    let (out, input, buf) = unsafe {
        (
            slice::from_raw_parts_mut(out.add(pos), count),
            slice::from_raw_parts(input, count),
            slice::from_raw_parts(buf, count),
        )
    };
    audio_mix::mix_with_buf(out, input, buf);
}

/// `void obs_scene_audio_mix_add(float *out, const float *in, size_t pos,
/// size_t count)`.
///
/// # Safety
///
/// As the C: `out` is writable from `pos` for `count` floats and `input`
/// readable for `count` floats, not overlapping `out`'s window.
/// `scene_audio_render()` passes a scene mix channel of
/// `AUDIO_OUTPUT_FRAMES`, the item's channel and a window inside the tick.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_scene_audio_mix_add(
    out: *mut f32,
    input: *const f32,
    pos: usize,
    count: usize,
) {
    if count == 0 {
        return;
    }
    // SAFETY: the caller's contract.
    let (out, input) = unsafe {
        (
            slice::from_raw_parts_mut(out.add(pos), count),
            slice::from_raw_parts(input, count),
        )
    };
    audio_mix::mix_add(out, input);
}
