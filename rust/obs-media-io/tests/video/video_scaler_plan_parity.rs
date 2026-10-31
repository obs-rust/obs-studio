//! Tier 3: `libobs/media-io/video-scaler-plan.c.h` against its port.
//!
//! The oracle (`rust/obs-c-oracle/oracle/video_scaler_plan.c`) is the
//! unchanged C file with its five functions renamed `oracle_*`. Each case
//! runs the oracle, the C ABI shims (`ffi::video_scaler_plan`, what the
//! hybrid libobs links) and the safe core on the same inputs and compares
//! everything they return and write:
//!
//! - the constants and the enum values the C maps;
//! - the three maps over every value from -3 to 12, the ends of `int` and,
//!   by proptest, every `int`;
//! - the plane heights over every height from 0 to 4100, shifts 0 to 2 and
//!   every pattern of component planes (0 to 3 each), plus heights at the
//!   ends of `uint32_t`; the heights a plane the format lacks keeps, and the
//!   chroma shift passed as a null pointer to both sides whenever the format
//!   has no plane 1 or 2, so a read the C did not make would crash the shim's
//!   side;
//! - the plane copy over line sizes up to 16384 and heights up to 4100 by
//!   proptest, and the ends of `size_t`.
//!
//! Intentional difference (the only one): a shifted chroma plane's height
//! rounds up in the port, where the C rounds down. Where the height is not a
//! multiple of `2^shift`, planes 1 and 2 are compared with the C's height
//! plus one (`expected_port`); every other plane, height and input is
//! compared with the C unchanged. Left out, as the C leaves them undefined
//! and no libavutil descriptor gives them: component planes outside 0 to 3
//! and shifts of 32 or more (Tier 1 pins what the core does). A null pointer
//! for the heights, the component planes or the size, which the C
//! dereferences, is checked on the Rust side only: the shims write nothing.
//!
//! Reminder: the mutation check (break the core, see this file fail) is done
//! once per port, not on every change.

use core::ffi::c_int;

use obs_c_oracle::video_scaler_plan as c;
use obs_media_io::ffi::video_scaler_plan as shim;
use obs_media_io::video_io::{
    VideoColorspace, VideoRangeType, VideoScaleType, video_colorspace_to_c, video_range_type_to_c,
    video_scale_type_to_c,
};
use obs_media_io::video_scaler_plan::{self as core_, PlaneCopy, sws};
use proptest::prelude::*;

const UNSET: c_int = -7;

fn oracle_int(name: &str) -> i64 {
    let which = c::INT_NAMES
        .iter()
        .position(|&n| n == name)
        .unwrap_or_else(|| panic!("no oracle value {name}"));
    // SAFETY: an index into the oracle's table.
    unsafe { c::oracle_video_scaler_plan_int(which) }
}

fn maps_agree(v: c_int) {
    // SAFETY: the oracle reads only its argument.
    let (flags, cs, range) = unsafe {
        (
            c::oracle_video_scaler_sws_flags(v),
            c::oracle_video_scaler_sws_colorspace(v),
            c::oracle_video_scaler_sws_range(v),
        )
    };
    assert_eq!(shim::video_scaler_sws_flags(v), flags, "flags {v}");
    assert_eq!(core_::sws_flags(v), flags, "flags {v}");
    assert_eq!(shim::video_scaler_sws_colorspace(v), cs, "colorspace {v}");
    assert_eq!(core_::sws_colorspace(v), cs, "colorspace {v}");
    assert_eq!(shim::video_scaler_sws_range(v), range, "range {v}");
    assert_eq!(core_::sws_range(v), range, "range {v}");
}

fn has_chroma(planes: [c_int; 4]) -> bool {
    planes.iter().any(|&p| p == 1 || p == 2)
}

fn oracle_heights(height: u32, planes: [c_int; 4], shift: u8) -> [c_int; 4] {
    let mut heights = [UNSET; 4];
    let shift_ptr: *const u8 = if has_chroma(planes) {
        &raw const shift
    } else {
        core::ptr::null()
    };
    // SAFETY: 4 writable ints, 4 readable ints, and the shift (or null for
    // a format without plane 1 or 2, which the C never reads).
    unsafe {
        c::oracle_video_scaler_plane_heights(
            heights.as_mut_ptr(),
            height,
            planes.as_ptr(),
            shift_ptr,
        );
    }
    heights
}

fn shim_heights(height: u32, planes: [c_int; 4], shift: u8) -> [c_int; 4] {
    let mut heights = [UNSET; 4];
    let shift_ptr: *const u8 = if has_chroma(planes) {
        &raw const shift
    } else {
        core::ptr::null()
    };
    // SAFETY: as for the oracle.
    unsafe {
        shim::video_scaler_plane_heights(&raw mut heights, height, &raw const planes, shift_ptr);
    }
    heights
}

/// The C's heights with the intentional difference applied: planes 1 and 2,
/// where the C wrote them, one more where the height is not a multiple of
/// `2^shift` (wrapping as the C's `int` cast does).
fn expected_port(c_heights: [c_int; 4], height: u32, shift: u8) -> [c_int; 4] {
    let mut want = c_heights;
    let rem = height & ((1u32 << shift) - 1);
    for plane in [1, 2] {
        if want[plane] != UNSET && rem != 0 {
            want[plane] = want[plane].wrapping_add(1);
        }
    }
    want
}

fn heights_agree(height: u32, planes: [c_int; 4], shift: u8) {
    let c_heights = oracle_heights(height, planes, shift);
    let want = expected_port(c_heights, height, shift);
    assert_eq!(
        shim_heights(height, planes, shift),
        want,
        "shim, {planes:?} shift {shift} at {height} (C {c_heights:?})"
    );
    let mut reads = 0;
    let core_heights = core_::plane_heights(height, planes, || {
        reads += 1;
        shift
    });
    let core_heights = core_heights.map(|h| h.unwrap_or(UNSET));
    assert_eq!(
        core_heights, want,
        "core, {planes:?} shift {shift} at {height}"
    );
    // the C reads the shift once for each of planes 1 and 2 the format has
    let chroma = [1, 2].iter().filter(|&&p| planes.contains(&p)).count();
    assert_eq!(reads, chroma, "reads, {planes:?}");
}

fn copy_agrees(scaled: usize, plane: usize, height: usize) {
    let mut c_size = usize::MAX;
    let mut shim_size = usize::MAX;
    // SAFETY: a writable size_t each.
    let (c_whole, shim_whole) = unsafe {
        (
            c::oracle_video_scaler_plane_copy(scaled, plane, height, &raw mut c_size),
            shim::video_scaler_plane_copy(scaled, plane, height, &raw mut shim_size),
        )
    };
    assert_eq!(
        (shim_whole, shim_size),
        (c_whole, c_size),
        "copy {scaled} to {plane}, {height} rows"
    );
    let core_copy = core_::plane_copy(scaled, plane, height);
    let want = if c_whole {
        PlaneCopy::Whole { size: c_size }
    } else {
        PlaneCopy::Rows { row: c_size }
    };
    assert_eq!(core_copy, want, "core copy {scaled} to {plane}, {height}");
}

/// Every pattern of four component planes, 0 to 3 each.
fn patterns() -> impl Iterator<Item = [c_int; 4]> {
    (0..256).map(|i: c_int| [i & 3, (i >> 2) & 3, (i >> 4) & 3, (i >> 6) & 3])
}

#[test]
fn parity_constants() {
    for (name, value) in [
        ("VIDEO_SCALER_SWS_FAST_BILINEAR", sws::FAST_BILINEAR),
        ("VIDEO_SCALER_SWS_BILINEAR", sws::BILINEAR),
        ("VIDEO_SCALER_SWS_BICUBIC", sws::BICUBIC),
        ("VIDEO_SCALER_SWS_POINT", sws::POINT),
        ("VIDEO_SCALER_SWS_AREA", sws::AREA),
        ("VIDEO_SCALER_SWS_CS_ITU709", sws::CS_ITU709),
        ("VIDEO_SCALER_SWS_CS_ITU601", sws::CS_ITU601),
        ("VIDEO_SCALER_SWS_CS_BT2020", sws::CS_BT2020),
        (
            "VIDEO_SCALE_DEFAULT",
            video_scale_type_to_c(VideoScaleType::Default),
        ),
        (
            "VIDEO_SCALE_POINT",
            video_scale_type_to_c(VideoScaleType::Point),
        ),
        (
            "VIDEO_SCALE_FAST_BILINEAR",
            video_scale_type_to_c(VideoScaleType::FastBilinear),
        ),
        (
            "VIDEO_SCALE_BILINEAR",
            video_scale_type_to_c(VideoScaleType::Bilinear),
        ),
        (
            "VIDEO_SCALE_BICUBIC",
            video_scale_type_to_c(VideoScaleType::Bicubic),
        ),
        (
            "VIDEO_CS_DEFAULT",
            video_colorspace_to_c(VideoColorspace::Default),
        ),
        (
            "VIDEO_CS_601",
            video_colorspace_to_c(VideoColorspace::Cs601),
        ),
        (
            "VIDEO_CS_709",
            video_colorspace_to_c(VideoColorspace::Cs709),
        ),
        (
            "VIDEO_CS_SRGB",
            video_colorspace_to_c(VideoColorspace::Srgb),
        ),
        (
            "VIDEO_CS_2100_PQ",
            video_colorspace_to_c(VideoColorspace::Cs2100Pq),
        ),
        (
            "VIDEO_CS_2100_HLG",
            video_colorspace_to_c(VideoColorspace::Cs2100Hlg),
        ),
        (
            "VIDEO_RANGE_DEFAULT",
            video_range_type_to_c(VideoRangeType::Default),
        ),
        (
            "VIDEO_RANGE_PARTIAL",
            video_range_type_to_c(VideoRangeType::Partial),
        ),
        (
            "VIDEO_RANGE_FULL",
            video_range_type_to_c(VideoRangeType::Full),
        ),
    ] {
        assert_eq!(oracle_int(name), i64::from(value), "{name}");
    }
    // SAFETY: an index past the oracle's table.
    assert_eq!(
        unsafe { c::oracle_video_scaler_plan_int(c::INT_NAMES.len()) },
        -1
    );
}

#[test]
fn parity_maps_every_value() {
    for v in -3..=12 {
        maps_agree(v);
    }
    for v in [c_int::MIN, c_int::MIN + 1, c_int::MAX - 1, c_int::MAX] {
        maps_agree(v);
    }
}

#[test]
fn parity_heights_every_pattern() {
    // Every pattern and shift at every height to 4100; at the reduced
    // level a fixed tenth of the heights past the first 64.
    for height in 0..=4100u32 {
        if height >= 64 && !obs_c_oracle::parity::keep(u64::from(height)) {
            continue;
        }
        for planes in patterns() {
            for shift in 0..=2 {
                heights_agree(height, planes, shift);
            }
        }
    }
}

#[test]
fn parity_heights_at_the_ends_of_uint32() {
    let heights = [
        (1u32 << 31) - 2,
        (1 << 31) - 1,
        1 << 31,
        (1 << 31) + 1,
        u32::MAX - 2,
        u32::MAX - 1,
        u32::MAX,
    ];
    for height in heights {
        for planes in patterns() {
            for shift in 0..=2 {
                heights_agree(height, planes, shift);
            }
        }
    }
}

#[test]
fn parity_copy_edges() {
    let sizes = [
        0,
        1,
        31,
        32,
        1920,
        1952,
        usize::MAX / 2,
        usize::MAX - 1,
        usize::MAX,
    ];
    for scaled in sizes {
        for plane in sizes {
            for height in [0, 1, 2, 1080, usize::MAX] {
                copy_agrees(scaled, plane, height);
            }
        }
    }
}

#[test]
fn null_pointers_do_nothing_in_the_shims() {
    let planes = [0, 1, 2, 3];
    let shift = 1u8;
    // SAFETY: null for the pointer under test, live locals for the others.
    unsafe {
        shim::video_scaler_plane_heights(
            core::ptr::null_mut(),
            720,
            &raw const planes,
            &raw const shift,
        );
    }
    let mut heights = [UNSET; 4];
    // SAFETY: as above.
    unsafe {
        shim::video_scaler_plane_heights(
            &raw mut heights,
            720,
            core::ptr::null(),
            &raw const shift,
        );
    }
    assert_eq!(heights, [UNSET; 4]);
    // a null shift reads as 0
    // SAFETY: as above.
    unsafe {
        shim::video_scaler_plane_heights(
            &raw mut heights,
            720,
            &raw const planes,
            core::ptr::null(),
        );
    }
    assert_eq!(heights, [720; 4]);
    // SAFETY: a null size.
    let whole = unsafe { shim::video_scaler_plane_copy(64, 64, 4, core::ptr::null_mut()) };
    assert!(whole);
    // SAFETY: a null size.
    let whole = unsafe { shim::video_scaler_plane_copy(64, 32, 4, core::ptr::null_mut()) };
    assert!(!whole);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(20_000)))]

    #[test]
    fn prop_maps(v in any::<i32>()) {
        maps_agree(v);
    }

    #[test]
    fn prop_heights(
        height in any::<u32>(),
        planes in prop::array::uniform4(0..4i32),
        shift in 0..=2u8,
    ) {
        heights_agree(height, planes, shift);
    }

    #[test]
    fn prop_copy(
        scaled in 0..=16384usize,
        plane in 0..=16384usize,
        height in 0..=4100usize,
    ) {
        copy_agrees(scaled, plane, height);
    }

    #[test]
    fn prop_copy_equal(linesize in 0..=16384usize, height in 0..=4100usize) {
        copy_agrees(linesize, linesize, height);
    }
}
