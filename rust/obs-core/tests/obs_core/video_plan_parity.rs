//! Tier 3: the shims against the original `libobs/obs-video-plan.c.h`.
//!
//! The oracle (`rust/obs-c-oracle/oracle/video_plan.c`) is the C file,
//! unmodified, with its five functions renamed. Compared:
//!
//! - the layout of `struct obs_video_plane` and `struct obs_video_output_plan`
//!   and the width of the enums, against the C header;
//! - the enum values the safe core uses, against the C header;
//! - exhaustively, every `enum video_format` value and every
//!   `enum video_colorspace` value, with values outside both enums
//!   (`-1`, one and two past the end, `i32::MIN`, `i32::MAX`), at every size
//!   of `test_video_output_plan` and the extremes: every field of the plan,
//!   the technique names byte for byte (the pointers differ), floats bit for
//!   bit;
//! - every `enum obs_scale_type` value and values outside it;
//! - the size checks and the alignment, at the edges and over any `u32`;
//! - a proptest over any format and color space value and any sizes.
//!
//! Intentional difference, excluded here and tested on the shim alone: a
//! null pointer to `obs_video_align_output_size` is skipped (C dereferences
//! it). `test_video_output_plan` compiles the C into itself, so this is what
//! compares the swap; `test_video_output_formats` runs the swap through
//! `obs_reset_video()` and real frames.

use std::ffi::{CStr, c_char, c_int};
use std::mem::{align_of, offset_of, size_of};

use obs_c_oracle::video_plan::{
    LAYOUT, OracleVideoOutputPlan, VALUE_COUNTS, oracle_obs_video_align_output_size,
    oracle_obs_video_output_plan, oracle_obs_video_output_size_valid,
    oracle_obs_video_scale_type_name, oracle_obs_video_size_valid, oracle_video_plan_layout,
    oracle_video_plan_value,
};
use obs_core::ffi::video_plan::{
    obs_video_align_output_size, obs_video_output_plan, obs_video_output_size_valid,
    obs_video_plane, obs_video_scale_type_name, obs_video_size_valid,
};
use obs_core::video_plan::{GsColorFormat, GsColorSpace, NUM_CHANNELS, ObsScaleType};
use obs_media_io::video_io::{VideoColorspace, VideoFormat};
use proptest::prelude::*;

/// `test_video_output_plan`'s sizes, 0, and the extremes.
const SIZES: [u32; 24] = [
    0,
    1,
    2,
    3,
    4,
    5,
    6,
    7,
    8,
    9,
    767,
    1080,
    1081,
    1364,
    1366,
    1920,
    32767,
    32768,
    32769,
    65536,
    0x7FFF_FFFF,
    0x8000_0000,
    0xFFFF_FFFE,
    u32::MAX,
];

/// Values outside an enum whose last value is `last`.
fn outside(last: c_int) -> [c_int; 5] {
    [-1, last + 1, last + 2, c_int::MIN, c_int::MAX]
}

fn all_format_values() -> Vec<c_int> {
    let last = VideoFormat::R10l as c_int;
    (0..=last).chain(outside(last)).collect()
}

fn all_colorspace_values() -> Vec<c_int> {
    let last = VideoColorspace::Cs2100Hlg as c_int;
    (0..=last).chain(outside(last)).collect()
}

/// The string at `p`, or `None` for a null pointer.
///
/// # Safety
///
/// `p` is null or a NUL-terminated string.
unsafe fn name(p: *const c_char) -> Option<Vec<u8>> {
    // SAFETY: the caller passes null or a NUL-terminated string.
    (!p.is_null()).then(|| unsafe { CStr::from_ptr(p) }.to_bytes().to_vec())
}

fn compare_plan(format: c_int, colorspace: c_int, width: u32, height: u32) {
    let rust = obs_video_output_plan(format, colorspace, width, height);
    // SAFETY: no pointers in; the oracle returns the struct by value.
    let c: OracleVideoOutputPlan =
        unsafe { oracle_obs_video_output_plan(format, colorspace, width, height) };
    let what = format!("format {format}, colorspace {colorspace}, {width}x{height}");

    assert_eq!(rust.conversion_needed, c.conversion_needed, "{what}");
    for i in 0..NUM_CHANNELS {
        // SAFETY: both are null or static NUL-terminated strings.
        let (r, o) = unsafe { (name(rust.conversion_techs[i]), name(c.conversion_techs[i])) };
        assert_eq!(r, o, "{what}: technique {i}");
    }
    assert_eq!(
        rust.conversion_width_i.to_bits(),
        c.conversion_width_i.to_bits(),
        "{what}: width_i {} vs {}",
        rust.conversion_width_i,
        c.conversion_width_i
    );
    assert_eq!(
        rust.conversion_height_i.to_bits(),
        c.conversion_height_i.to_bits(),
        "{what}: height_i {} vs {}",
        rust.conversion_height_i,
        c.conversion_height_i
    );
    for i in 0..NUM_CHANNELS {
        let (r, o) = (rust.planes[i], c.planes[i]);
        assert_eq!(
            (r.width, r.height, r.format),
            (o.width, o.height, o.format),
            "{what}: plane {i}"
        );
    }
    assert_eq!(rust.texture_format, c.texture_format, "{what}");
    assert_eq!(rust.render_space, c.render_space, "{what}");
}

fn compare_sizes(width: u32, height: u32) {
    // SAFETY: no pointers.
    let (c_valid, c_output_valid) = unsafe {
        (
            oracle_obs_video_size_valid(width, height),
            oracle_obs_video_output_size_valid(width, height),
        )
    };
    assert_eq!(
        obs_video_size_valid(width, height),
        c_valid,
        "{width}x{height}"
    );
    assert_eq!(
        obs_video_output_size_valid(width, height),
        c_output_valid,
        "{width}x{height}"
    );

    let (mut rw, mut rh) = (width, height);
    let (mut cw, mut ch) = (width, height);
    // SAFETY: four live locals.
    unsafe {
        obs_video_align_output_size(&raw mut rw, &raw mut rh);
        oracle_obs_video_align_output_size(&raw mut cw, &raw mut ch);
    }
    assert_eq!((rw, rh), (cw, ch), "align {width}x{height}");
}

fn compare_scale_type(value: c_int) {
    // SAFETY: both return a static NUL-terminated string.
    let (r, o) = unsafe {
        (
            name(obs_video_scale_type_name(value)),
            name(oracle_obs_video_scale_type_name(value)),
        )
    };
    assert!(r.is_some(), "scale type {value}: never NULL");
    assert_eq!(r, o, "scale type {value}");
}

#[test]
fn layout_matches_the_c_header() {
    use obs_core::ffi::video_plan::obs_video_output_plan as Plan;
    let rust = [
        size_of::<obs_video_plane>(),
        align_of::<obs_video_plane>(),
        offset_of!(obs_video_plane, width),
        offset_of!(obs_video_plane, height),
        offset_of!(obs_video_plane, format),
        size_of::<Plan>(),
        align_of::<Plan>(),
        offset_of!(Plan, conversion_needed),
        offset_of!(Plan, conversion_techs),
        offset_of!(Plan, conversion_width_i),
        offset_of!(Plan, conversion_height_i),
        offset_of!(Plan, planes),
        offset_of!(Plan, texture_format),
        offset_of!(Plan, render_space),
        NUM_CHANNELS,
        size_of::<c_int>(),
        size_of::<c_int>(),
        size_of::<c_int>(),
        size_of::<c_int>(),
        size_of::<c_int>(),
    ];
    for (i, (want, what)) in rust.into_iter().zip(LAYOUT).enumerate() {
        // SAFETY: no pointers.
        assert_eq!(unsafe { oracle_video_plan_layout(i) }, want, "{what}");
    }
    // SAFETY: no pointers.
    assert_eq!(
        unsafe { oracle_video_plan_layout(LAYOUT.len()) },
        usize::MAX
    );
}

#[test]
fn enum_values_match_the_c_header() {
    let groups: [Vec<i64>; 5] = [
        GsColorFormat::ALL.map(|v| i64::from(v.to_c())).to_vec(),
        GsColorSpace::ALL.map(|v| i64::from(v.to_c())).to_vec(),
        ObsScaleType::ALL.map(|v| i64::from(v.to_c())).to_vec(),
        VideoFormat::ALL.map(|v| v as i64).to_vec(),
        VideoColorspace::ALL.map(|v| v as i64).to_vec(),
    ];
    let mut which = 0;
    for (group, (values, count)) in groups.iter().zip(VALUE_COUNTS).enumerate() {
        assert_eq!(values.len(), count, "group {group}");
        for &v in values {
            // SAFETY: no pointers.
            assert_eq!(
                unsafe { oracle_video_plan_value(which) },
                v,
                "group {group}"
            );
            which += 1;
        }
    }
    // SAFETY: no pointers.
    assert_eq!(unsafe { oracle_video_plan_value(which) }, i64::MIN);
}

/// Every format and color space value, inside and outside the enums, at
/// every size pair.
#[test]
fn every_format_colorspace_and_size() {
    let mut checked = 0usize;
    for f in all_format_values() {
        for c in all_colorspace_values() {
            for w in SIZES {
                for h in SIZES {
                    compare_plan(f, c, w, h);
                    checked += 1;
                }
            }
        }
    }
    assert_eq!(checked, 31 * 11 * SIZES.len() * SIZES.len());
}

#[test]
fn every_scale_type() {
    let last = ObsScaleType::Area.to_c();
    for v in (0..=last).chain(outside(last)).chain([-2]) {
        compare_scale_type(v);
    }
}

#[test]
fn size_checks_and_alignment_at_the_edges() {
    for w in SIZES {
        for h in SIZES {
            compare_sizes(w, h);
        }
    }
}

/// The intentional difference: a null pointer is skipped, and the other
/// one is still aligned (C dereferences it). Shim only.
#[test]
fn align_skips_a_null_pointer() {
    let mut w = 1366u32;
    let mut h = 767u32;
    // SAFETY: one live local and one null pointer, which the shim skips.
    unsafe {
        obs_video_align_output_size(&raw mut w, std::ptr::null_mut());
        obs_video_align_output_size(std::ptr::null_mut(), &raw mut h);
        obs_video_align_output_size(std::ptr::null_mut(), std::ptr::null_mut());
    }
    assert_eq!((w, h), (1364, 766));
}

/// A format or color space value from all of `c_int`, biased to the enums.
fn any_enum_value(last: c_int) -> impl Strategy<Value = c_int> {
    prop_oneof![
        4 => 0..=last,
        1 => -4..=last + 4,
        1 => any::<c_int>(),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4096))]

    #[test]
    fn plan_matches_the_c_for_any_size(
        f in any_enum_value(VideoFormat::R10l as c_int),
        c in any_enum_value(VideoColorspace::Cs2100Hlg as c_int),
        w in any::<u32>(),
        h in any::<u32>(),
    ) {
        compare_plan(f, c, w, h);
    }

    #[test]
    fn plan_matches_the_c_for_small_sizes(
        f in any_enum_value(VideoFormat::R10l as c_int),
        c in any_enum_value(VideoColorspace::Cs2100Hlg as c_int),
        w in 0u32..=65536,
        h in 0u32..=65536,
    ) {
        compare_plan(f, c, w, h);
    }

    #[test]
    fn size_checks_match_the_c_for_any_size(w in any::<u32>(), h in any::<u32>()) {
        compare_sizes(w, h);
    }

    #[test]
    fn size_checks_match_the_c_near_the_bounds(w in 0u32..=40000, h in 0u32..=40000) {
        compare_sizes(w, h);
    }

    #[test]
    fn scale_type_matches_the_c(v in any::<c_int>()) {
        compare_scale_type(v);
    }
}
