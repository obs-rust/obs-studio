//! Tier 3: the `obs_source_conversion_plan_fill` shim against the original
//! C (`obs-c-oracle`, `oracle/source_conversion_plan.c`: the unmodified
//! `libobs/obs-source-conversion-plan.c.h` with its function renamed).
//!
//! Both sides fill a `struct obs_source_conversion_plan` through the C ABI,
//! each from a plan prefilled with `0xA5` bytes, so a field either side
//! leaves alone shows. A run compares every field: the convert type, the
//! channel count, the four planes' widths, heights and formats, the linear
//! flag, and the technique as a string (both null, or the same bytes).
//!
//! Runs:
//! - the struct's layout and every enum value the port uses against the C
//!   headers;
//! - exhaustive over format x range x transfer: every `enum video_format`
//!   and values around it (-3 to 40 and the ends of `i32`, 47 values), both
//!   ranges, and transfers -2 to 260, 511 to 514 and the ends of `i32` (270
//!   values: every `uint8_t` value, and values that wrap onto PQ and HLG),
//!   each at 22 widths and 22 heights (around 0, the half and multiply wraps
//!   and `u32::MAX`), the other side cycling: 1,116,720 plans;
//! - every pair of the 22 sizes for formats 0 to 26, both ranges and
//!   transfers 0 to 4: 130,680 plans;
//! - a proptest (20,000 cases) over any `u32` width and height, any `i32`
//!   format and transfer (half of them in their enums) and both ranges;
//! - the technique is a static string: the same pointer for the same
//!   input, and the core's `CStr`.
//!
//! Intentional difference, checked on the shim only: a null plan is left
//! alone (the C writes through it).

use std::ffi::{CStr, c_int};
use std::mem::{align_of, offset_of, size_of};
use std::ptr;

use obs_c_oracle::source_conversion_plan::{self as oracle, OracleConversionPlan};
use obs_source::conversion_plan::{
    ConvertType, PLANES, gs_color_format as GS, plan, video_format as F, video_trc,
};
use obs_source::ffi::source_conversion_plan::{
    obs_source_conversion_plan, obs_source_conversion_plan_fill,
};
use proptest::prelude::*;

#[test]
fn layout_matches_the_c_header() {
    type P = obs_source_conversion_plan;
    let rust = [
        size_of::<P>(),
        align_of::<P>(),
        offset_of!(P, convert_type),
        offset_of!(P, channel_count),
        offset_of!(P, width),
        offset_of!(P, height),
        offset_of!(P, format),
        offset_of!(P, technique),
        offset_of!(P, linear),
        PLANES,
        size_of::<c_int>(),
        size_of::<c_int>(),
        size_of::<c_int>(),
        size_of::<c_int>(),
        size_of::<bool>(),
    ];
    for (which, (name, value)) in oracle::LAYOUT.iter().zip(rust).enumerate() {
        // SAFETY: a pure lookup.
        let c = unsafe { oracle::oracle_source_conversion_plan_layout(which) };
        assert_eq!(value, c, "{name}");
    }
    // SAFETY: a pure lookup.
    assert_eq!(
        unsafe { oracle::oracle_source_conversion_plan_layout(oracle::LAYOUT.len()) },
        usize::MAX
    );
    assert_eq!(size_of::<OracleConversionPlan>(), size_of::<P>());
    assert_eq!(align_of::<OracleConversionPlan>(), align_of::<P>());
}

#[test]
fn values_match_the_c_headers() {
    let trcs = [
        video_trc::DEFAULT,
        video_trc::SRGB,
        video_trc::PQ,
        video_trc::HLG,
    ];
    let gs = [
        GS::UNKNOWN,
        GS::R8,
        GS::RGBA,
        GS::BGRX,
        GS::BGRA,
        GS::R10G10B10A2,
        GS::R16,
        GS::RGBA16F,
        GS::R8G8,
        GS::BGRA_UNORM,
        GS::RG16,
    ];
    let formats = [
        F::NONE,
        F::I420,
        F::NV12,
        F::YVYU,
        F::YUY2,
        F::UYVY,
        F::RGBA,
        F::BGRA,
        F::BGRX,
        F::Y800,
        F::I444,
        F::BGR3,
        F::I422,
        F::I40A,
        F::I42A,
        F::YUVA,
        F::AYUV,
        F::I010,
        F::P010,
        F::I210,
        F::I412,
        F::YA2L,
        F::P216,
        F::P416,
        F::V210,
        F::R10L,
    ];
    let rust: Vec<i64> = formats
        .into_iter()
        .map(i64::from)
        .chain(trcs.into_iter().map(i64::from))
        .chain(gs.into_iter().map(i64::from))
        .chain(ConvertType::ALL.into_iter().map(|t| i64::from(t.to_c())))
        .collect();
    assert_eq!(rust.len(), oracle::VALUES.len());
    for (which, (name, value)) in oracle::VALUES.iter().zip(rust).enumerate() {
        // SAFETY: a pure lookup.
        let c = unsafe { oracle::oracle_source_conversion_plan_value(which) };
        assert_eq!(value, c, "{name}");
    }
    // SAFETY: a pure lookup.
    assert_eq!(
        unsafe { oracle::oracle_source_conversion_plan_value(oracle::VALUES.len()) },
        i64::MIN
    );
}

// --------------------------------------------------------------------------
// One call on each side

/// What a plan holds, the technique as its bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Filled {
    convert_type: c_int,
    channel_count: c_int,
    width: [u32; PLANES],
    height: [u32; PLANES],
    format: [c_int; PLANES],
    technique: Option<Vec<u8>>,
    linear: bool,
}

/// The bytes of a technique pointer: `None` for null.
///
/// # Safety
///
/// `technique` is null or a NUL-terminated string that lives for the call.
unsafe fn technique_bytes(technique: *const std::ffi::c_char) -> Option<Vec<u8>> {
    if technique.is_null() {
        None
    } else {
        // SAFETY: non-null and NUL-terminated (caller contract).
        Some(unsafe { CStr::from_ptr(technique) }.to_bytes().to_vec())
    }
}

fn c_fill(format: i32, full_range: bool, trc: i32, width: u32, height: u32) -> Filled {
    let mut plan = std::mem::MaybeUninit::<OracleConversionPlan>::uninit();
    // SAFETY: every byte written before the call, then the C writes the
    // plan; the bool is written by the C (memset, then a store) before it
    // is read.
    let plan = unsafe {
        ptr::write_bytes(plan.as_mut_ptr(), 0xA5, 1);
        oracle::oracle_obs_source_conversion_plan_fill(
            format,
            full_range,
            trc,
            width,
            height,
            plan.as_mut_ptr(),
        );
        plan.assume_init()
    };
    Filled {
        convert_type: plan.convert_type,
        channel_count: plan.channel_count,
        width: plan.width,
        height: plan.height,
        format: plan.format,
        // SAFETY: the C's string literals, or null.
        technique: unsafe { technique_bytes(plan.technique) },
        linear: plan.linear,
    }
}

fn rust_fill(format: i32, full_range: bool, trc: i32, width: u32, height: u32) -> Filled {
    let mut plan = std::mem::MaybeUninit::<obs_source_conversion_plan>::uninit();
    // SAFETY: as for the C; the shim writes the whole struct.
    let plan = unsafe {
        ptr::write_bytes(plan.as_mut_ptr(), 0xA5, 1);
        obs_source_conversion_plan_fill(format, full_range, trc, width, height, plan.as_mut_ptr());
        plan.assume_init()
    };
    Filled {
        convert_type: plan.convert_type,
        channel_count: plan.channel_count,
        width: plan.width,
        height: plan.height,
        format: plan.format,
        // SAFETY: the core's static C strings, or null.
        technique: unsafe { technique_bytes(plan.technique) },
        linear: plan.linear,
    }
}

#[track_caller]
fn check(format: i32, full_range: bool, trc: i32, width: u32, height: u32) {
    let c = c_fill(format, full_range, trc, width, height);
    let r = rust_fill(format, full_range, trc, width, height);
    assert_eq!(
        r, c,
        "format {format} full_range {full_range} trc {trc} size {width} x {height}"
    );
}

// --------------------------------------------------------------------------
// Exhaustive runs

/// Every `enum video_format` and values around it.
fn formats() -> Vec<i32> {
    (-3..=40)
        .chain([i32::MIN, i32::MIN + 1, i32::MAX])
        .collect()
}

/// Every `uint8_t`, values that wrap onto every enum value, and the ends of
/// `i32`.
fn trcs() -> Vec<i32> {
    (-2..=260)
        .chain(511..=514)
        .chain([i32::MIN, i32::MIN + 2, i32::MAX])
        .collect()
}

/// Sizes around 0, the half and multiply wraps and `u32::MAX`.
const SIZES: [u32; 22] = [
    0,
    1,
    2,
    3,
    4,
    5,
    6,
    7,
    1080,
    1281,
    1920,
    0x5555_5555,
    0x5555_5556,
    0x7FFF_FFFF,
    0x8000_0000,
    u32::MAX - 6,
    u32::MAX - 5,
    u32::MAX - 4,
    u32::MAX - 3,
    u32::MAX - 2,
    u32::MAX - 1,
    u32::MAX,
];

/// Format x range x transfer, each at every width (the height cycling
/// through the sizes) and every height (the width cycling).
#[test]
fn exhaustive_format_range_trc() {
    let mut count = 0usize;
    for format in formats() {
        for full_range in [false, true] {
            for (t, trc) in trcs().into_iter().enumerate() {
                for (i, &width) in SIZES.iter().enumerate() {
                    let height = SIZES[(i + t) % SIZES.len()];
                    check(format, full_range, trc, width, height);
                    check(format, full_range, trc, height, width);
                    count += 2;
                }
            }
        }
    }
    assert_eq!(count, formats().len() * 2 * trcs().len() * SIZES.len() * 2);
}

/// Every pair of sizes, for every in-enum format, both ranges and the
/// four transfers.
#[test]
fn exhaustive_sizes() {
    for format in F::NONE..=F::R10L + 1 {
        for full_range in [false, true] {
            for trc in 0..=4 {
                for &width in &SIZES {
                    for &height in &SIZES {
                        check(format, full_range, trc, width, height);
                    }
                }
            }
        }
    }
}

/// The technique is a static string: the shim gives the core's pointer,
/// the same for the same input.
#[test]
fn technique_is_static() {
    for format in formats() {
        for full_range in [false, true] {
            for trc in 0..=4 {
                let core = plan(format, full_range, trc, 2, 2).technique;
                let mut a = std::mem::MaybeUninit::<obs_source_conversion_plan>::uninit();
                let mut b = std::mem::MaybeUninit::<obs_source_conversion_plan>::uninit();
                // SAFETY: writable plans.
                let (a, b) = unsafe {
                    obs_source_conversion_plan_fill(format, full_range, trc, 2, 2, a.as_mut_ptr());
                    obs_source_conversion_plan_fill(format, full_range, trc, 7, 9, b.as_mut_ptr());
                    (a.assume_init(), b.assume_init())
                };
                assert_eq!(a.technique, b.technique);
                assert_eq!(a.technique, core.map_or(ptr::null(), CStr::as_ptr));
            }
        }
    }
}

/// Intentional difference: a null plan is left alone (the C writes through
/// it).
#[test]
fn null_plan_is_left_alone() {
    // SAFETY: a null plan is allowed.
    unsafe { obs_source_conversion_plan_fill(F::NV12, false, 0, 1920, 1080, ptr::null_mut()) };
}

fn any_format() -> impl Strategy<Value = i32> {
    prop_oneof![F::NONE..=F::R10L, any::<i32>()]
}

fn any_trc() -> impl Strategy<Value = i32> {
    prop_oneof![0..=4i32, any::<i32>()]
}

fn any_size() -> impl Strategy<Value = u32> {
    prop_oneof![any::<u32>(), (u32::MAX - 8)..=u32::MAX, 0..=8u32]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20_000))]

    #[test]
    fn proptest_full_u32_sizes(
        format in any_format(),
        full_range in any::<bool>(),
        trc in any_trc(),
        width in any_size(),
        height in any_size(),
    ) {
        let c = c_fill(format, full_range, trc, width, height);
        let r = rust_fill(format, full_range, trc, width, height);
        prop_assert_eq!(r, c);
    }
}
