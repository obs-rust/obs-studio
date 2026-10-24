//! Tier 1: `obs_source::frame_copy`, the async frame copy plan and cache
//! decisions of `obs-source.c`.
//!
//! The `steps_*` cases port `test/cmocka/test_source_frame_copy_steps.c`
//! 1:1, under the same case names, in the same order, with the same inputs
//! and expectations.
//!
//! The `edge_*` cases are new: `half()` at its ends; the plan is its pieces
//! for formats -3 to 30; the queue limit over every count up to 64 and the
//! `usize` ends; `clean()` over counts around 0, the limit and the `c_long`
//! ends; the range rules against `is_yuv()`; and proptests over any `i32`
//! format, any `u32` height and line sizes (against a model in `u64`), any
//! keys and queue length, any count and any preload shapes.

use core::ffi::c_long;

use obs_source::frame_copy::{
    CacheAge, CacheStep, CopyPlan, Key, MAX_ASYNC_FRAMES, MAX_UNUSED, PLANES, PlaneCopy, Rows,
    Shape, cache_step, clean, copy_plan, full_range, full_range2, half, is_yuv, plane_copy,
    plane_rows, preload_changed, resolve_range, texture_changed, video_format as F,
    video_range as R,
};
use proptest::prelude::*;

use Rows::{Full as H, Half as HALF};

/// Every `enum video_format` value in order, with the planes the copy makes
/// (the C test's `shapes`).
const SHAPES: [(i32, &str, &[Rows]); 26] = [
    (F::NONE, "NONE", &[H]),
    (F::I420, "I420", &[H, HALF, HALF]),
    (F::NV12, "NV12", &[H, HALF]),
    (F::YVYU, "YVYU", &[H]),
    (F::YUY2, "YUY2", &[H]),
    (F::UYVY, "UYVY", &[H]),
    (F::RGBA, "RGBA", &[H]),
    (F::BGRA, "BGRA", &[H]),
    (F::BGRX, "BGRX", &[H]),
    (F::Y800, "Y800", &[H]),
    (F::I444, "I444", &[H, H, H]),
    (F::BGR3, "BGR3", &[H]),
    (F::I422, "I422", &[H, H, H]),
    (F::I40A, "I40A", &[H, HALF, HALF, H]),
    (F::I42A, "I42A", &[H, H, H, H]),
    (F::YUVA, "YUVA", &[H, H, H, H]),
    (F::AYUV, "AYUV", &[H]),
    (F::I010, "I010", &[H, HALF, HALF]),
    (F::P010, "P010", &[H, HALF]),
    (F::I210, "I210", &[H, H, H]),
    (F::I412, "I412", &[H, H, H]),
    (F::YA2L, "YA2L", &[H, H, H, H]),
    // "Unimplemented": characterized, not endorsed
    (F::P216, "P216", &[]),
    (F::P416, "P416", &[]),
    (F::V210, "V210", &[H]),
    (F::R10L, "R10L", &[H]),
];

/// `format_is_yuv()` per format, in enum order.
const YUV: [bool; 26] = [
    false, true, true, true, true, true, false, false, false, false, true, false, true, true, true,
    true, true, true, true, true, true, true, true, true, true, false,
];

const HEIGHTS: [u32; 12] = [0, 1, 2, 3, 4, 5, 9, 719, 720, 1079, 1080, 1081];

fn rows_of(rule: Rows, height: u32) -> u32 {
    match rule {
        Rows::Half => height.wrapping_add(1) / 2,
        Rows::Full => height,
    }
}

const fn lines4(l0: u32, l1: u32, l2: u32, l3: u32) -> [u32; PLANES] {
    [l0, l1, l2, l3]
}

const fn lines_all(l: u32) -> [u32; PLANES] {
    [l; PLANES]
}

fn size(value: u32) -> usize {
    usize::try_from(value).unwrap()
}

#[track_caller]
#[allow(clippy::too_many_arguments)]
fn check_copy(
    what: &str,
    plane: usize,
    got: &PlaneCopy,
    whole: bool,
    bytes: usize,
    rows: u32,
    src_stride: u32,
    dst_stride: u32,
) {
    assert_eq!(
        *got,
        PlaneCopy {
            whole,
            bytes,
            rows,
            src_stride,
            dst_stride
        },
        "{what} plane {plane}"
    );
}

#[track_caller]
fn check_count(what: &str, plan: &CopyPlan, count: usize) {
    assert_eq!(plan.count, count, "{what}: planes");
}

#[track_caller]
fn check_relation(what: &str, plane: usize, got: &PlaneCopy, s: u32, d: u32, rows: u32) {
    if s == d {
        check_copy(what, plane, got, true, size(d) * size(rows), rows, s, d);
    } else {
        check_copy(what, plane, got, false, size(d.min(s)), rows, s, d);
    }
}

#[track_caller]
fn check_unused_zero(what: &str, plan: &CopyPlan) {
    for (i, copy) in plan.planes.iter().enumerate().skip(plan.count) {
        assert_eq!(*copy, PlaneCopy::default(), "{what} unused plane {i}");
    }
}

const fn key(width: u32, height: u32, convert_type: i32) -> Key {
    Key {
        width,
        height,
        convert_type,
    }
}

const fn shape(format: i32, width: u32, height: u32) -> Shape {
    Shape {
        format,
        width,
        height,
    }
}

/// `struct obs_source_frame_cache_age` as the C test checks it: counted,
/// the count, drop.
fn age_fields(age: CacheAge, unused_count: c_long) -> (bool, c_long, bool) {
    match age {
        CacheAge::InUse => (false, unused_count, false),
        CacheAge::Counted(n) => (true, n, false),
        CacheAge::Drop(n) => (true, n, true),
    }
}

#[track_caller]
fn check_age(what: &str, used: bool, unused_count: c_long, counted: bool, n: c_long, drop: bool) {
    assert_eq!(
        age_fields(clean(used, unused_count), unused_count),
        (counted, n, drop),
        "{what}"
    );
}

// ---------------------------------------------------------------------------
// the C test, 1:1

#[test]
fn steps_constants() {
    assert_eq!(PLANES, 4);
    assert_eq!(MAX_ASYNC_FRAMES, 30);
    assert_eq!(MAX_UNUSED, 5);
    assert_ne!(CacheStep::Keep, CacheStep::Reset);
    assert_ne!(CacheStep::Reset, CacheStep::Overflow);
}

#[test]
fn steps_table_covers_every_format() {
    assert_eq!(SHAPES.len(), usize::try_from(F::R10L + 1).unwrap());
    assert_eq!(YUV.len(), SHAPES.len());
    for (i, (format, _, _)) in SHAPES.iter().enumerate() {
        assert_eq!(usize::try_from(*format).unwrap(), i);
    }
}

#[test]
fn steps_plan_every_format_equal_lines() {
    for (format, name, rules) in SHAPES {
        for height in HEIGHTS {
            let l = lines4(1920, 960, 961, 1921);
            let plan = copy_plan(format, true, height, l, l);
            check_count(name, &plan, rules.len());
            for (i, rule) in rules.iter().enumerate() {
                let rows = rows_of(*rule, height);
                check_copy(
                    name,
                    i,
                    &plan.planes[i],
                    true,
                    size(l[i]) * size(rows),
                    rows,
                    l[i],
                    l[i],
                );
            }
            check_unused_zero(name, &plan);
        }
    }
}

#[test]
fn steps_plan_every_format_padded_source() {
    for (format, name, rules) in SHAPES {
        for height in HEIGHTS {
            let src = lines4(2048, 1024, 1024, 2048);
            let dst = lines4(1920, 960, 960, 1920);
            let plan = copy_plan(format, true, height, src, dst);
            check_count(name, &plan, rules.len());
            for (i, rule) in rules.iter().enumerate() {
                let rows = rows_of(*rule, height);
                check_copy(
                    name,
                    i,
                    &plan.planes[i],
                    false,
                    size(dst[i]),
                    rows,
                    src[i],
                    dst[i],
                );
            }
        }
    }
}

#[test]
fn steps_plan_formats_without_copy() {
    let formats = [-1, F::R10L + 1, 1000, i32::MIN, i32::MAX, F::P216, F::P416];
    for format in formats {
        for height in HEIGHTS {
            let plan = copy_plan(format, false, height, lines_all(64), lines_all(32));
            check_count("no copy", &plan, 0);
            check_unused_zero("no copy", &plan);
            assert!(plan.color_range);
        }
    }
}

#[test]
fn steps_plan_odd_heights() {
    let formats = [F::I420, F::I010, F::NV12, F::P010, F::I40A];
    let h = [1, 3, 5, 719, 1081, 2161];
    let halves = [1, 2, 3, 360, 541, 1081];
    for format in formats {
        for (height, half_rows) in h.into_iter().zip(halves) {
            let plan = copy_plan(format, true, height, lines_all(16), lines_all(16));
            assert_eq!(plan.planes[0].rows, height);
            assert_eq!(plan.planes[1].rows, half_rows);
            if plan.count > 2 {
                assert_eq!(plan.planes[2].rows, half_rows);
            }
            if plan.count > 3 {
                assert_eq!(plan.planes[3].rows, height);
            }
        }
    }
    let plan = copy_plan(F::I422, true, 1081, lines_all(16), lines_all(16));
    assert_eq!(plan.planes[1].rows, 1081);
    assert_eq!(plan.planes[2].rows, 1081);
}

#[test]
fn steps_plan_height_wrap() {
    let plan = copy_plan(F::I420, true, u32::MAX, lines_all(8), lines_all(8));
    check_count("I420 UINT32_MAX", &plan, 3);
    check_copy(
        "I420 UINT32_MAX",
        0,
        &plan.planes[0],
        true,
        8 * size(u32::MAX),
        u32::MAX,
        8,
        8,
    );
    check_copy("I420 UINT32_MAX", 1, &plan.planes[1], true, 0, 0, 8, 8);
    check_copy("I420 UINT32_MAX", 2, &plan.planes[2], true, 0, 0, 8, 8);

    let plan = copy_plan(F::NV12, true, u32::MAX - 1, lines_all(8), lines_all(9));
    check_copy(
        "NV12 UINT32_MAX - 1",
        1,
        &plan.planes[1],
        false,
        8,
        0x7fff_ffff,
        8,
        9,
    );
}

#[test]
fn steps_plan_whole_size() {
    let plan = copy_plan(
        F::RGBA,
        true,
        u32::MAX,
        lines_all(u32::MAX),
        lines_all(u32::MAX),
    );
    check_copy(
        "RGBA max",
        0,
        &plan.planes[0],
        true,
        size(u32::MAX).wrapping_mul(size(u32::MAX)),
        u32::MAX,
        u32::MAX,
        u32::MAX,
    );
}

#[test]
fn steps_plan_each_line_size() {
    let rel = [100, 99, 1, 101, u32::MAX, 0];
    for (format, name, rules) in SHAPES {
        for (p, rule) in rules.iter().enumerate() {
            for r in rel {
                let mut src = lines_all(100);
                let dst = lines_all(100);
                src[p] = r;
                let plan = copy_plan(format, true, 7, src, dst);
                check_count(name, &plan, rules.len());
                for (i, rule_i) in rules.iter().enumerate() {
                    check_relation(
                        name,
                        i,
                        &plan.planes[i],
                        src[i],
                        dst[i],
                        rows_of(*rule_i, 7),
                    );
                }
            }
            let mut dst = lines_all(100);
            dst[p] = 0;
            let plan = copy_plan(format, true, 7, lines_all(100), dst);
            check_copy(
                name,
                p,
                &plan.planes[p],
                false,
                0,
                rows_of(*rule, 7),
                100,
                0,
            );
            let plan = copy_plan(format, true, 7, lines_all(0), lines_all(0));
            check_copy(name, p, &plan.planes[p], true, 0, rows_of(*rule, 7), 0, 0);
        }
    }
}

#[test]
fn steps_plan_line_sizes_beyond_planes() {
    let plan = copy_plan(F::YUVA, true, 2, lines4(1, 2, 3, 4), lines4(1, 2, 3, 5));
    check_count("YUVA", &plan, 4);
    check_copy("YUVA", 3, &plan.planes[3], false, 4, 2, 4, 5);
}

#[test]
fn steps_plan_color_range() {
    for format in -2..=F::R10L + 2 {
        assert!(copy_plan(format, false, 4, lines_all(4), lines_all(4)).color_range);
        assert!(!copy_plan(format, true, 4, lines_all(4), lines_all(4)).color_range);
    }
}

#[test]
fn steps_cache_queue_limit() {
    let same = key(1920, 1080, 2);
    let other = key(1280, 720, 1);
    let limit = MAX_ASYNC_FRAMES;
    assert_eq!(cache_step(0, same, same), CacheStep::Keep);
    assert_eq!(cache_step(limit - 1, same, same), CacheStep::Keep);
    assert_eq!(cache_step(limit - 1, same, other), CacheStep::Reset);
    assert_eq!(cache_step(limit, same, same), CacheStep::Overflow);
    assert_eq!(cache_step(limit, same, other), CacheStep::Overflow);
    assert_eq!(cache_step(limit + 1, same, same), CacheStep::Overflow);
    assert_eq!(cache_step(limit + 1, same, other), CacheStep::Overflow);
    assert_eq!(cache_step(usize::MAX, same, same), CacheStep::Overflow);
}

#[test]
fn steps_cache_texture_changed() {
    let cache = key(1920, 1080, 2);
    assert_eq!(cache_step(3, cache, key(1920, 1080, 2)), CacheStep::Keep);
    assert_eq!(cache_step(3, cache, key(1921, 1080, 2)), CacheStep::Reset);
    assert_eq!(cache_step(3, cache, key(1920, 1079, 2)), CacheStep::Reset);
    assert_eq!(cache_step(3, cache, key(1920, 1080, 3)), CacheStep::Reset);
    assert_eq!(cache_step(3, cache, key(1080, 1920, 2)), CacheStep::Reset);
    assert_eq!(cache_step(0, key(0, 0, 0), key(0, 0, 0)), CacheStep::Keep);
    assert_eq!(
        cache_step(
            0,
            key(u32::MAX, u32::MAX, i32::MIN),
            key(u32::MAX, u32::MAX, i32::MIN)
        ),
        CacheStep::Keep
    );
    assert_eq!(
        cache_step(0, key(1, 1, i32::MAX), key(1, 1, i32::MIN)),
        CacheStep::Reset
    );
}

#[test]
fn steps_cache_clean_counts() {
    check_age("unused 0", false, 0, true, 1, false);
    check_age("unused 1", false, 1, true, 2, false);
    check_age("unused 2", false, 2, true, 3, false);
    check_age("unused 3", false, 3, true, 4, false);
    check_age("unused 4", false, 4, true, 5, true);
    check_age("unused 5", false, 5, true, 6, false);
    check_age("unused 6", false, 6, true, 7, false);
    check_age("unused -1", false, -1, true, 0, false);
    check_age(
        "unused LONG_MIN",
        false,
        c_long::MIN,
        true,
        c_long::MIN + 1,
        false,
    );
    check_age(
        "unused LONG_MAX - 1",
        false,
        c_long::MAX - 1,
        true,
        c_long::MAX,
        false,
    );
}

#[test]
fn steps_cache_clean_used() {
    let counts: [c_long; 9] = [0, 1, 3, 4, 5, 6, -1, c_long::MIN, c_long::MAX];
    for count in counts {
        check_age("used", true, count, false, count, false);
    }
}

#[test]
fn steps_cache_clean_drop_at_limit() {
    let mut count: c_long = 0;
    for call in 1..=MAX_UNUSED {
        let (counted, n, drop) = age_fields(clean(false, count), count);
        assert!(counted);
        assert_eq!(n, call);
        assert_eq!(drop, call == MAX_UNUSED);
        count = n;
    }
}

#[test]
fn steps_full_range_rule() {
    for (format, yuv) in (0..).zip(YUV) {
        assert_eq!(full_range(format, false), !yuv, "format {format}");
        assert!(full_range(format, true));
    }
    for format in [-1, F::R10L + 1, 1000, i32::MIN, i32::MAX] {
        assert!(full_range(format, false));
        assert!(full_range(format, true));
    }
}

#[test]
fn steps_full_range2_rule() {
    for (format, yuv) in (0..).zip(YUV) {
        assert_eq!(full_range2(format, R::DEFAULT), !yuv, "format {format}");
        assert!(!full_range2(format, R::PARTIAL));
        assert!(full_range2(format, R::FULL));
        assert!(!full_range2(format, R::FULL + 1));
        assert!(!full_range2(format, -1));
        assert!(!full_range2(format, i32::MIN));
        assert!(!full_range2(format, i32::MAX));
    }
    for format in [-1, F::R10L + 1, i32::MIN, i32::MAX] {
        assert!(full_range2(format, R::DEFAULT));
        assert!(!full_range2(format, R::PARTIAL));
        assert!(full_range2(format, R::FULL));
    }
}

#[test]
fn steps_preload_changed() {
    let p = shape(F::NV12, 1920, 1080);
    assert!(preload_changed(None, p));
    assert!(preload_changed(None, shape(0, 0, 0)));
    assert!(!preload_changed(Some(p), p));
    assert!(preload_changed(Some(p), shape(F::NV12, 1921, 1080)));
    assert!(preload_changed(Some(p), shape(F::NV12, 1920, 1081)));
    assert!(preload_changed(Some(p), shape(F::I420, 1920, 1080)));
    assert!(preload_changed(Some(p), shape(F::NV12, 1080, 1920)));
    assert!(preload_changed(
        Some(shape(F::P010, 64, 64)),
        shape(F::I010, 64, 64)
    ));
    assert!(!preload_changed(
        Some(shape(i32::MIN, u32::MAX, 0)),
        shape(i32::MIN, u32::MAX, 0)
    ));
}

// ---------------------------------------------------------------------------
// new edge cases

#[test]
fn edge_half() {
    assert_eq!(half(0), 0);
    assert_eq!(half(1), 1);
    assert_eq!(half(2), 1);
    assert_eq!(half(3), 2);
    assert_eq!(half(u32::MAX - 1), 0x7fff_ffff);
    assert_eq!(half(u32::MAX), 0);
}

#[test]
fn edge_plane_copy() {
    assert_eq!(
        plane_copy(3, 7, 7),
        PlaneCopy {
            whole: true,
            bytes: 21,
            rows: 3,
            src_stride: 7,
            dst_stride: 7
        }
    );
    assert_eq!(
        plane_copy(3, 8, 7),
        PlaneCopy {
            whole: false,
            bytes: 7,
            rows: 3,
            src_stride: 8,
            dst_stride: 7
        }
    );
    assert_eq!(
        plane_copy(3, 6, 7),
        PlaneCopy {
            whole: false,
            bytes: 6,
            rows: 3,
            src_stride: 6,
            dst_stride: 7
        }
    );
    assert_eq!(
        plane_copy(0, 0, 0),
        PlaneCopy {
            whole: true,
            ..PlaneCopy::default()
        }
    );
}

/// The plan is `plane_rows()` and `plane_copy()` for every format around
/// the enum, and the range flag.
#[test]
fn edge_plan_is_its_pieces() {
    let lines = [
        (lines4(1, 2, 3, 4), lines4(1, 3, 3, 0)),
        (lines_all(0), lines_all(u32::MAX)),
        (lines4(7, 7, 7, 7), lines4(7, 7, 7, 7)),
    ];
    for format in -3..=30 {
        for height in [0, 1, 2, 3, 1081, u32::MAX - 1, u32::MAX] {
            for (src, dst) in lines {
                for full in [false, true] {
                    let plan = copy_plan(format, full, height, src, dst);
                    let rules = plane_rows(format);
                    assert_eq!(plan.color_range, !full);
                    assert_eq!(plan.count, rules.len());
                    for (i, copy) in plan.planes.iter().enumerate() {
                        let want = rules.get(i).map_or(PlaneCopy::default(), |rule| {
                            plane_copy(rows_of(*rule, height), src[i], dst[i])
                        });
                        assert_eq!(*copy, want, "format {format} height {height} plane {i}");
                    }
                }
            }
        }
    }
}

#[test]
fn edge_cache_step_every_queue_length() {
    let same = key(16, 9, 1);
    let other = key(16, 9, 2);
    for queued in 0..=64 {
        let full = queued >= MAX_ASYNC_FRAMES;
        assert_eq!(
            cache_step(queued, same, same),
            if full {
                CacheStep::Overflow
            } else {
                CacheStep::Keep
            }
        );
        assert_eq!(
            cache_step(queued, same, other),
            if full {
                CacheStep::Overflow
            } else {
                CacheStep::Reset
            }
        );
    }
    assert_eq!(cache_step(usize::MAX, same, other), CacheStep::Overflow);
    assert!(!texture_changed(same, same));
    assert!(texture_changed(same, other));
}

#[test]
fn edge_clean_counts() {
    for count in -10..=10 {
        let next = count + 1;
        let want = if next == MAX_UNUSED {
            CacheAge::Drop(next)
        } else {
            CacheAge::Counted(next)
        };
        assert_eq!(clean(false, count), want, "count {count}");
        assert_eq!(clean(true, count), CacheAge::InUse);
    }
    // The C's ++ would overflow at LONG_MAX: no entry gets there (counts
    // start at 0 and the entry is erased at 5); the port wraps.
    assert_eq!(clean(false, c_long::MAX), CacheAge::Counted(c_long::MIN));
}

#[test]
fn edge_range_rules() {
    for format in -3..=40 {
        let yuv = is_yuv(format);
        assert_eq!(
            yuv,
            usize::try_from(format)
                .ok()
                .and_then(|i| YUV.get(i))
                .copied()
                .unwrap_or(false)
        );
        assert_eq!(full_range(format, false), !yuv);
        assert!(full_range(format, true));
        assert_eq!(
            resolve_range(format, R::DEFAULT),
            if yuv { R::PARTIAL } else { R::FULL }
        );
        for range in -3..=5 {
            if range != R::DEFAULT {
                assert_eq!(resolve_range(format, range), range);
            }
            assert_eq!(
                full_range2(format, range),
                resolve_range(format, range) == R::FULL
            );
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

fn any_format() -> impl Strategy<Value = i32> {
    prop_oneof![-3i32..=30, any::<i32>()]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4000))]

    /// Any format outside the enum copies no plane, at any range, height
    /// and line sizes.
    #[test]
    fn edge_outside_the_enum_copies_nothing(
        format in prop_oneof![i32::MIN..0, 26i32..=i32::MAX],
        full in any::<bool>(),
        height in any_u32(),
        src in proptest::array::uniform4(any_u32()),
        dst in proptest::array::uniform4(any_u32()),
    ) {
        let plan = copy_plan(format, full, height, src, dst);
        prop_assert_eq!(plan.count, 0);
        prop_assert_eq!(plan.planes, [PlaneCopy::default(); PLANES]);
        prop_assert_eq!(plan.color_range, !full);
    }

    /// Each plane against a model in u64: the rows (height + 1 reduced
    /// modulo 2^32, then halved), one copy of d * rows exactly when the line sizes are
    /// equal (the product fits a 64-bit usize), else rows of the smaller.
    #[test]
    fn edge_plan_against_a_u64_model(
        format in any_format(),
        height in any_u32(),
        src in proptest::array::uniform4(any_u32()),
        dst in proptest::array::uniform4(any_u32()),
    ) {
        let plan = copy_plan(format, true, height, src, dst);
        let rules = plane_rows(format);
        prop_assert_eq!(plan.count, rules.len());
        for (i, rule) in rules.iter().enumerate() {
            let rows = match rule {
                Rows::Full => u64::from(height),
                Rows::Half => ((u64::from(height) + 1) % (1 << 32)) / 2,
            };
            let rows32 = u32::try_from(rows).unwrap();
            let (s, d) = (u64::from(src[i]), u64::from(dst[i]));
            let copy = plan.planes[i];
            prop_assert_eq!(copy.rows, rows32);
            prop_assert_eq!(copy.src_stride, src[i]);
            prop_assert_eq!(copy.dst_stride, dst[i]);
            prop_assert_eq!(copy.whole, s == d);
            let bytes = if s == d { d * rows } else { d.min(s) };
            prop_assert_eq!(u64::try_from(copy.bytes).unwrap(), bytes);
        }
    }

    /// The queue limit comes first, then any difference in the keys.
    #[test]
    fn edge_cache_step_model(
        queued in prop_oneof![0usize..=64, any::<usize>()],
        cw in any_u32(), ch in any_u32(), ct in any::<i32>(),
        fw in any_u32(), fh in any_u32(), ft in any::<i32>(),
        same in any::<bool>(),
    ) {
        let cache = key(cw, ch, ct);
        let frame = if same { cache } else { key(fw, fh, ft) };
        let want = if queued >= 30 {
            CacheStep::Overflow
        } else if cw != frame.width || ch != frame.height || ct != frame.convert_type {
            CacheStep::Reset
        } else {
            CacheStep::Keep
        };
        prop_assert_eq!(cache_step(queued, cache, frame), want);
    }

    /// clean() over any count.
    #[test]
    fn edge_clean_model(used in any::<bool>(), count in any::<c_long>()) {
        let got = clean(used, count);
        if used {
            prop_assert_eq!(got, CacheAge::InUse);
        } else {
            let next = count.wrapping_add(1);
            prop_assert_eq!(got, if next == 5 { CacheAge::Drop(next) } else { CacheAge::Counted(next) });
        }
    }

    /// preload_changed() over any shapes.
    #[test]
    fn edge_preload_model(
        has in any::<bool>(),
        pf in any_format(), pw in any_u32(), ph in any_u32(),
        nf in any_format(), nw in any_u32(), nh in any_u32(),
        same in any::<bool>(),
    ) {
        let preload = shape(pf, pw, ph);
        let frame = if same { preload } else { shape(nf, nw, nh) };
        let want = !has || preload != frame;
        prop_assert_eq!(preload_changed(has.then_some(preload), frame), want);
    }
}
