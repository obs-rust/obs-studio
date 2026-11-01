//! obs-graphics: the file-name check and the pixel-format pick of the image
//! loader's Windows host (`libobs/graphics/graphics-ffmpeg-host-wic-checks.c.h`).
//! Both run once per image FFmpeg cannot load. Each side checks the same
//! 64 names of 4 to 40 bytes and picks for every pair of flags, and both
//! counts must match. Nothing is allocated while it runs.

use super::pair;

use criterion::Criterion;
use obs_c_oracle::image_wic_checks::{
    oracle_gs_image_host_fallback_refuses, oracle_gs_image_wic_kind,
};
use obs_graphics::ffi::image_pq_scrgb::{gs_image_host_fallback_refuses, gs_image_wic_kind};
use std::ffi::CString;
use std::hint::black_box;

fn names() -> Vec<CString> {
    let stems = [".jxr", ".JXR", ".png", "a.jx", "a.jxr", "shot.png", "C:\\images\\hdr.jxr"];
    (0..64)
        .map(|i| {
            let stem = stems[i % stems.len()];
            let pad = "x".repeat((i * 7) % 23);
            let name = if pad.len() + stem.len() > 40 || i % 3 == 0 {
                stem.to_owned()
            } else {
                format!("{pad}{stem}")
            };
            CString::new(name).unwrap()
        })
        .collect()
}

pub fn benchmarks(criterion: &mut Criterion) {
    let names = names();
    let flags = [(false, false), (true, false), (false, true), (true, true)];
    pair(
        criterion,
        "image_wic_checks/64_names_4_picks",
        || {
            let mut count = 0_i64;
            for name in &names {
                let len = name.as_bytes().len();
                // SAFETY: `len` bytes and a NUL at `name`, live for the call.
                count += i64::from(unsafe { gs_image_host_fallback_refuses(black_box(name.as_ptr()), len) });
            }
            for (scrgb, pq10) in flags {
                count += i64::from(gs_image_wic_kind(black_box(scrgb), pq10));
            }
            count
        },
        || {
            let mut count = 0_i64;
            for name in &names {
                let len = name.as_bytes().len();
                // SAFETY: as above; every name has at least 4 bytes, so the C
                // reads from `name + len - 4` to its NUL.
                count += i64::from(unsafe {
                    oracle_gs_image_host_fallback_refuses(black_box(name.as_ptr()), len)
                });
            }
            for (scrgb, pq10) in flags {
                // SAFETY: plain values in and out.
                count += i64::from(unsafe { oracle_gs_image_wic_kind(black_box(scrgb), pq10) });
            }
            count
        },
    );
}
