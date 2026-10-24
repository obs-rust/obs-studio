//! obs-core: the output frame copy plan and the scale effect choice
//! (`libobs/obs-video-copy-plan.c.h`). The core video thread makes one copy
//! plan per output frame of each mix (`set_gpu_converted_data()` or
//! `copy_rgbx_frame()`), one scale effect choice per frame of each mix whose
//! output is scaled, and one reuse match per earlier mix per frame
//! (`can_reuse_mix_texture()`): a few calls per frame at 60 fps. Each side
//! runs 1024 calls over fixed inputs and folds the results into a checksum
//! both must match.

use super::pair;

use core::ffi::{c_int, c_void};

use criterion::Criterion;
use obs_c_oracle::video_copy_plan::{
    oracle_obs_video_gpu_copy_plan, oracle_obs_video_mix_reuse_match,
    oracle_obs_video_rgbx_copy_plan, oracle_obs_video_scale_effect_choice,
};
use obs_core::ffi::video_copy_plan::{
    obs_video_gpu_copy_plan, obs_video_mix_reuse_key, obs_video_mix_reuse_match,
    obs_video_rgbx_copy_plan, obs_video_scale_effect_choice, obs_video_scale_input,
};
use std::hint::black_box;

const N: u32 = 1024;

/// A frame: format, width, height, input and output line sizes.
type Frame = (c_int, u32, u32, [u32; 8], [u32; 8]);

/// The GPU-converted formats (I420, NV12, I444, I010, P010, P216, P416)
/// and BGRA, which has no GPU copy.
const FORMATS: [c_int; 8] = [1, 2, 10, 17, 18, 22, 23, 7];

/// Frame `i`: a format, 1080p, 720p or 4K, tight or padded line sizes, and
/// NV12 / P010 as one texture now and then.
fn frames() -> Vec<Frame> {
    (0..N)
        .map(|i| {
            let format = FORMATS[(i % 8) as usize];
            let (w, h) = match i % 3 {
                0 => (1920, 1080),
                1 => (1280, 720),
                _ => (3840, 2160),
            };
            let row = if matches!(format, 17 | 18 | 22 | 23) {
                w * 2
            } else {
                w
            };
            let pad = if i.is_multiple_of(5) { 256 } else { 0 };
            let mut input = [0u32; 8];
            let mut output = [0u32; 8];
            input[..3].copy_from_slice(&[
                row + pad,
                if i.is_multiple_of(7) { 0 } else { row },
                row / 2,
            ]);
            output[..3].copy_from_slice(&[row, row, row / 2]);
            (format, w, h, input, output)
        })
        .collect()
}

fn scale_inputs() -> Vec<obs_video_scale_input> {
    (0..N)
        .map(|i| {
            let (tw, th) = match i % 4 {
                0 => (1280, 720),
                1 => (640, 360),
                2 => (1910, 1075),
                _ => (2560, 1440),
            };
            obs_video_scale_input {
                base_width: 1920,
                base_height: 1080,
                target_width: tw,
                target_height: th,
                output_width: tw,
                output_height: th,
                scale_type: (i % 6) as c_int,
                loaded: if i.is_multiple_of(9) { 0x15 } else { 0x1f },
            }
        })
        .collect()
}

fn rust_gpu(frames: &[Frame]) -> u64 {
    frames.iter().fold(0, |acc, f| {
        let (format, w, h, input, output) = black_box(*f);
        // SAFETY: two arrays of eight line sizes.
        let p = unsafe { obs_video_gpu_copy_plan(format, w, h, input.as_ptr(), output.as_ptr()) };
        p.copies.iter().take(p.count).fold(acc, |acc, c| {
            acc.wrapping_add(c.bytes as u64)
                .wrapping_add(u64::from(c.rows))
                .wrapping_add(u64::from(c.whole))
        })
    })
}

fn c_gpu(frames: &[Frame]) -> u64 {
    frames.iter().fold(0, |acc, f| {
        let (format, w, h, input, output) = black_box(*f);
        // SAFETY: as above.
        let p = unsafe {
            oracle_obs_video_gpu_copy_plan(format, w, h, input.as_ptr(), output.as_ptr())
        };
        p.copies.iter().take(p.count).fold(acc, |acc, c| {
            acc.wrapping_add(c.bytes as u64)
                .wrapping_add(u64::from(c.rows))
                .wrapping_add(u64::from(c.whole))
        })
    })
}

fn rust_scale(inputs: &[obs_video_scale_input]) -> u64 {
    inputs.iter().fold(0, |acc, s| {
        let s = black_box(*s);
        // SAFETY: a live input.
        acc + u64::from(unsafe { obs_video_scale_effect_choice(&raw const s) })
    })
}

fn c_scale(inputs: &[obs_video_scale_input]) -> u64 {
    inputs.iter().fold(0, |acc, s| {
        let s = black_box(*s);
        // SAFETY: as above; the oracle reads it as the C struct.
        acc + u64::from(unsafe {
            oracle_obs_video_scale_effect_choice((&raw const s).cast::<c_void>())
        })
    })
}

fn key(i: u32) -> obs_video_mix_reuse_key {
    obs_video_mix_reuse_key {
        view: 0x1000 * (1 + (i % 2) as usize),
        render_space: c_int::from(i.is_multiple_of(3)),
        base_width: 1920,
        base_height: if i.is_multiple_of(5) { 720 } else { 1080 },
        rendered: !i.is_multiple_of(4),
    }
}

fn rust_rgbx_reuse() -> u64 {
    (0..N).fold(0, |acc, i| {
        let i = black_box(i);
        let c = obs_video_rgbx_copy_plan(1920, 1080, 7680 + (i % 3) * 64, 7680);
        let (a, b) = (key(i), key(i / 2));
        // SAFETY: two live keys.
        let r = unsafe { obs_video_mix_reuse_match(&raw const a, &raw const b) };
        acc.wrapping_add(c.bytes as u64).wrapping_add(u64::from(r))
    })
}

fn c_rgbx_reuse() -> u64 {
    (0..N).fold(0, |acc, i| {
        let i = black_box(i);
        // SAFETY: plain values and two live keys.
        let (c, r) = unsafe {
            let (a, b) = (key(i), key(i / 2));
            (
                oracle_obs_video_rgbx_copy_plan(1920, 1080, 7680 + (i % 3) * 64, 7680),
                oracle_obs_video_mix_reuse_match(
                    (&raw const a).cast::<c_void>(),
                    (&raw const b).cast::<c_void>(),
                ),
            )
        };
        acc.wrapping_add(c.bytes as u64).wrapping_add(u64::from(r))
    })
}

pub fn benchmarks(criterion: &mut Criterion) {
    let f = frames();
    let s = scale_inputs();
    pair(
        criterion,
        "video_copy_plan/gpu_copy_plan/1024",
        || rust_gpu(&f),
        || c_gpu(&f),
    );
    pair(
        criterion,
        "video_copy_plan/scale_effect/1024",
        || rust_scale(&s),
        || c_scale(&s),
    );
    pair(
        criterion,
        "video_copy_plan/rgbx_and_reuse/1024",
        rust_rgbx_reuse,
        c_rgbx_reuse,
    );
}
