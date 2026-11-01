//! obs-graphics: the 16-bit RGBA rows of `graphics-ffmpeg.c`
//! (`libobs/graphics/graphics-ffmpeg-rgba64.c.h`). Every 16-bit PNG an image
//! source loads goes through them once, after it is decoded: here a 1920 x
//! 1080 frame with noise in every channel, premultiplied in sRGB (two `powf`
//! calls per colour channel; the image source's "linear alpha"), premultiplied
//! (its default) and straight. Each side converts the same frame into a
//! buffer allocated once, and both outputs must match.

use super::pair;

use criterion::Criterion;
use obs_c_oracle::image_rgba64::oracle_gs_ffmpeg_image_rgba64be_rows;
use obs_graphics::ffi::image_rgba64::gs_ffmpeg_image_rgba64be_rows;
use std::cell::RefCell;
use std::hint::black_box;

const W: usize = 1920;
const H: usize = 1080;
const LINESIZE: usize = W * 8;

fn frame() -> Vec<u8> {
    let mut x = 0x9e37_79b9_u32;
    (0..LINESIZE * H)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            x.to_le_bytes()[0]
        })
        .collect()
}

/// A checksum of the output, so both sides must produce it.
fn sum(out: &[u8]) -> u64 {
    out.chunks(4096).fold(0_u64, |acc, c| {
        acc.wrapping_mul(31)
            .wrapping_add(c.iter().map(|&b| u64::from(b)).sum::<u64>())
    })
}

pub fn benchmarks(criterion: &mut Criterion) {
    let src = frame();
    let rust_out = RefCell::new(vec![0_u8; W * H * 4]);
    let c_out = RefCell::new(vec![0_u8; W * H * 4]);
    let (cx, cy) = (i32::try_from(W).unwrap(), i32::try_from(H).unwrap());
    for (name, mode) in [("premultiply_srgb", 1), ("premultiply", 2), ("straight", 0)] {
        pair(
            criterion,
            &format!("image_rgba64/{name}/1920x1080"),
            || {
                let mut out = rust_out.borrow_mut();
                // SAFETY: W * H * 4 writable bytes, LINESIZE * H readable,
                // not overlapping.
                unsafe {
                    gs_ffmpeg_image_rgba64be_rows(out.as_mut_ptr(), black_box(src.as_ptr()), cx, cy, LINESIZE, mode);
                }
                sum(&out)
            },
            || {
                let mut out = c_out.borrow_mut();
                // SAFETY: as above; the C reads and writes exactly that much.
                unsafe {
                    oracle_gs_ffmpeg_image_rgba64be_rows(
                        out.as_mut_ptr(),
                        black_box(src.as_ptr()),
                        cx,
                        cy,
                        LINESIZE,
                        mode,
                    );
                }
                sum(&out)
            },
        );
    }
}
