//! #3079: the scene audio mix kernels of `libobs/obs-scene-audio-mix.c.h`
//! against the C, for the `audio_mix` category of #3009 and #3007. One call
//! adds one channel of an item into one channel of a scene mix, as
//! `scene_audio_render()` does for every audible item, mix and channel of
//! every tick: a whole tick of 1024 frames, and the 724 frames of an item
//! whose audio starts on frame 300. Before timing, both sides run once on
//! copies of the same buffers and must match bit for bit.

use std::hint::black_box;

use criterion::Criterion;
use obs_c_oracle::scene_audio_mix::{
    oracle_obs_scene_audio_mix_add, oracle_obs_scene_audio_mix_with_buf,
};
use obs_scene::ffi::scene_audio_mix::{obs_scene_audio_mix_add, obs_scene_audio_mix_with_buf};

use crate::measure_pair;

const FRAMES: usize = 1024;

/// A tone in [-1, 1) that differs per frame and per seed.
fn tone(seed: usize) -> Vec<f32> {
    (0..FRAMES)
        .map(|f| ((f * 37 + seed * 11) % 199) as f32 / 99.5 - 1.0)
        .collect()
}

type Kernel = unsafe extern "C" fn(*mut f32, *const f32, *const f32, usize, usize);
type AddKernel = unsafe extern "C" fn(*mut f32, *const f32, usize, usize);

fn with_buf(criterion: &mut Criterion, name: &str, pos: usize) {
    let input = tone(1);
    // A hide at frame 400 of the item's buffer.
    let gain: Vec<f32> = (0..FRAMES).map(|f| if f < 400 { 1.0 } else { 0.0 }).collect();
    let count = FRAMES - pos;
    let run = |f: Kernel, out: &mut Vec<f32>| {
        // SAFETY: `out` holds a tick, the window ends inside it, and the
        // input and gain hold `count` floats; nothing overlaps.
        unsafe {
            f(
                black_box(out.as_mut_ptr()),
                input.as_ptr(),
                gain.as_ptr(),
                pos,
                count,
            );
        }
    };
    let (mut r, mut c) = (tone(2), tone(2));
    run(obs_scene_audio_mix_with_buf, &mut r);
    run(oracle_obs_scene_audio_mix_with_buf, &mut c);
    assert!(
        r.iter().zip(&c).all(|(a, b)| a.to_bits() == b.to_bits()),
        "benchmark fixture differs: {name}"
    );
    measure_pair(
        criterion,
        name,
        || run(obs_scene_audio_mix_with_buf, &mut r),
        || run(oracle_obs_scene_audio_mix_with_buf, &mut c),
    );
}

fn add(criterion: &mut Criterion, name: &str, pos: usize) {
    let input = tone(3);
    let count = FRAMES - pos;
    let run = |f: AddKernel, out: &mut Vec<f32>| {
        // SAFETY: as in `with_buf`.
        unsafe { f(black_box(out.as_mut_ptr()), input.as_ptr(), pos, count) };
    };
    let (mut r, mut c) = (tone(4), tone(4));
    run(obs_scene_audio_mix_add, &mut r);
    run(oracle_obs_scene_audio_mix_add, &mut c);
    assert!(
        r.iter().zip(&c).all(|(a, b)| a.to_bits() == b.to_bits()),
        "benchmark fixture differs: {name}"
    );
    measure_pair(
        criterion,
        name,
        || run(obs_scene_audio_mix_add, &mut r),
        || run(oracle_obs_scene_audio_mix_add, &mut c),
    );
}

pub fn benchmarks(criterion: &mut Criterion) {
    with_buf(criterion, "audio_mix/scene_mix_with_buf/whole_tick", 0);
    with_buf(criterion, "audio_mix/scene_mix_with_buf/from_frame_300", 300);
    add(criterion, "audio_mix/scene_mix_add/whole_tick", 0);
    add(criterion, "audio_mix/scene_mix_add/from_frame_300", 300);
}
