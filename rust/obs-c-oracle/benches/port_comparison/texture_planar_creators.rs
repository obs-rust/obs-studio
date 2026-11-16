//! obs-graphics: `gs_texture_create_nv12` and `gs_texture_create_p010`
//! (`libobs/graphics/texture-planar-creators.c.h`), over the oracle's
//! stand-in context and `gs_*` stand-in with recording off. Each side runs
//! both entry points through the fallback (no device export), the device
//! export and the odd-size refusal, and the success counts must match. The
//! stand-ins hand out static handles, so nothing is allocated.

use super::pair;

use core::ffi::c_void;
use core::ptr;
use criterion::Criterion;
use obs_c_oracle::texture_planar_creators::{
    oracle_gs_texture_create_nv12, oracle_gs_texture_create_p010, oracle_tp_reset,
};
use obs_c_oracle::texture_render::{oracle_texrender_device_quiet, oracle_texrender_device_reset};
use obs_graphics::ffi::texture_planar_creators::{gs_texture_create_nv12, gs_texture_create_p010};
use std::hint::black_box;

type Entry = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void, u32, u32, u32) -> bool;

const SIZES: [(u32, u32); 3] = [(1920, 1080), (1280, 720), (1921, 1080)];

fn run(nv12: Entry, p010: Entry) -> i64 {
    let mut count = 0_i64;
    for export in [false, true] {
        // SAFETY: resets the stand-ins; recording stays off.
        unsafe {
            oracle_texrender_device_reset();
            oracle_texrender_device_quiet(true);
            oracle_tp_reset(true, export, export, true, true);
        }
        for (width, height) in SIZES {
            for entry in [nv12, p010] {
                let (mut y, mut uv) = (ptr::null_mut(), ptr::null_mut());
                // SAFETY: both out-pointers are live locals.
                count += i64::from(unsafe {
                    entry(&raw mut y, &raw mut uv, black_box(width), height, 0)
                });
            }
        }
    }
    // SAFETY: as above.
    unsafe { oracle_texrender_device_quiet(false) };
    count
}

pub fn benchmarks(criterion: &mut Criterion) {
    pair(
        criterion,
        "texture_planar_creators/12_creates",
        || run(gs_texture_create_nv12, gs_texture_create_p010),
        || run(oracle_gs_texture_create_nv12, oracle_gs_texture_create_p010),
    );
}
