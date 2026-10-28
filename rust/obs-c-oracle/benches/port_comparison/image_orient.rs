//! obs-graphics: the image orientation of `graphics-ffmpeg.c`
//! (`libobs/graphics/graphics-ffmpeg-orient.c.h`). Every image loaded with an
//! EXIF orientation of 2 to 8 is turned once, pixel by pixel, after it is
//! decoded: a phone photo taken upright is a 4000 x 3000 image with
//! orientation 6 (rotate 90 degrees clockwise). Each side turns the same
//! image into a buffer allocated once, and both outputs must match.

use super::pair;

use criterion::Criterion;
use obs_c_oracle::image_orient::oracle_gs_ffmpeg_image_orient_copy;
use obs_graphics::ffi::image_orient::gs_ffmpeg_image_orient_copy;
use std::cell::RefCell;
use std::hint::black_box;

const W: usize = 4000;
const H: usize = 3000;
const ORIENT: i32 = 6;

fn photo() -> Vec<u8> {
    let mut x = 0x9e37_79b9_u32;
    (0..W * H * 4)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            x.to_le_bytes()[0]
        })
        .collect()
}

/// A checksum of the turned image, so both sides must produce it.
fn sum(out: &[u8]) -> u64 {
    out.chunks(4096).fold(0_u64, |acc, c| {
        acc.wrapping_mul(31)
            .wrapping_add(c.iter().map(|&b| u64::from(b)).sum::<u64>())
    })
}

pub fn benchmarks(criterion: &mut Criterion) {
    let src = photo();
    let rust_out = RefCell::new(vec![0_u8; src.len()]);
    let c_out = RefCell::new(vec![0_u8; src.len()]);
    pair(
        criterion,
        "image_orient/rotate90/4000x3000",
        || {
            let mut out = rust_out.borrow_mut();
            // SAFETY: two buffers of W * H * 4 bytes, not overlapping.
            unsafe {
                gs_ffmpeg_image_orient_copy(out.as_mut_ptr(), black_box(src.as_ptr()), W, H, ORIENT);
            }
            sum(&out)
        },
        || {
            let mut out = c_out.borrow_mut();
            // SAFETY: as above; the C reads and writes exactly that much.
            unsafe {
                oracle_gs_ffmpeg_image_orient_copy(out.as_mut_ptr(), black_box(src.as_ptr()), W, H, ORIENT);
            }
            sum(&out)
        },
    );
}
