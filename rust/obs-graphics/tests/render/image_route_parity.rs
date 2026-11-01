//! Tier 3 parity: the shims of `obs_graphics::ffi::image_route` (and the
//! safe core under them) against the original
//! `libobs/graphics/graphics-ffmpeg-route.c.h`, compiled as
//! `oracle_gs_ffmpeg_image_*` (obs-c-oracle, oracle/image_route.c).
//!
//! The plan and the texture format: every pixel format value from -2 to
//! 320 (past `AV_PIX_FMT_NB`, 267 in FFmpeg 9) and the ends of `int`, in
//! every alpha mode from -1 to 4 and at the ends of `int`, on the shim, the
//! C and the core; the plan field by field. The layout of
//! `struct gs_ffmpeg_image_plan` against the C's.
//!
//! The copy: every width from 1 to 12 at 0 to 5 rows with tight, padded,
//! 64-aligned and short line sizes, and a proptest over images up to
//! 48 x 24 with any line size from 0 to `cx * 4 + 64`. Each runs on the
//! shim, the C and the core, each into an output filled with the same
//! canary, compared byte for byte, so a byte one side leaves unwritten
//! counts too. The core also copies into an uninitialized destination, as
//! the shim's `bmalloc()` buffer is, against the C.
//!
//! No intentional difference. Left out, as the C reads or writes out of
//! bounds there and `graphics-ffmpeg.c` never passes them: a null pointer
//! and a size that overflows, where the copy shim writes nothing (checked
//! here on the shim alone). `test_graphics_ffmpeg_route_steps` runs the C
//! test against the C; the real-libobs image tests run against the hybrid
//! libobs, where these shims replace the C.

use core::mem::{MaybeUninit, align_of, offset_of, size_of};

use obs_c_oracle::image_route::{
    OraclePlan, oracle_gs_ffmpeg_image_color_format, oracle_gs_ffmpeg_image_copy_rows,
    oracle_gs_ffmpeg_image_plan, oracle_image_route_plan_layout,
};
use obs_graphics::ffi::image_route::{
    gs_ffmpeg_image_color_format, gs_ffmpeg_image_copy_rows, gs_ffmpeg_image_plan,
};
use obs_graphics::image_route::{color_format, copy_rows, copy_src_len, plan};
use proptest::prelude::*;

const CANARY: u8 = 0xa5;

fn formats() -> impl Iterator<Item = i32> {
    (-2..=320).chain([i32::MIN, i32::MIN + 1, i32::MAX - 1, i32::MAX])
}

fn alpha_modes() -> impl Iterator<Item = i32> {
    (-1..=4).chain([i32::MIN, i32::MAX])
}

#[test]
fn plan_layout_matches_c() {
    let mut out = [0usize; 8];
    // SAFETY: `out` has room for the 7 values the oracle writes.
    let n = unsafe { oracle_image_route_plan_layout(out.as_mut_ptr()) };
    type P = obs_graphics::ffi::image_route::gs_ffmpeg_image_plan;
    let rust = [
        size_of::<P>(),
        align_of::<P>(),
        offset_of!(P, route),
        offset_of!(P, premultiply),
        offset_of!(P, sws_format),
        offset_of!(P, full_range),
        offset_of!(P, out_format),
    ];
    assert_eq!(&out[..n], &rust[..]);
    assert_eq!(size_of::<OraclePlan>(), size_of::<P>());
}

#[test]
fn plan_matches_c_for_every_format_and_mode() {
    for fmt in formats() {
        for mode in alpha_modes() {
            // SAFETY: the oracle takes two ints and touches no memory.
            let c = unsafe { oracle_gs_ffmpeg_image_plan(fmt, mode) };
            let shim = gs_ffmpeg_image_plan(fmt, mode);
            let core = plan(fmt, mode);
            let what = format!("format {fmt}, alpha mode {mode}");
            assert_eq!(shim.route, c.route, "{what}: route");
            assert_eq!(shim.premultiply, c.premultiply, "{what}: premultiply");
            assert_eq!(shim.sws_format, c.sws_format, "{what}: swscale format");
            assert_eq!(shim.full_range, c.full_range, "{what}: full range");
            assert_eq!(shim.out_format, c.out_format, "{what}: output format");
            assert_eq!(core.route.code(), c.route, "{what}: core route");
            assert_eq!(
                core.premultiply.code(),
                c.premultiply,
                "{what}: core premultiply"
            );
            assert_eq!(
                (core.sws_format, core.full_range, core.out_format),
                (c.sws_format, c.full_range, c.out_format),
                "{what}: core formats"
            );
        }
    }
}

#[test]
fn color_format_matches_c_for_every_format() {
    for fmt in formats() {
        // SAFETY: the oracle takes an int and touches no memory.
        let c = unsafe { oracle_gs_ffmpeg_image_color_format(fmt) };
        assert_eq!(gs_ffmpeg_image_color_format(fmt), c, "format {fmt}");
        assert_eq!(color_format(fmt), c, "format {fmt}: core");
    }
}

fn same(what: &str, got: &[u8], want: &[u8]) {
    if let Some(i) = got.iter().zip(want).position(|(a, b)| a != b) {
        panic!("{what}: byte {i} is {}, expected {}", got[i], want[i]);
    }
    assert_eq!(got.len(), want.len(), "{what}: length");
}

/// One copy on all three sides.
fn check_copy(what: &str, src: &[u8], cx: usize, cy: usize, linesize: usize) {
    let len = cx * cy * 4;
    assert!(
        src.len() >= copy_src_len(cx, cy, linesize).unwrap(),
        "{what}: source too short"
    );
    let mut shim = vec![CANARY; len];
    let mut c = vec![CANARY; len];
    let mut core = vec![CANARY; len];
    // SAFETY: `len` writable bytes in each output and at least
    // `copy_src_len()` readable bytes in `src`, which is all either side
    // touches.
    unsafe {
        gs_ffmpeg_image_copy_rows(shim.as_mut_ptr(), src.as_ptr(), cx, cy, linesize);
        oracle_gs_ffmpeg_image_copy_rows(c.as_mut_ptr(), src.as_ptr(), cx, cy, linesize);
    }
    copy_rows(&mut core, src, cx, cy, linesize);
    same(&format!("{what}: shim vs C"), &shim, &c);
    same(&format!("{what}: core vs C"), &core, &c);
}

fn pattern(len: usize, seed: u64) -> Vec<u8> {
    (0..len)
        .map(|i| (seed.wrapping_mul(i as u64 + 1).wrapping_add(i as u64) >> 11) as u8)
        .collect()
}

#[test]
fn copy_matches_c_on_every_small_layout() {
    for cx in 1..=12usize {
        let row = cx * 4;
        let mut lines = vec![row, row + 1, row + 4, row + 13, row.div_ceil(64) * 64 + 64];
        lines.extend(
            [1, 3, row - 1, row / 2]
                .into_iter()
                .filter(|&l| l > 0 && l < row),
        );
        for cy in 0..=5usize {
            for &ls in &lines {
                let src = pattern(ls * cy + 8, (cx * 31 + cy * 7 + ls) as u64);
                check_copy(&format!("{cx} x {cy}, line size {ls}"), &src, cx, cy, ls);
            }
        }
    }
}

/// The core into an uninitialized destination (the shim's `bmalloc()`
/// buffer), tight and padded: every byte the C writes, the same. Here, not
/// in Tier 1, because reading the destination back takes `unsafe`.
#[test]
fn copy_core_into_uninit_matches_c() {
    for (cx, cy, ls) in [(4usize, 3usize, 16usize), (5, 4, 32), (7, 2, 28)] {
        let src = pattern(ls * cy, (cx + cy + ls) as u64);
        let mut c = vec![CANARY; cx * cy * 4];
        // SAFETY: `cx * cy * 4` writable bytes and `ls * cy` readable ones,
        // at least what the C touches for a line size of `cx * 4` or more.
        unsafe { oracle_gs_ffmpeg_image_copy_rows(c.as_mut_ptr(), src.as_ptr(), cx, cy, ls) };
        let mut dst = vec![MaybeUninit::<u8>::uninit(); cx * cy * 4];
        assert_eq!(copy_rows(&mut dst, &src, cx, cy, ls), cy);
        // SAFETY: with a line size of at least `cx * 4` every row is copied
        // whole, so every byte of `dst` was written.
        let dst: Vec<u8> = dst.iter().map(|b| unsafe { b.assume_init() }).collect();
        same(
            &format!("{cx} x {cy}, line size {ls}: uninit core vs C"),
            &dst,
            &c,
        );
    }
}

/// A null pointer or an overflowing size: the shim writes nothing.
#[test]
fn copy_shim_outside_the_contract_writes_nothing() {
    let src = [1u8; 64];
    let mut dst = [CANARY; 64];
    // SAFETY: each call writes nothing (checked below), so no buffer is
    // touched past its end.
    unsafe {
        gs_ffmpeg_image_copy_rows(dst.as_mut_ptr(), core::ptr::null(), 4, 4, 16);
        gs_ffmpeg_image_copy_rows(core::ptr::null_mut(), src.as_ptr(), 4, 4, 16);
        gs_ffmpeg_image_copy_rows(dst.as_mut_ptr(), src.as_ptr(), usize::MAX, 2, 16);
        gs_ffmpeg_image_copy_rows(dst.as_mut_ptr(), src.as_ptr(), 2, usize::MAX, 16);
        gs_ffmpeg_image_copy_rows(dst.as_mut_ptr(), src.as_ptr(), 4, 4, usize::MAX);
    }
    assert!(dst.iter().all(|&b| b == CANARY));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(512)))]

    #[test]
    fn copy_matches_c(
        cx in 1usize..=48,
        cy in 0usize..=24,
        extra in 0usize..=128,
        seed in any::<u64>(),
    ) {
        // Line sizes from 0 to cx * 4 + 64: short, tight and padded.
        let ls = (cx * 4 + extra).saturating_sub(64);
        let src = pattern(ls * cy + 8, seed);
        check_copy(&format!("{cx} x {cy}, line size {ls}"), &src, cx, cy, ls);
    }
}
