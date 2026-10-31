//! obs-media-io: the swscale scaler's decisions
//! (`libobs/media-io/video-scaler-plan.c.h`). `video_scaler_create()` asks
//! the plane heights once per scaler, and `video_scaler_scale()` asks the copy
//! of each plane for every frame it converts. Each side sets up an I420 1080p
//! scaler (an even height, where the port and the C agree) and plans 64
//! frames, every fourth with output line sizes that differ from the scaled
//! ones, folded into a checksum both sides must match. Nothing is allocated
//! with `bmalloc`.

use super::pair;

use core::ffi::c_int;
use criterion::Criterion;
use obs_c_oracle::video_scaler_plan::{
    oracle_video_scaler_plane_copy, oracle_video_scaler_plane_heights,
};
use obs_media_io::ffi::video_scaler_plan::{video_scaler_plane_copy, video_scaler_plane_heights};
use std::hint::black_box;

const FRAMES: usize = 64;
const HEIGHT: u32 = 1080;
const PLANES: [c_int; 4] = [0, 1, 2, 0];
const SHIFT: u8 = 1;
const SCALED: [usize; 3] = [1920, 960, 960];
const TIGHT: [usize; 3] = [1910, 955, 955];

fn mix(acc: u64, value: u64) -> u64 {
    acc.wrapping_mul(31).wrapping_add(value)
}

fn out_linesize(frame: usize, plane: usize) -> usize {
    if frame.is_multiple_of(4) {
        TIGHT[plane]
    } else {
        SCALED[plane]
    }
}

fn rust_frames() -> u64 {
    let mut heights = [0 as c_int; 4];
    let planes = PLANES;
    let shift = SHIFT;
    // SAFETY: live locals to read and write.
    unsafe {
        video_scaler_plane_heights(
            &raw mut heights,
            black_box(HEIGHT),
            &raw const planes,
            &raw const shift,
        );
    }
    let mut acc = 0_u64;
    for frame in 0..FRAMES {
        for plane in 0..3 {
            let mut size = 0_usize;
            let height = usize::try_from(heights[plane]).unwrap_or(0);
            // SAFETY: a live local to write.
            let whole = unsafe {
                video_scaler_plane_copy(
                    SCALED[plane],
                    black_box(out_linesize(frame, plane)),
                    height,
                    &raw mut size,
                )
            };
            acc = mix(acc, u64::from(whole) ^ size as u64);
        }
    }
    acc
}

fn c_frames() -> u64 {
    let mut heights = [0 as c_int; 4];
    let planes = PLANES;
    let shift = SHIFT;
    // SAFETY: live locals to read and write.
    unsafe {
        oracle_video_scaler_plane_heights(
            heights.as_mut_ptr(),
            black_box(HEIGHT),
            planes.as_ptr(),
            &raw const shift,
        );
    }
    let mut acc = 0_u64;
    for frame in 0..FRAMES {
        for plane in 0..3 {
            let mut size = 0_usize;
            let height = usize::try_from(heights[plane]).unwrap_or(0);
            // SAFETY: a live local to write.
            let whole = unsafe {
                oracle_video_scaler_plane_copy(
                    SCALED[plane],
                    black_box(out_linesize(frame, plane)),
                    height,
                    &raw mut size,
                )
            };
            acc = mix(acc, u64::from(whole) ^ size as u64);
        }
    }
    acc
}

pub fn benchmarks(criterion: &mut Criterion) {
    pair(criterion, "video_scaler_plan/frames/64", rust_frames, c_frames);
}
