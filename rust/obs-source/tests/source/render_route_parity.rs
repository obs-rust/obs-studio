//! Tier 3: the `obs_source_render_route_*` shims against the original C
//! (`obs-c-oracle`, `oracle/source_render_route.c`: the unmodified
//! `libobs/obs-source-render-route.c.h` with its functions renamed).
//!
//! Every function takes and returns plain values, so a run compares the
//! bytes of the two results: the C's `bool` as the byte it returns, the
//! enums as their value. `obs_source_render_route_tech_name()` returns a
//! pointer: the given one must come back itself on both sides, and NULL
//! must give `"Draw"` on both (the two literals live at different
//! addresses, which `gs_effect_get_technique()` never compares).
//!
//! Runs:
//! - the enum values, flags and type sizes against the C headers;
//! - types, direct render choices, spaces, formats and sizes over every
//!   value from -70000 to 70000, every 65521st `i32` over the whole range
//!   and the ends of `i32`;
//! - output flags: all 65,536 low 16-bit patterns, each with the high bits
//!   clear and set, for every rule that reads flags;
//! - can_bypass(): every wiring, 14 direct render choices, every pair of
//!   65 flag patterns and every pair of 12 spaces (17,035,200 inputs), then
//!   every parent flag pattern above for four filter flags;
//! - every draw step value from 0 to 70000 and the ends of `u32`, both ways;
//! - the decisions of `source_render_video()`,
//!   `obs_source_process_filter_begin_with_color_space()`,
//!   `obs_source_process_filter_tech_end()` and
//!   `obs_source_skip_video_filter()` in their order, reads included, one
//!   copy on each side, over every combination of their inputs;
//! - proptests (20,000 cases each) over full-range values.
//!
//! None of these functions divides or does arithmetic: there is no input on
//! which the C could trap (MSVC's `_udiv128` or `INT_MIN / -1`), so no input
//! is skipped on any target.
//!
//! No intentional differences beyond the `"Draw"` literal's address.

use std::ffi::CStr;
use std::os::raw::{c_char, c_int};

use obs_c_oracle::source_render_route as oracle;
use obs_source::ffi::source_render_route as shim;
use obs_source::render_route::{
    ALLOW_DIRECT_RENDERING, Draw, FLAG_ASYNC, FLAG_AUDIO, FLAG_CAP_DISABLED, FLAG_CUSTOM_DRAW,
    FLAG_SRGB, FLAG_VIDEO, GS_CS_709_EXTENDED, GS_CS_709_SCRGB, GS_CS_SRGB, GS_CS_SRGB_16F,
    GS_RG16, NO_DIRECT_RENDERING, TYPE_FILTER, TYPE_INPUT, TYPE_SCENE, TYPE_TRANSITION,
};
use proptest::prelude::*;

#[test]
fn values_match_the_c_headers() {
    let mut rust: Vec<i64> = vec![
        TYPE_INPUT.into(),
        TYPE_FILTER.into(),
        TYPE_TRANSITION.into(),
        TYPE_SCENE.into(),
        FLAG_VIDEO.into(),
        FLAG_AUDIO.into(),
        FLAG_ASYNC.into(),
        FLAG_CUSTOM_DRAW.into(),
        FLAG_CAP_DISABLED.into(),
        FLAG_SRGB.into(),
        NO_DIRECT_RENDERING.into(),
        ALLOW_DIRECT_RENDERING.into(),
        GS_CS_SRGB.into(),
        GS_CS_SRGB_16F.into(),
        GS_CS_709_EXTENDED.into(),
        GS_CS_709_SCRGB.into(),
        GS_RG16.into(),
        0,
        1,
        2,
    ];
    rust.extend(Draw::ALL.iter().map(|d| i64::from(d.value())));
    rust.extend([0, 1]);
    // the four libobs enums, the three route enums, int, bool, uint32_t
    rust.extend([4, 4, 4, 4, 4, 4, 4, 4, 1, 4]);
    assert_eq!(rust.len(), oracle::VALUE_NAMES.len());
    for (which, (name, value)) in oracle::VALUE_NAMES.iter().zip(rust).enumerate() {
        // SAFETY: a pure lookup.
        let c = unsafe { oracle::oracle_source_render_route_value(which) };
        assert_eq!(value, c, "{name}");
    }
    // SAFETY: a pure lookup, one past the end.
    let past = unsafe { oracle::oracle_source_render_route_value(oracle::VALUE_NAMES.len()) };
    assert_eq!(past, i64::MIN, "the list has as many values as names");
}

// --------------------------------------------------------------------------
// One call on each side, compared

fn b(v: bool) -> u8 {
    u8::from(v)
}

fn check_filter(t: bool, p: bool) {
    // SAFETY: plain values.
    let c = unsafe { oracle::oracle_obs_source_render_route_filter(t, p) };
    assert_eq!(
        shim::obs_source_render_route_filter(t, p),
        c,
        "filter {t} {p}"
    );
}

fn check_type(ty: c_int) {
    // SAFETY: plain values.
    unsafe {
        assert_eq!(
            b(shim::obs_source_render_route_video_reads_flags(ty)),
            oracle::oracle_obs_source_render_route_video_reads_flags(ty),
            "video_reads_flags {ty}"
        );
        assert_eq!(
            b(shim::obs_source_render_route_async_reads_flags(ty)),
            oracle::oracle_obs_source_render_route_async_reads_flags(ty),
            "async_reads_flags {ty}"
        );
    }
}

fn check_type_flags(ty: c_int, flags: u32) {
    // SAFETY: plain values.
    unsafe {
        assert_eq!(
            b(shim::obs_source_render_route_video_none(ty, flags)),
            oracle::oracle_obs_source_render_route_video_none(ty, flags),
            "video_none {ty} {flags:#x}"
        );
        assert_eq!(
            b(shim::obs_source_render_route_async_reads_rendering_filter(
                ty, flags
            )),
            oracle::oracle_obs_source_render_route_async_reads_rendering_filter(ty, flags),
            "async_reads_rendering_filter {ty} {flags:#x}"
        );
        for rf in [false, true] {
            assert_eq!(
                b(shim::obs_source_render_route_async_update(ty, flags, rf)),
                oracle::oracle_obs_source_render_route_async_update(ty, flags, rf),
                "async_update {ty} {flags:#x} {rf}"
            );
        }
    }
}

fn check_flags(flags: u32) {
    // SAFETY: plain values.
    unsafe {
        assert_eq!(
            b(shim::obs_source_render_route_main_custom_draw(flags)),
            oracle::oracle_obs_source_render_route_main_custom_draw(flags)
        );
        assert_eq!(
            b(shim::obs_source_render_route_main_srgb_aware(flags)),
            oracle::oracle_obs_source_render_route_main_srgb_aware(flags)
        );
        assert_eq!(
            b(shim::obs_source_render_route_tech_end_linear_srgb(flags)),
            oracle::oracle_obs_source_render_route_tech_end_linear_srgb(flags)
        );
        for same in [false, true] {
            assert_eq!(
                shim::obs_source_render_route_filter_render(same, flags),
                oracle::oracle_obs_source_render_route_filter_render(same, flags),
                "filter_render {same} {flags:#x}"
            );
            assert_eq!(
                shim::obs_source_render_route_skip(same, flags),
                oracle::oracle_obs_source_render_route_skip(same, flags),
                "skip {same} {flags:#x}"
            );
            for filters in [false, true] {
                assert_eq!(
                    b(shim::obs_source_render_route_main_default_effect(
                        flags, same, filters
                    )),
                    oracle::oracle_obs_source_render_route_main_default_effect(
                        flags, same, filters
                    ),
                    "main_default_effect {flags:#x} {same} {filters}"
                );
            }
        }
    }
}

fn check_bypass(same: bool, allow: c_int, ff: u32, pf: u32, space: c_int, current: c_int) {
    // SAFETY: plain values.
    unsafe {
        assert_eq!(
            b(shim::obs_source_render_route_bypass(
                same, allow, ff, pf, space, current
            )),
            oracle::oracle_obs_source_render_route_bypass(same, allow, ff, pf, space, current),
            "bypass {same} {allow} {ff:#x} {pf:#x} {space} {current}"
        );
    }
}

fn check_bypass_reads(same: bool, allow: c_int, ff: u32, pf: u32) {
    // SAFETY: plain values.
    unsafe {
        assert_eq!(
            b(shim::obs_source_render_route_bypass_reads_space(
                same, allow, ff, pf
            )),
            oracle::oracle_obs_source_render_route_bypass_reads_space(same, allow, ff, pf),
            "bypass_reads_space {same} {allow} {ff:#x} {pf:#x}"
        );
    }
}

fn check_ints(x: c_int, y: c_int) {
    // SAFETY: plain values.
    unsafe {
        assert_eq!(
            b(shim::obs_source_render_route_size_skip(x, y)),
            oracle::oracle_obs_source_render_route_size_skip(x, y),
            "size_skip {x} {y}"
        );
        assert_eq!(
            b(shim::obs_source_render_route_texrender_stale(x, y)),
            oracle::oracle_obs_source_render_route_texrender_stale(x, y),
            "texrender_stale {x} {y}"
        );
    }
}

fn check_step(step: u32) {
    for v in [false, true] {
        // SAFETY: plain values.
        let c = unsafe { oracle::oracle_obs_source_render_route_draw_next(step, v) };
        assert_eq!(
            shim::obs_source_render_route_draw_next(step, v),
            c,
            "draw_next {step} {v}"
        );
    }
}

/// Every value from -70000 to 70000, every 65521st `i32` over the whole
/// range and the ends.
fn sweep_ints() -> Vec<c_int> {
    let mut all: Vec<c_int> = (-70_000..=70_000).collect();
    all.extend((i32::MIN..=i32::MAX).step_by(65_521));
    all.extend((0..8).map(|i| i32::MIN + i));
    all.extend((0..8).map(|i| i32::MAX - i));
    all
}

/// The low 16-bit patterns, each with the high bits clear and set.
fn every_low_flags() -> impl Iterator<Item = u32> {
    (0u32..=0xffff).flat_map(|low| [low, low | 0xffff_0000])
}

/// Every combination of the six bits the C test combines, then u32::MAX.
fn flag_combos() -> Vec<u32> {
    let bits = [
        FLAG_VIDEO,
        FLAG_ASYNC,
        FLAG_CUSTOM_DRAW,
        FLAG_SRGB,
        FLAG_AUDIO,
        FLAG_CAP_DISABLED,
    ];
    let mut all: Vec<u32> = (0u32..64)
        .map(|combo| {
            bits.iter()
                .enumerate()
                .filter(|(bit, _)| combo & (1 << bit) != 0)
                .fold(0, |flags, (_, flag)| flags | flag)
        })
        .collect();
    all.push(u32::MAX);
    all
}

const TYPES: [c_int; 10] = [0, 1, 2, 3, -1, 4, 5, 99, i32::MIN, i32::MAX];
const SPACES: [c_int; 12] = [0, 1, 2, 3, -2, -1, 4, 5, 99, -99, i32::MIN, i32::MAX];
const ALLOW: [c_int; 14] = [
    0,
    1,
    2,
    3,
    -1,
    -2,
    99,
    256,
    257,
    0x1_0001,
    i32::MIN,
    i32::MIN + 1,
    i32::MAX,
    i32::MAX - 1,
];

#[test]
fn bools_every_input() {
    for t in [false, true] {
        for p in [false, true] {
            check_filter(t, p);
            // SAFETY: plain values.
            unsafe {
                assert_eq!(
                    b(shim::obs_source_render_route_inactive(t, p)),
                    oracle::oracle_obs_source_render_route_inactive(t, p)
                );
            }
        }
        // SAFETY: plain values.
        unsafe {
            assert_eq!(
                b(shim::obs_source_render_route_reads_enabled(t)),
                oracle::oracle_obs_source_render_route_reads_enabled(t)
            );
            assert_eq!(
                b(shim::obs_source_render_route_main_reads_filters(t)),
                oracle::oracle_obs_source_render_route_main_reads_filters(t)
            );
            assert_eq!(
                shim::obs_source_render_route_draw(t),
                oracle::oracle_obs_source_render_route_draw(t)
            );
            assert_eq!(
                shim::obs_source_render_route_tech_end(t),
                oracle::oracle_obs_source_render_route_tech_end(t)
            );
        }
    }
}

#[test]
fn types_sweep() {
    for ty in sweep_ints() {
        check_type(ty);
        for flags in [0, FLAG_VIDEO, FLAG_ASYNC, FLAG_ASYNC | FLAG_VIDEO, u32::MAX] {
            check_type_flags(ty, flags);
        }
    }
}

#[test]
fn type_and_flag_patterns() {
    for flags in every_low_flags() {
        for ty in TYPES {
            check_type_flags(ty, flags);
        }
        check_flags(flags);
    }
}

#[test]
fn bypass_every_combination() {
    let combos = flag_combos();
    for same in [false, true] {
        for allow in ALLOW {
            for &ff in &combos {
                for &pf in &combos {
                    check_bypass_reads(same, allow, ff, pf);
                    for s in SPACES {
                        for cs in SPACES {
                            check_bypass(same, allow, ff, pf, s, cs);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn bypass_every_parent_pattern_and_value() {
    for pf in every_low_flags() {
        for ff in [0, FLAG_SRGB, u32::MAX, u32::MAX ^ FLAG_SRGB] {
            check_bypass_reads(true, 1, ff, pf);
            check_bypass(true, 1, ff, pf, 0, 0);
            check_bypass(true, 1, ff, pf, 2, 3);
        }
    }
    for v in sweep_ints() {
        check_bypass_reads(true, v, 0, FLAG_VIDEO);
        check_bypass(true, v, 0, FLAG_VIDEO, 0, 0);
        check_bypass(true, 1, 0, FLAG_VIDEO, v, v);
        check_bypass(true, 1, 0, FLAG_VIDEO, v, 0);
        check_bypass(true, 1, 0, FLAG_VIDEO, 2, v);
    }
}

#[test]
fn sizes_and_formats() {
    let ints = sweep_ints();
    let few = [0, 1, -1, 2, 22, 23, 1920, i32::MIN, i32::MAX];
    for &x in &ints {
        for &y in &few {
            check_ints(x, y);
            check_ints(y, x);
        }
        check_ints(x, x);
    }
    for x in -64..=64 {
        for y in -64..=64 {
            check_ints(x, y);
        }
    }
}

#[test]
fn every_step_value() {
    for step in 0u32..=70_000 {
        check_step(step);
    }
    for step in (0..64).map(|i| u32::MAX - i) {
        check_step(step);
    }
    for step in (0..64).map(|i| 0x8000_0000 + i) {
        check_step(step);
    }
}

#[test]
fn tech_name_pointers() {
    static NAMES: [&CStr; 4] = [c"Draw", c"DrawCustom", c"", c"Draw\u{1}"];
    for name in NAMES {
        let p = name.as_ptr();
        // SAFETY: the pointer is passed back, never read.
        let c = unsafe { oracle::oracle_obs_source_render_route_tech_name(p) };
        assert_eq!(c, p);
        assert_eq!(shim::obs_source_render_route_tech_name(p), p);
    }
    // A pointer that is not a string is passed back too, unread.
    let odd = std::ptr::dangling::<c_char>();
    // SAFETY: as above.
    unsafe {
        assert_eq!(oracle::oracle_obs_source_render_route_tech_name(odd), odd);
    }
    assert_eq!(shim::obs_source_render_route_tech_name(odd), odd);
    // SAFETY: NULL gives a static string on each side.
    let (c, r) = unsafe {
        (
            CStr::from_ptr(oracle::oracle_obs_source_render_route_tech_name(
                std::ptr::null(),
            )),
            CStr::from_ptr(shim::obs_source_render_route_tech_name(std::ptr::null())),
        )
    };
    assert_eq!(c, r);
    assert_eq!(r, c"Draw");
}

// --------------------------------------------------------------------------
// The decisions of each caller in its order, one copy on each side

/// The functions one side links.
struct Side {
    filter: fn(bool, bool) -> u32,
    video_reads_flags: fn(c_int) -> u8,
    video_none: fn(c_int, u32) -> u8,
    async_reads_flags: fn(c_int) -> u8,
    async_reads_rendering_filter: fn(c_int, u32) -> u8,
    async_update: fn(c_int, u32, bool) -> u8,
    reads_enabled: fn(bool) -> u8,
    inactive: fn(bool, bool) -> u8,
    draw: fn(bool) -> u32,
    draw_next: fn(u32, bool) -> u32,
    main_custom_draw: fn(u32) -> u8,
    main_srgb_aware: fn(u32) -> u8,
    main_reads_filters: fn(bool) -> u8,
    main_default_effect: fn(u32, bool, bool) -> u8,
    bypass_reads_space: fn(bool, c_int, u32, u32) -> u8,
    bypass: fn(bool, c_int, u32, u32, c_int, c_int) -> u8,
    size_skip: fn(c_int, c_int) -> u8,
    texrender_stale: fn(c_int, c_int) -> u8,
    filter_render: fn(bool, u32) -> u32,
    tech_end_linear_srgb: fn(u32) -> u8,
    tech_end: fn(bool) -> u32,
    skip: fn(bool, u32) -> u32,
}

// Each oracle function wrapped by hand: the arities differ.
fn c_side() -> Side {
    // SAFETY (each closure): plain values in and out.
    Side {
        filter: |t, p| unsafe { oracle::oracle_obs_source_render_route_filter(t, p) },
        video_reads_flags: |t| unsafe {
            oracle::oracle_obs_source_render_route_video_reads_flags(t)
        },
        video_none: |t, f| unsafe { oracle::oracle_obs_source_render_route_video_none(t, f) },
        async_reads_flags: |t| unsafe {
            oracle::oracle_obs_source_render_route_async_reads_flags(t)
        },
        async_reads_rendering_filter: |t, f| unsafe {
            oracle::oracle_obs_source_render_route_async_reads_rendering_filter(t, f)
        },
        async_update: |t, f, r| unsafe {
            oracle::oracle_obs_source_render_route_async_update(t, f, r)
        },
        reads_enabled: |d| unsafe { oracle::oracle_obs_source_render_route_reads_enabled(d) },
        inactive: |d, e| unsafe { oracle::oracle_obs_source_render_route_inactive(d, e) },
        draw: |f| unsafe { oracle::oracle_obs_source_render_route_draw(f) },
        draw_next: |s, v| unsafe { oracle::oracle_obs_source_render_route_draw_next(s, v) },
        main_custom_draw: |f| unsafe { oracle::oracle_obs_source_render_route_main_custom_draw(f) },
        main_srgb_aware: |f| unsafe { oracle::oracle_obs_source_render_route_main_srgb_aware(f) },
        main_reads_filters: |p| unsafe {
            oracle::oracle_obs_source_render_route_main_reads_filters(p)
        },
        main_default_effect: |f, p, h| unsafe {
            oracle::oracle_obs_source_render_route_main_default_effect(f, p, h)
        },
        bypass_reads_space: |s, a, ff, pf| unsafe {
            oracle::oracle_obs_source_render_route_bypass_reads_space(s, a, ff, pf)
        },
        bypass: |s, a, ff, pf, sp, cs| unsafe {
            oracle::oracle_obs_source_render_route_bypass(s, a, ff, pf, sp, cs)
        },
        size_skip: |x, y| unsafe { oracle::oracle_obs_source_render_route_size_skip(x, y) },
        texrender_stale: |x, y| unsafe {
            oracle::oracle_obs_source_render_route_texrender_stale(x, y)
        },
        filter_render: |s, f| unsafe { oracle::oracle_obs_source_render_route_filter_render(s, f) },
        tech_end_linear_srgb: |f| unsafe {
            oracle::oracle_obs_source_render_route_tech_end_linear_srgb(f)
        },
        tech_end: |b| unsafe { oracle::oracle_obs_source_render_route_tech_end(b) },
        skip: |s, f| unsafe { oracle::oracle_obs_source_render_route_skip(s, f) },
    }
}

fn rust_side() -> Side {
    Side {
        filter: |t, p| shim::obs_source_render_route_filter(t, p),
        video_reads_flags: |t| b(shim::obs_source_render_route_video_reads_flags(t)),
        video_none: |t, f| b(shim::obs_source_render_route_video_none(t, f)),
        async_reads_flags: |t| b(shim::obs_source_render_route_async_reads_flags(t)),
        async_reads_rendering_filter: |t, f| {
            b(shim::obs_source_render_route_async_reads_rendering_filter(
                t, f,
            ))
        },
        async_update: |t, f, r| b(shim::obs_source_render_route_async_update(t, f, r)),
        reads_enabled: |d| b(shim::obs_source_render_route_reads_enabled(d)),
        inactive: |d, e| b(shim::obs_source_render_route_inactive(d, e)),
        draw: |f| shim::obs_source_render_route_draw(f),
        draw_next: |s, v| shim::obs_source_render_route_draw_next(s, v),
        main_custom_draw: |f| b(shim::obs_source_render_route_main_custom_draw(f)),
        main_srgb_aware: |f| b(shim::obs_source_render_route_main_srgb_aware(f)),
        main_reads_filters: |p| b(shim::obs_source_render_route_main_reads_filters(p)),
        main_default_effect: |f, p, h| {
            b(shim::obs_source_render_route_main_default_effect(f, p, h))
        },
        bypass_reads_space: |s, a, ff, pf| {
            b(shim::obs_source_render_route_bypass_reads_space(
                s, a, ff, pf,
            ))
        },
        bypass: |s, a, ff, pf, sp, cs| {
            b(shim::obs_source_render_route_bypass(s, a, ff, pf, sp, cs))
        },
        size_skip: |x, y| b(shim::obs_source_render_route_size_skip(x, y)),
        texrender_stale: |x, y| b(shim::obs_source_render_route_texrender_stale(x, y)),
        filter_render: |s, f| shim::obs_source_render_route_filter_render(s, f),
        tech_end_linear_srgb: |f| b(shim::obs_source_render_route_tech_end_linear_srgb(f)),
        tech_end: |b| shim::obs_source_render_route_tech_end(b),
        skip: |s, f| shim::obs_source_render_route_skip(s, f),
    }
}

/// A source as `source_render_video()` and `obs_source_main_render()` see
/// it, and what they read.
#[derive(Clone, Copy, Debug)]
struct Source {
    source_type: c_int,
    flags: u32,
    has_data: bool,
    enabled: bool,
    rendering_filter: bool,
    has_filters: bool,
    has_filter_parent: bool,
    video_render: bool,
    filter_target: bool,
    deinterlacing: bool,
}

/// What C reads, calls and draws, in order: a trace of the decisions.
fn trace_render_video(side: &Side, s: Source) -> Vec<u32> {
    let mut t = Vec::new();
    let read_ty = s.source_type;
    let reads = (side.video_reads_flags)(read_ty);
    t.push(u32::from(reads));
    let flags = if reads != 0 { s.flags } else { 0 };
    if (side.video_none)(read_ty, flags) != 0 {
        t.push(100);
        return t;
    }
    let reads = (side.async_reads_flags)(s.source_type);
    t.push(u32::from(reads));
    let flags = if reads != 0 { s.flags } else { 0 };
    let reads_rf = (side.async_reads_rendering_filter)(s.source_type, flags);
    t.push(u32::from(reads_rf));
    let rf = reads_rf != 0 && s.rendering_filter;
    t.push(u32::from((side.async_update)(s.source_type, flags, rf)));
    let reads_enabled = (side.reads_enabled)(s.has_data);
    t.push(u32::from(reads_enabled));
    if (side.inactive)(s.has_data, reads_enabled != 0 && s.enabled) != 0 {
        t.push(101);
        return t;
    }
    let mut route = (side.draw)(s.has_filters);
    loop {
        t.push(route);
        let value = match Draw::from_value(route) {
            Some(Draw::ReadRenderingFilter) => s.rendering_filter,
            Some(Draw::ReadVideoRender) => s.video_render,
            Some(Draw::ReadFilterTarget) => s.filter_target,
            Some(Draw::ReadDeinterlacing) => s.deinterlacing,
            _ => break,
        };
        route = (side.draw_next)(route, value);
    }
    if Draw::from_value(route) == Some(Draw::Main) {
        // obs_source_main_render()
        t.push(u32::from((side.main_custom_draw)(s.flags)));
        t.push(u32::from((side.main_srgb_aware)(s.flags)));
        let reads = (side.main_reads_filters)(s.has_filter_parent);
        t.push(u32::from(reads));
        t.push(u32::from((side.main_default_effect)(
            s.flags,
            s.has_filter_parent,
            reads != 0 && s.has_filters,
        )));
    }
    t
}

#[test]
fn render_video_decisions_every_source() {
    let (c, r) = (c_side(), rust_side());
    let mut count = 0u64;
    for source_type in TYPES {
        for flags in flag_combos() {
            for m in 0u32..256 {
                let s = Source {
                    source_type,
                    flags,
                    has_data: m & 1 != 0,
                    enabled: m & 2 != 0,
                    rendering_filter: m & 4 != 0,
                    has_filters: m & 8 != 0,
                    has_filter_parent: m & 16 != 0,
                    video_render: m & 32 != 0,
                    filter_target: m & 64 != 0,
                    deinterlacing: m & 128 != 0,
                };
                assert_eq!(
                    trace_render_video(&c, s),
                    trace_render_video(&r, s),
                    "{s:?}"
                );
                count += 1;
            }
        }
    }
    assert_eq!(count, 10 * 65 * 256);
}

/// A filter as the begin, tech end and skip steps see it.
#[derive(Clone, Copy, Debug)]
struct Filter {
    has_target: bool,
    has_parent: bool,
    same: bool,
    filter_flags: u32,
    parent_flags: u32,
    allow: c_int,
    space: c_int,
    current_space: c_int,
    cx: c_int,
    cy: c_int,
    texrender: bool,
    texrender_format: c_int,
    format: c_int,
    bypass_active: bool,
    video_render: bool,
    deinterlacing: bool,
}

fn trace_begin(side: &Side, f: Filter) -> Vec<u32> {
    let mut t = vec![(side.filter)(f.has_target, f.has_parent)];
    if t[0] != 0 {
        return t;
    }
    let reads = (side.bypass_reads_space)(f.same, f.allow, f.filter_flags, f.parent_flags);
    t.push(u32::from(reads));
    let current = if reads != 0 { f.current_space } else { 0 };
    let bypass = (side.bypass)(
        f.same,
        f.allow,
        f.filter_flags,
        f.parent_flags,
        f.space,
        current,
    );
    t.push(u32::from(bypass));
    if bypass != 0 {
        return t;
    }
    let skip = (side.size_skip)(f.cx, f.cy);
    t.push(u32::from(skip));
    if skip != 0 {
        t.extend(trace_skip(side, f));
        return t;
    }
    if f.texrender {
        t.push(u32::from((side.texrender_stale)(
            f.texrender_format,
            f.format,
        )));
    }
    t.push((side.filter_render)(f.same, f.parent_flags));
    t
}

fn trace_tech_end(side: &Side, f: Filter) -> Vec<u32> {
    let mut t = vec![(side.filter)(f.has_target, f.has_parent)];
    if t[0] != 0 {
        return t;
    }
    t.push(u32::from((side.tech_end_linear_srgb)(f.filter_flags)));
    t.push((side.tech_end)(f.bypass_active));
    t
}

fn trace_skip(side: &Side, f: Filter) -> Vec<u32> {
    let mut t = vec![(side.filter)(f.has_target, f.has_parent)];
    if t[0] != 0 {
        return t;
    }
    let mut route = (side.skip)(f.same, f.parent_flags);
    loop {
        t.push(route);
        let value = match Draw::from_value(route) {
            Some(Draw::ReadTargetVideoRender) => f.video_render,
            Some(Draw::ReadTargetDeinterlacing) => f.deinterlacing,
            _ => break,
        };
        route = (side.draw_next)(route, value);
    }
    t
}

#[test]
fn filter_decisions_every_filter() {
    let (c, r) = (c_side(), rust_side());
    let few_flags = [
        0,
        FLAG_VIDEO,
        FLAG_SRGB,
        FLAG_ASYNC | FLAG_VIDEO,
        FLAG_CUSTOM_DRAW | FLAG_VIDEO,
        FLAG_CUSTOM_DRAW | FLAG_ASYNC,
        FLAG_SRGB | FLAG_VIDEO,
        FLAG_SRGB | FLAG_ASYNC,
        u32::MAX,
    ];
    let sizes = [0, 1, 1920, -1];
    let mut count = 0u64;
    for wiring in 0u32..8 {
        for &ff in &few_flags {
            for &pf in &few_flags {
                for allow in [0, 1, 2] {
                    for space in [0, 1, 2, 3, 4] {
                        for current_space in [0, 2, 3, -1] {
                            for &cx in &sizes {
                                for &cy in &sizes {
                                    for m in 0u32..32 {
                                        let f = Filter {
                                            has_target: wiring & 1 != 0,
                                            has_parent: wiring & 2 != 0,
                                            same: wiring & 4 != 0,
                                            filter_flags: ff,
                                            parent_flags: pf,
                                            allow,
                                            space,
                                            current_space,
                                            cx,
                                            cy,
                                            texrender: m & 1 != 0,
                                            texrender_format: if m & 2 != 0 { 3 } else { 9 },
                                            format: 3,
                                            bypass_active: m & 4 != 0,
                                            video_render: m & 8 != 0,
                                            deinterlacing: m & 16 != 0,
                                        };
                                        assert_eq!(trace_begin(&c, f), trace_begin(&r, f), "{f:?}");
                                        assert_eq!(
                                            trace_tech_end(&c, f),
                                            trace_tech_end(&r, f),
                                            "{f:?}"
                                        );
                                        assert_eq!(trace_skip(&c, f), trace_skip(&r, f), "{f:?}");
                                        count += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(count, 8 * 9 * 9 * 3 * 5 * 4 * 4 * 4 * 32);
}

// --------------------------------------------------------------------------
// Proptests

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20_000))]

    #[test]
    fn prop_single_calls(ty in any::<i32>(), flags in any::<u32>(), x in any::<i32>(),
                         y in any::<i32>(), step in any::<u32>()) {
        check_type(ty);
        check_type_flags(ty, flags);
        check_flags(flags);
        check_ints(x, y);
        check_step(step);
    }

    #[test]
    fn prop_bypass(same in any::<bool>(), allow in any::<i32>(), ff in any::<u32>(),
                   pf in any::<u32>(), space in -8i32..8, current in -8i32..8,
                   wide in any::<i32>()) {
        check_bypass_reads(same, allow, ff, pf);
        check_bypass(same, allow, ff, pf, space, current);
        check_bypass(same, allow, ff, pf, wide, wide);
        check_bypass(same, 1, ff & !(FLAG_ASYNC | FLAG_CUSTOM_DRAW),
                     ff & !(FLAG_ASYNC | FLAG_CUSTOM_DRAW), wide, space);
    }

    #[test]
    fn prop_render_video(source_type in prop_oneof![0i32..6, any::<i32>()], flags in any::<u32>(),
                         m in any::<u16>()) {
        let s = Source {
            source_type,
            flags,
            has_data: m & 1 != 0,
            enabled: m & 2 != 0,
            rendering_filter: m & 4 != 0,
            has_filters: m & 8 != 0,
            has_filter_parent: m & 16 != 0,
            video_render: m & 32 != 0,
            filter_target: m & 64 != 0,
            deinterlacing: m & 128 != 0,
        };
        prop_assert_eq!(trace_render_video(&c_side(), s), trace_render_video(&rust_side(), s));
    }

    #[test]
    fn prop_filter(m in any::<u32>(), ff in any::<u32>(), pf in any::<u32>(),
                   allow in prop_oneof![0i32..3, any::<i32>()],
                   space in prop_oneof![0i32..4, any::<i32>()],
                   current_space in prop_oneof![0i32..4, any::<i32>()],
                   cx in prop_oneof![0i32..2, any::<i32>()], cy in prop_oneof![0i32..2, any::<i32>()],
                   texrender_format in -2i32..24, format in -2i32..24) {
        let f = Filter {
            has_target: m & 1 != 0,
            has_parent: m & 2 != 0,
            same: m & 4 != 0,
            filter_flags: ff,
            parent_flags: pf,
            allow,
            space,
            current_space,
            cx,
            cy,
            texrender: m & 8 != 0,
            texrender_format,
            format,
            bypass_active: m & 16 != 0,
            video_render: m & 32 != 0,
            deinterlacing: m & 64 != 0,
        };
        let (c, r) = (c_side(), rust_side());
        prop_assert_eq!(trace_begin(&c, f), trace_begin(&r, f));
        prop_assert_eq!(trace_tech_end(&c, f), trace_tech_end(&r, f));
        prop_assert_eq!(trace_skip(&c, f), trace_skip(&r, f));
    }
}
