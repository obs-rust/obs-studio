//! obs-graphics: the JPEG XR PQ-to-scRGB conversion of the image loader's
//! Windows host (`libobs/graphics/graphics-ffmpeg-host-pq.c.h`). An HDR10
//! JPEG XR image, such as a Windows HDR screenshot, is converted once, texel
//! by texel, after WIC decodes it. Each side converts the same 1920 x 1080
//! image into a buffer allocated once, and both outputs must match.

use super::pair;

use criterion::Criterion;
use obs_c_oracle::image_pq_scrgb::oracle_gs_image_pq10_to_scrgb;
use obs_graphics::ffi::image_pq_scrgb::gs_image_pq10_to_scrgb;
use std::cell::RefCell;
use std::hint::black_box;

const TEXELS: usize = 1920 * 1080;

fn screenshot() -> Vec<u8> {
    let mut x = 0x9e37_79b9_u32;
    (0..TEXELS)
        .flat_map(|_| {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            (x & 0x3fff_ffff).to_ne_bytes()
        })
        .collect()
}

/// A checksum of the converted image, so both sides must produce it.
fn sum(out: &[u8]) -> u64 {
    out.chunks(4096).fold(0_u64, |acc, c| {
        acc.wrapping_mul(31)
            .wrapping_add(c.iter().map(|&b| u64::from(b)).sum::<u64>())
    })
}

pub fn benchmarks(criterion: &mut Criterion) {
    let src = screenshot();
    let dst_len = TEXELS.checked_mul(8).unwrap();
    let dst_size = u32::try_from(dst_len).unwrap();
    let src_size = u32::try_from(src.len()).unwrap();
    let rust_out = RefCell::new(vec![0_u8; dst_len]);
    let c_out = RefCell::new(vec![0_u8; dst_len]);
    pair(
        criterion,
        "image_pq_scrgb/convert/1920x1080",
        || {
            let mut out = rust_out.borrow_mut();
            // SAFETY: dst_size writable and src_size readable bytes, not
            // overlapping.
            unsafe {
                gs_image_pq10_to_scrgb(out.as_mut_ptr(), dst_size, black_box(src.as_ptr()), src_size);
            }
            sum(&out)
        },
        || {
            let mut out = c_out.borrow_mut();
            // SAFETY: as above; dst holds 8 bytes per source texel, which is
            // what the C writes.
            unsafe {
                oracle_gs_image_pq10_to_scrgb(out.as_mut_ptr(), dst_size, black_box(src.as_ptr()), src_size);
            }
            sum(&out)
        },
    );
}
