//! Tier 3: the six `obs_source_frame_copy_*` shims and the safe core
//! against the original C (`obs-c-oracle`, `oracle/source_frame_copy.c`: the
//! unmodified `libobs/obs-source-frame-copy.c.h` with its functions renamed).
//!
//! Every call runs the shim, the oracle and the core, and compares each
//! result field by field: every plane's whole flag, bytes, rows and strides,
//! the count and the colour range flag; the step as the C's enum value; the
//! age's three fields; the range and preload answers as bools.
//!
//! Runs:
//! - the layout of the six structs, the constants, the step and range values,
//!   and every `enum video_format` value, against the C headers;
//! - exhaustive over format x line-size relation: every C format value (-3
//!   to 30 and the ends of `i32`, 38 values) against all 625 patterns of the
//!   four planes (the source's line size equal to the destination's,
//!   smaller, larger or 0, or the destination's 0), at 16 heights (0 to 5,
//!   1079 to 1081 and the `u32` wrap edges) and 3 destination line sizes
//!   (1920, 7 and `u32::MAX`, each plane's 3 less than the one before), for
//!   both ranges: 2,280,000 plans, every kind of copy reached; and the line
//!   sizes real frames have, padded and not, at real sizes;
//! - a proptest (8192 cases) over full-range `u32` heights and line sizes,
//!   any `i32` format and both ranges;
//! - the cache step over every queue length up to 64 and the `usize` ends
//!   against keys that differ in each field alone, at the field ends; the
//!   clean step over every count from -1000 to 1000 and the `c_long` ends;
//!   the range rules over formats -300 to 300 and the `i32` ends, both ranges
//!   and every `video_range_type` from -10 to 10 and the ends; the preload
//!   rule over every combination of shapes around the edges; and proptests
//!   over each.
//!
//! Intentional difference, unreachable, kept out of the oracle: for an
//! unused entry with a count of `LONG_MAX`, the C's `++unused_count`
//! overflows a `long`, which is undefined (and MSVC can trap it, as it traps
//! other overflows). The oracle is never called with that input, on any
//! platform; the shim and the core wrap to `LONG_MIN` and are compared with
//! each other there. No entry gets past 5: counts start at 0 and an entry is
//! erased at 5. Every other input of every function is defined in C on every
//! platform: the plan's arithmetic is unsigned, and the formats and ranges
//! only convert and compare.

use std::ffi::{c_int, c_long};
use std::mem::{align_of, offset_of, size_of};

use obs_c_oracle::source_frame_copy::{
    self as oracle, OracleCacheAge, OracleCopyPlan, OracleKey, OracleLines, OracleShape,
};
use obs_source::ffi::source_frame_copy::{
    self as shim, obs_source_frame_cache_age, obs_source_frame_copy_key,
    obs_source_frame_copy_lines, obs_source_frame_copy_plan, obs_source_frame_copy_shape,
    obs_source_frame_plane_copy,
};
use obs_source::frame_copy::{
    self as core, CacheAge, CacheStep, Key, MAX_ASYNC_FRAMES, MAX_UNUSED, PLANES, Shape,
    video_format as F, video_range as R,
};
use proptest::prelude::*;

/// One plan as plain fields: the colour range, the count, and each plane's
/// whole, bytes, rows and strides.
type PlanFields = (bool, usize, [(bool, usize, u32, u32, u32); PLANES]);

fn shim_fields(plan: obs_source_frame_copy_plan) -> PlanFields {
    (
        plan.color_range,
        plan.count,
        plan.planes
            .map(|p| (p.whole, p.bytes, p.rows, p.src_stride, p.dst_stride)),
    )
}

fn oracle_fields(plan: OracleCopyPlan) -> PlanFields {
    (
        plan.color_range,
        plan.count,
        plan.planes
            .map(|p| (p.whole, p.bytes, p.rows, p.src_stride, p.dst_stride)),
    )
}

fn core_fields(plan: core::CopyPlan) -> PlanFields {
    (
        plan.color_range,
        plan.count,
        plan.planes
            .map(|p| (p.whole, p.bytes, p.rows, p.src_stride, p.dst_stride)),
    )
}

#[track_caller]
fn check_plan(
    format: c_int,
    full_range: bool,
    height: u32,
    src: [u32; PLANES],
    dst: [u32; PLANES],
) {
    let s = shim_fields(shim::obs_source_frame_copy_plan(
        format,
        full_range,
        height,
        obs_source_frame_copy_lines { linesize: src },
        obs_source_frame_copy_lines { linesize: dst },
    ));
    // SAFETY: plain values in and out; every input is defined in C.
    let c = oracle_fields(unsafe {
        oracle::oracle_obs_source_frame_copy_plan(
            format,
            full_range,
            height,
            OracleLines { linesize: src },
            OracleLines { linesize: dst },
        )
    });
    let r = core_fields(core::copy_plan(format, full_range, height, src, dst));
    assert_eq!(
        s, c,
        "plan: shim vs C, format {format} full {full_range} height {height} src {src:?} dst {dst:?}"
    );
    assert_eq!(r, c, "plan: core vs C, format {format}");
}

fn step_value(step: CacheStep) -> u32 {
    match step {
        CacheStep::Keep => shim::OBS_SOURCE_FRAME_CACHE_KEEP,
        CacheStep::Reset => shim::OBS_SOURCE_FRAME_CACHE_RESET,
        CacheStep::Overflow => shim::OBS_SOURCE_FRAME_CACHE_OVERFLOW,
    }
}

#[track_caller]
fn check_step(queued: usize, cache: (u32, u32, c_int), frame: (u32, u32, c_int)) {
    let s = shim::obs_source_frame_copy_cache_step(
        queued,
        obs_source_frame_copy_key {
            width: cache.0,
            height: cache.1,
            convert_type: cache.2,
        },
        obs_source_frame_copy_key {
            width: frame.0,
            height: frame.1,
            convert_type: frame.2,
        },
    );
    // SAFETY: plain values.
    let c = unsafe {
        oracle::oracle_obs_source_frame_copy_cache_step(
            queued,
            OracleKey {
                width: cache.0,
                height: cache.1,
                convert_type: cache.2,
            },
            OracleKey {
                width: frame.0,
                height: frame.1,
                convert_type: frame.2,
            },
        )
    };
    let r = step_value(core::cache_step(
        queued,
        Key {
            width: cache.0,
            height: cache.1,
            convert_type: cache.2,
        },
        Key {
            width: frame.0,
            height: frame.1,
            convert_type: frame.2,
        },
    ));
    assert_eq!(s, c, "step: shim vs C, {queued} {cache:?} {frame:?}");
    assert_eq!(r, c, "step: core vs C, {queued} {cache:?} {frame:?}");
}

fn age_core(used: bool, count: c_long) -> (bool, c_long, bool) {
    match core::clean(used, count) {
        CacheAge::InUse => (false, count, false),
        CacheAge::Counted(n) => (true, n, false),
        CacheAge::Drop(n) => (true, n, true),
    }
}

const fn age_shim(age: obs_source_frame_cache_age) -> (bool, c_long, bool) {
    (age.counted, age.unused_count, age.drop)
}

const fn age_oracle(age: OracleCacheAge) -> (bool, c_long, bool) {
    (age.counted, age.unused_count, age.drop)
}

/// Whether the C is defined for this input: `++unused_count` overflows at
/// `LONG_MAX`.
const fn clean_defined(used: bool, count: c_long) -> bool {
    used || count != c_long::MAX
}

#[track_caller]
fn check_clean(used: bool, count: c_long) {
    let s = age_shim(shim::obs_source_frame_copy_cache_clean(used, count));
    let r = age_core(used, count);
    assert_eq!(s, r, "clean: shim vs core, {used} {count}");
    if clean_defined(used, count) {
        // SAFETY: plain values; defined in C (not LONG_MAX unused).
        let c =
            age_oracle(unsafe { oracle::oracle_obs_source_frame_copy_cache_clean(used, count) });
        assert_eq!(s, c, "clean: shim vs C, {used} {count}");
    }
}

#[track_caller]
fn check_full_range(format: c_int, full: bool) {
    let s = shim::obs_source_frame_copy_full_range(format, full);
    // SAFETY: plain values.
    let c = unsafe { oracle::oracle_obs_source_frame_copy_full_range(format, full) };
    assert_eq!(s, c, "full_range({format}, {full})");
    assert_eq!(core::full_range(format, full), c);
}

#[track_caller]
fn check_full_range2(format: c_int, range: c_int) {
    let s = shim::obs_source_frame_copy_full_range2(format, range);
    // SAFETY: plain values.
    let c = unsafe { oracle::oracle_obs_source_frame_copy_full_range2(format, range) };
    assert_eq!(s, c, "full_range2({format}, {range})");
    assert_eq!(core::full_range2(format, range), c);
}

#[track_caller]
fn check_preload(has: bool, preload: (c_int, u32, u32), frame: (c_int, u32, u32)) {
    let shape = |(format, width, height): (c_int, u32, u32)| obs_source_frame_copy_shape {
        format,
        width,
        height,
    };
    let oshape = |(format, width, height): (c_int, u32, u32)| OracleShape {
        format,
        width,
        height,
    };
    let cshape = |(format, width, height): (c_int, u32, u32)| Shape {
        format,
        width,
        height,
    };
    let s = shim::obs_source_frame_copy_preload_changed(has, shape(preload), shape(frame));
    // SAFETY: plain values.
    let c = unsafe {
        oracle::oracle_obs_source_frame_copy_preload_changed(has, oshape(preload), oshape(frame))
    };
    let r = core::preload_changed(has.then_some(cshape(preload)), cshape(frame));
    assert_eq!(s, c, "preload {has} {preload:?} {frame:?}");
    assert_eq!(r, c, "preload core {has} {preload:?} {frame:?}");
}

// ---------------------------------------------------------------------------

#[test]
fn layout_matches_the_c_header() {
    type L = obs_source_frame_copy_lines;
    type PC = obs_source_frame_plane_copy;
    type P = obs_source_frame_copy_plan;
    type K = obs_source_frame_copy_key;
    type A = obs_source_frame_cache_age;
    type S = obs_source_frame_copy_shape;
    let rust: [usize; 46] = [
        size_of::<L>(),
        align_of::<L>(),
        offset_of!(L, linesize),
        size_of::<PC>(),
        align_of::<PC>(),
        offset_of!(PC, whole),
        offset_of!(PC, bytes),
        offset_of!(PC, rows),
        offset_of!(PC, src_stride),
        offset_of!(PC, dst_stride),
        size_of::<P>(),
        align_of::<P>(),
        offset_of!(P, color_range),
        offset_of!(P, count),
        offset_of!(P, planes),
        size_of::<K>(),
        align_of::<K>(),
        offset_of!(K, width),
        offset_of!(K, height),
        offset_of!(K, convert_type),
        size_of::<A>(),
        align_of::<A>(),
        offset_of!(A, counted),
        offset_of!(A, unused_count),
        offset_of!(A, drop),
        size_of::<S>(),
        align_of::<S>(),
        offset_of!(S, format),
        offset_of!(S, width),
        offset_of!(S, height),
        size_of::<shim::obs_source_frame_cache_step>(),
        usize::try_from(shim::OBS_SOURCE_FRAME_CACHE_KEEP).unwrap(),
        usize::try_from(shim::OBS_SOURCE_FRAME_CACHE_RESET).unwrap(),
        usize::try_from(shim::OBS_SOURCE_FRAME_CACHE_OVERFLOW).unwrap(),
        PLANES,
        MAX_ASYNC_FRAMES,
        usize::try_from(MAX_UNUSED).unwrap(),
        size_of::<c_long>(),
        size_of::<usize>(),
        size_of::<c_int>(),
        size_of::<bool>(),
        size_of::<c_int>(),
        size_of::<c_int>(),
        usize::try_from(R::DEFAULT).unwrap(),
        usize::try_from(R::PARTIAL).unwrap(),
        usize::try_from(R::FULL).unwrap(),
    ];
    for (which, (name, value)) in oracle::LAYOUT.iter().zip(rust).enumerate() {
        // SAFETY: a pure lookup.
        let c = unsafe { oracle::oracle_source_frame_copy_layout(which) };
        assert_eq!(value, c, "{name}");
    }
    // SAFETY: a pure lookup.
    assert_eq!(
        unsafe { oracle::oracle_source_frame_copy_layout(oracle::LAYOUT.len()) },
        usize::MAX
    );
    assert_eq!(size_of::<OracleCopyPlan>(), size_of::<P>());
    assert_eq!(size_of::<OracleCacheAge>(), size_of::<A>());
}

#[test]
fn format_values_match_the_c_header() {
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
    for (which, format) in formats.into_iter().enumerate() {
        // SAFETY: a pure lookup.
        assert_eq!(
            unsafe { oracle::oracle_source_frame_copy_format(which) },
            format
        );
    }
    // SAFETY: a pure lookup.
    assert_eq!(
        unsafe { oracle::oracle_source_frame_copy_format(formats.len()) },
        i32::MIN
    );
}

/// Every C format value.
fn formats() -> Vec<c_int> {
    (-3..=30)
        .chain([i32::MIN, i32::MIN + 1, i32::MAX - 1, i32::MAX])
        .collect()
}

const HEIGHTS: [u32; 16] = [
    0,
    1,
    2,
    3,
    4,
    5,
    1079,
    1080,
    1081,
    0x7fff_ffff,
    0x8000_0000,
    0x8000_0001,
    u32::MAX - 2,
    u32::MAX - 1,
    u32::MAX,
    0x5555_5556,
];

const DST_LINES: [u32; 3] = [1920, 7, u32::MAX];

/// The source line size for relation `rel` against the destination's `d`:
/// equal, smaller, larger, 0, or (with the destination's set to 0 by the
/// caller) any.
fn related(d: u32, rel: u32) -> (u32, u32) {
    match rel {
        0 => (d, d),
        1 => (d.saturating_sub(1), d),
        2 => (d.checked_add(1).unwrap_or(0), d),
        3 => (0, d),
        _ => (d, 0),
    }
}

#[test]
fn plan_exhaustive_over_format_and_line_size_relation() {
    let mut whole = 0u64;
    let mut rows = 0u64;
    let mut none = 0u64;
    for format in formats() {
        for pattern in 0..5u32.pow(4) {
            for height in HEIGHTS {
                for base in DST_LINES {
                    let mut src = [0; PLANES];
                    let mut dst = [0; PLANES];
                    let mut p = pattern;
                    for i in 0..PLANES {
                        // each plane's base differs, so the planes cannot be
                        // read from one another unnoticed
                        let d = base.wrapping_sub(u32::try_from(i).unwrap() * 3);
                        (src[i], dst[i]) = related(d, p % 5);
                        p /= 5;
                    }
                    for full in [false, true] {
                        check_plan(format, full, height, src, dst);
                    }
                    let plan = core::copy_plan(format, true, height, src, dst);
                    if plan.count == 0 {
                        none += 1;
                    }
                    for copy in &plan.planes[..plan.count] {
                        if copy.whole {
                            whole += 1;
                        } else {
                            rows += 1;
                        }
                    }
                }
            }
        }
    }
    assert!(whole > 0 && rows > 0 && none > 0, "{whole} {rows} {none}");
}

/// The line sizes real frames have (video_frame_init()'s widths x 1, 2, 3,
/// 4 and the v210 rule), padded and not, at real heights.
#[test]
fn plan_real_frames() {
    for format in formats() {
        for width in [1u32, 2, 3, 640, 1280, 1281, 1920, 3840] {
            for height in [1u32, 2, 360, 480, 719, 720, 1080, 2160] {
                for pad in [0u32, 1, 32, 64] {
                    let l = [width * 2, width, width.div_ceil(2), width * 4];
                    let padded = l.map(|x| x + pad);
                    check_plan(format, false, height, padded, l);
                    check_plan(format, true, height, l, padded);
                    check_plan(format, false, height, l, l);
                }
            }
        }
    }
}

fn any_u32() -> impl Strategy<Value = u32> {
    prop_oneof![
        Just(0u32),
        Just(1u32),
        Just(u32::MAX),
        Just(u32::MAX - 1),
        Just(0x8000_0000u32),
        0u32..4096,
        any::<u32>(),
    ]
}

fn any_format() -> impl Strategy<Value = c_int> {
    prop_oneof![-3i32..=30, any::<i32>()]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(8192))]

    #[test]
    fn plan_full_range(
        format in any_format(),
        full in any::<bool>(),
        height in any_u32(),
        src in proptest::array::uniform4(any_u32()),
        dst in proptest::array::uniform4(any_u32()),
        equal in proptest::array::uniform4(any::<bool>()),
    ) {
        let mut src = src;
        for i in 0..PLANES {
            if equal[i] {
                src[i] = dst[i];
            }
        }
        check_plan(format, full, height, src, dst);
    }

    #[test]
    fn step_full_range(
        queued in prop_oneof![0usize..=64, any::<usize>()],
        cache in (any_u32(), any_u32(), any::<i32>()),
        frame in (any_u32(), any_u32(), any::<i32>()),
        same in any::<bool>(),
    ) {
        check_step(queued, cache, if same { cache } else { frame });
    }

    #[test]
    fn clean_full_range(used in any::<bool>(), count in any::<c_long>()) {
        check_clean(used, count);
    }

    #[test]
    fn range_full_range(format in any::<i32>(), full in any::<bool>(), range in any::<i32>()) {
        check_full_range(format, full);
        check_full_range2(format, range);
    }

    #[test]
    fn preload_full_range(
        has in any::<bool>(),
        preload in (any_format(), any_u32(), any_u32()),
        frame in (any_format(), any_u32(), any_u32()),
        same in any::<bool>(),
    ) {
        check_preload(has, preload, if same { preload } else { frame });
    }
}

#[test]
fn step_every_queue_length_and_key_field() {
    let ends = [0u32, 1, 0x7fff_ffff, 0x8000_0000, u32::MAX];
    let types = [0, 1, 20, -1, i32::MIN, i32::MAX];
    let queues = (0..=64).chain([usize::MAX - 1, usize::MAX]);
    for queued in queues {
        for &w in &ends {
            for &h in &ends {
                for &t in &types {
                    let cache = (w, h, t);
                    check_step(queued, cache, cache);
                    check_step(queued, cache, (w ^ 1, h, t));
                    check_step(queued, cache, (w, h ^ 1, t));
                    check_step(queued, cache, (w, h, t ^ 1));
                    check_step(queued, cache, (h, w, t));
                }
            }
        }
    }
}

#[test]
fn clean_every_count() {
    for count in -1000..=1000 {
        check_clean(false, count);
        check_clean(true, count);
    }
    for count in [
        c_long::MIN,
        c_long::MIN + 1,
        c_long::MAX - 1,
        c_long::MAX,
        MAX_UNUSED - 1,
        MAX_UNUSED,
    ] {
        check_clean(false, count);
        check_clean(true, count);
    }
}

/// The intentional difference: the shim and the core wrap where the C is
/// undefined; the oracle is not called there.
#[test]
fn clean_long_max_wraps() {
    assert!(!clean_defined(false, c_long::MAX));
    assert_eq!(
        age_shim(shim::obs_source_frame_copy_cache_clean(false, c_long::MAX)),
        (true, c_long::MIN, false)
    );
    assert_eq!(age_core(false, c_long::MAX), (true, c_long::MIN, false));
}

#[test]
fn range_rules_every_format_and_range() {
    let ends = [i32::MIN, i32::MIN + 1, i32::MAX - 1, i32::MAX];
    for format in (-300..=300).chain(ends) {
        for full in [false, true] {
            check_full_range(format, full);
        }
        for range in (-10..=10).chain(ends) {
            check_full_range2(format, range);
        }
    }
}

#[test]
fn preload_every_combination() {
    let formats = [-1, 0, 1, 2, 17, 18, 25, 26, i32::MIN, i32::MAX];
    let sizes = [0u32, 1, 2, 1080, 1920, u32::MAX];
    for has in [false, true] {
        for &pf in &formats {
            for &pw in &sizes {
                for &ph in &sizes {
                    for &nf in &[pf, formats[0], i32::MAX] {
                        for &nw in &[pw, 0, u32::MAX] {
                            for &nh in &[ph, 1, u32::MAX] {
                                check_preload(has, (pf, pw, ph), (nf, nw, nh));
                            }
                        }
                    }
                }
            }
        }
    }
}
