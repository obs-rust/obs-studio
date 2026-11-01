//! obs-graphics: the straight row copy of `graphics-ffmpeg.c`'s pixel route
//! (`libobs/graphics/graphics-ffmpeg-route.c.h`). Every RGBA or BGRA image an
//! image source loads with straight alpha (and every BGR0 one) is copied
//! through it once, after it is decoded: here a 4000 x 3000 photo, from a
//! tight frame (the C's one `memcpy`) and from one padded to 64 bytes a
//! line, as FFmpeg's frames often are. Each side copies the same frame into
//! a buffer allocated once, and both outputs must match.

use super::pair;

use criterion::Criterion;
use obs_c_oracle::image_route::oracle_gs_ffmpeg_image_copy_rows;
use obs_graphics::ffi::image_route::gs_ffmpeg_image_copy_rows;
use std::cell::RefCell;
use std::hint::black_box;

const W: usize = 4000;
const H: usize = 3000;

fn frame(linesize: usize) -> Vec<u8> {
    let mut x = 0x9e37_79b9_u32;
    (0..linesize * H)
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
    let rust_out = RefCell::new(vec![0_u8; W * H * 4]);
    let c_out = RefCell::new(vec![0_u8; W * H * 4]);
    for (name, linesize) in [("tight", W * 4), ("padded", (W * 4).div_ceil(64) * 64 + 64)] {
        let src = frame(linesize);
        pair(
            criterion,
            &format!("image_route/copy_rows_{name}/4000x3000"),
            || {
                let mut out = rust_out.borrow_mut();
                // SAFETY: W * H * 4 writable bytes, linesize * H readable,
                // not overlapping.
                unsafe {
                    gs_ffmpeg_image_copy_rows(out.as_mut_ptr(), black_box(src.as_ptr()), W, H, linesize);
                }
                sum(&out)
            },
            || {
                let mut out = c_out.borrow_mut();
                // SAFETY: as above; the C reads and writes exactly that much.
                unsafe {
                    oracle_gs_ffmpeg_image_copy_rows(out.as_mut_ptr(), black_box(src.as_ptr()), W, H, linesize);
                }
                sum(&out)
            },
        );
    }
}
