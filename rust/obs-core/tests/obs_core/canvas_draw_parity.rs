//! Tier 3: the `obs_canvas_draw_*` and `obs_display_clear_*` shims against
//! the original C (`obs-c-oracle`, `oracle/canvas_draw.c`: the unmodified
//! `libobs/obs-canvas-draw.c.h` with its functions renamed), and the safe
//! core against both.
//!
//! Every function takes and returns plain values, so a run compares the
//! bytes of the results: the C's `bool` as the byte it returns, the
//! technique as its value, the clear flags as the `uint32_t`, the
//! multiplier as its bits (a NaN's payload included).
//!
//! Runs:
//! - the enum values, the clear flags (copied and from `graphics.h`) and the
//!   type sizes against the C headers, and the technique names;
//! - exhaustive: every flag of the skip, the clear colour, the clear flags
//!   and the workaround; the technique, the white level read and the linear
//!   clear colour for every pair of 649 spaces (-320 to 320 and four near
//!   each end of `i32`); the multiplier for every `f32` whose top 16 bits
//!   vary (65,536 patterns, each sign, exponent and NaN class), every
//!   multiple of 80 to 80 * 2^16, and 24 special levels;
//! - the decisions of `obs_render_canvas_texture_internal()` and
//!   `render_display_begin()` in their order (the skip, the technique name,
//!   the white level read and the multiplier only when it is read; the host
//!   answer, the space only when it is read, the clear flags and the
//!   workaround), one copy on each side;
//! - proptests (20,000 cases each): every decision over any space and any
//!   white level bit pattern (#1951: never a float range), and both callers'
//!   decisions over all of it at once.
//!
//! No input reaches an integer division or a 128-bit multiply on either
//! side (the only division is the `f32` multiplier, which cannot trap), so
//! no input is skipped on MSVC x64. No intentional differences: there is no
//! pointer to check.

use std::ffi::CStr;
use std::os::raw::c_int;

use obs_c_oracle::canvas_draw as oracle;
use obs_core::canvas_draw::{
    self as core_draw, CLEAR_COLOR, CLEAR_DEPTH, CLEAR_STENCIL, GS_CS_709_EXTENDED,
    GS_CS_709_SCRGB, GS_CS_SRGB, GS_CS_SRGB_16F, Technique,
};
use obs_core::ffi::canvas_draw as shim;
use proptest::prelude::*;

const TECHNIQUES: [Technique; 3] = [
    Technique::Draw,
    Technique::DrawMultiply,
    Technique::DrawTonemap,
];

#[test]
fn values_match_the_c_headers() {
    let rust: [i64; 19] = [
        GS_CS_SRGB.into(),
        GS_CS_SRGB_16F.into(),
        GS_CS_709_EXTENDED.into(),
        GS_CS_709_SCRGB.into(),
        TECHNIQUES[0].value().into(),
        TECHNIQUES[1].value().into(),
        TECHNIQUES[2].value().into(),
        CLEAR_COLOR.into(),
        CLEAR_DEPTH.into(),
        CLEAR_STENCIL.into(),
        CLEAR_COLOR.into(),
        CLEAR_DEPTH.into(),
        CLEAR_STENCIL.into(),
        4,
        4,
        4,
        1,
        4,
        4,
    ];
    for (which, (name, value)) in oracle::VALUE_NAMES.iter().zip(rust).enumerate() {
        // SAFETY: a pure lookup.
        let c = unsafe { oracle::oracle_canvas_draw_value(which) };
        assert_eq!(value, c, "{name}");
    }
    // SAFETY: a pure lookup, one past the end.
    let past = unsafe { oracle::oracle_canvas_draw_value(oracle::VALUE_NAMES.len()) };
    assert_eq!(past, i64::MIN, "the list has as many values as names");
    // The names C maps each value the port returns to are the port's.
    for technique in TECHNIQUES {
        let value = c_int::try_from(technique.value()).unwrap();
        // SAFETY: returns a static string for any value.
        let name = unsafe { CStr::from_ptr(oracle::oracle_canvas_draw_technique_name(value)) };
        assert_eq!(name.to_str().unwrap(), technique.name());
    }
}

// --------------------------------------------------------------------------
// One call on each side

fn c_skip(has_mix: bool, rendered: bool) -> u8 {
    // SAFETY: plain values.
    unsafe { oracle::oracle_obs_canvas_draw_skip(has_mix, rendered) }
}

fn c_technique(s: c_int, c: c_int) -> u32 {
    // SAFETY: plain values.
    unsafe { oracle::oracle_obs_canvas_draw_technique(s, c) }
}

fn c_reads_white_level(c: c_int) -> u8 {
    // SAFETY: plain values.
    unsafe { oracle::oracle_obs_canvas_draw_reads_white_level(c) }
}

fn c_multiplier(level: f32) -> u32 {
    // SAFETY: plain values.
    unsafe { oracle::oracle_obs_canvas_draw_multiplier(level) }.to_bits()
}

fn c_reads_space(always: bool) -> u8 {
    // SAFETY: plain values.
    unsafe { oracle::oracle_obs_display_clear_reads_space(always) }
}

fn c_linear(c: c_int) -> u8 {
    // SAFETY: plain values.
    unsafe { oracle::oracle_obs_display_clear_linear(c) }
}

fn c_flags(workaround: bool) -> u32 {
    // SAFETY: plain values.
    unsafe { oracle::oracle_obs_display_clear_flags(workaround) }
}

fn c_solid(workaround: bool) -> u8 {
    // SAFETY: plain values.
    unsafe { oracle::oracle_obs_display_clear_draws_solid(workaround) }
}

fn check_flags(has_mix: bool, rendered: bool, always: bool, workaround: bool) {
    let what = format!("mix {has_mix} rendered {rendered} always {always} workaround {workaround}");
    assert_eq!(
        u8::from(shim::obs_canvas_draw_skip(has_mix, rendered)),
        c_skip(has_mix, rendered),
        "{what}"
    );
    assert_eq!(
        core_draw::skip(has_mix, rendered),
        c_skip(has_mix, rendered) != 0,
        "{what}"
    );
    assert_eq!(
        u8::from(shim::obs_display_clear_reads_space(always)),
        c_reads_space(always),
        "{what}"
    );
    assert_eq!(
        core_draw::clear_reads_space(always),
        c_reads_space(always) != 0,
        "{what}"
    );
    assert_eq!(
        shim::obs_display_clear_flags(workaround),
        c_flags(workaround),
        "{what}"
    );
    assert_eq!(
        core_draw::clear_flags(workaround),
        c_flags(workaround),
        "{what}"
    );
    assert_eq!(
        u8::from(shim::obs_display_clear_draws_solid(workaround)),
        c_solid(workaround),
        "{what}"
    );
    assert_eq!(
        core_draw::clear_draws_solid(workaround),
        c_solid(workaround) != 0,
        "{what}"
    );
}

fn check_spaces(s: c_int, c: c_int) {
    assert_eq!(
        shim::obs_canvas_draw_technique(s, c),
        c_technique(s, c),
        "technique {s} {c}"
    );
    assert_eq!(
        core_draw::technique(s, c).value(),
        c_technique(s, c),
        "core technique {s} {c}"
    );
    assert_eq!(
        u8::from(shim::obs_canvas_draw_reads_white_level(c)),
        c_reads_white_level(c),
        "reads {c}"
    );
    assert_eq!(
        core_draw::reads_white_level(c),
        c_reads_white_level(c) != 0,
        "core reads {c}"
    );
    assert_eq!(
        u8::from(shim::obs_display_clear_linear(c)),
        c_linear(c),
        "linear {c}"
    );
    assert_eq!(
        core_draw::clear_linear(c),
        c_linear(c) != 0,
        "core linear {c}"
    );
}

fn check_multiplier(level: f32) {
    let c = c_multiplier(level);
    assert_eq!(
        shim::obs_canvas_draw_multiplier(level).to_bits(),
        c,
        "multiplier {level} ({:#010x})",
        level.to_bits()
    );
    assert_eq!(
        core_draw::multiplier(level).to_bits(),
        c,
        "core multiplier {level} ({:#010x})",
        level.to_bits()
    );
}

/// `obs_render_canvas_texture_internal()`'s decisions in its order, as
/// (drawn, technique name, white level read, multiplier bits); one side's
/// functions are passed in.
struct Canvas {
    skip: fn(bool, bool) -> bool,
    technique: fn(c_int, c_int) -> u32,
    reads: fn(c_int) -> bool,
    multiplier: fn(f32) -> u32,
}

fn canvas_plan(
    side: &Canvas,
    has_mix: bool,
    rendered: bool,
    s: c_int,
    c: c_int,
    level: f32,
) -> Option<(String, bool, u32)> {
    if (side.skip)(has_mix, rendered) {
        return None;
    }
    let value = c_int::try_from((side.technique)(s, c)).unwrap();
    // SAFETY: returns a static string for any value.
    let name = unsafe { CStr::from_ptr(oracle::oracle_canvas_draw_technique_name(value)) };
    let reads = (side.reads)(c);
    let multiplier = if reads {
        (side.multiplier)(level)
    } else {
        1.0_f32.to_bits()
    };
    Some((name.to_str().unwrap().to_owned(), reads, multiplier))
}

/// `render_display_begin()`'s decisions in its order, as (space read,
/// linear, clear flags, solid draw).
fn display_plan_c(always: bool, c: c_int, workaround: bool) -> (bool, bool, u32, bool) {
    let reads = c_reads_space(always) != 0;
    let linear = reads && c_linear(c) != 0;
    (reads, linear, c_flags(workaround), c_solid(workaround) != 0)
}

fn display_plan_rs(always: bool, c: c_int, workaround: bool) -> (bool, bool, u32, bool) {
    let reads = shim::obs_display_clear_reads_space(always);
    let linear = reads && shim::obs_display_clear_linear(c);
    (
        reads,
        linear,
        shim::obs_display_clear_flags(workaround),
        shim::obs_display_clear_draws_solid(workaround),
    )
}

const C_SIDE: Canvas = Canvas {
    skip: |m, r| c_skip(m, r) != 0,
    technique: c_technique,
    reads: |c| c_reads_white_level(c) != 0,
    multiplier: c_multiplier,
};

const RS_SIDE: Canvas = Canvas {
    skip: |m, r| shim::obs_canvas_draw_skip(m, r),
    technique: |s, c| shim::obs_canvas_draw_technique(s, c),
    reads: |c| shim::obs_canvas_draw_reads_white_level(c),
    multiplier: |l| shim::obs_canvas_draw_multiplier(l).to_bits(),
};

fn check_callers(
    has_mix: bool,
    rendered: bool,
    s: c_int,
    c: c_int,
    level: f32,
    always: bool,
    workaround: bool,
) {
    assert_eq!(
        canvas_plan(&RS_SIDE, has_mix, rendered, s, c, level),
        canvas_plan(&C_SIDE, has_mix, rendered, s, c, level),
        "canvas mix {has_mix} rendered {rendered} {s} {c} {level}"
    );
    assert_eq!(
        display_plan_rs(always, c, workaround),
        display_plan_c(always, c, workaround),
        "display always {always} {c} workaround {workaround}"
    );
}

// --------------------------------------------------------------------------
// Exhaustive

/// -320 to 320 and four near each end of `i32`.
fn spaces() -> Vec<c_int> {
    let mut spaces: Vec<c_int> = (-320..=320).collect();
    spaces.extend([c_int::MIN, c_int::MIN + 1, c_int::MAX - 1, c_int::MAX]);
    spaces.extend([
        c_int::MIN + 2,
        c_int::MIN + 3,
        c_int::MAX - 3,
        c_int::MAX - 2,
    ]);
    spaces
}

const SPECIAL_LEVELS: [f32; 24] = [
    0.0,
    -0.0,
    1.0,
    -1.0,
    80.0,
    -80.0,
    100.0,
    203.0,
    300.0,
    1000.0,
    10000.0,
    f32::MIN_POSITIVE,
    1e-45,
    -1e-45,
    f32::MAX,
    f32::MIN,
    f32::EPSILON,
    f32::INFINITY,
    f32::NEG_INFINITY,
    f32::NAN,
    -f32::NAN,
    0.1,
    79.999_99,
    80.000_01,
];

#[test]
fn every_flag() {
    for bits in 0..16_u8 {
        check_flags(bits & 1 != 0, bits & 2 != 0, bits & 4 != 0, bits & 8 != 0);
    }
}

#[test]
fn every_space_pair() {
    let spaces = spaces();
    for &s in &spaces {
        for &c in &spaces {
            check_spaces(s, c);
        }
    }
}

#[test]
fn every_white_level_class() {
    for high in 0..=u16::MAX {
        let bits = u32::from(high) << 16 | 0x5a5a;
        check_multiplier(f32::from_bits(bits));
        check_multiplier(f32::from_bits(u32::from(high) << 16));
    }
    let mut k = 0.0_f32;
    while k <= 65536.0 {
        check_multiplier(80.0 * k);
        k += 1.0;
    }
    for level in SPECIAL_LEVELS {
        check_multiplier(level);
    }
}

#[test]
fn callers_every_input() {
    let spaces: Vec<c_int> = (-3..=6).chain([c_int::MIN, c_int::MAX]).collect();
    for bits in 0..16_u8 {
        for &s in &spaces {
            for &c in &spaces {
                for level in SPECIAL_LEVELS {
                    check_callers(
                        bits & 1 != 0,
                        bits & 2 != 0,
                        s,
                        c,
                        level,
                        bits & 4 != 0,
                        bits & 8 != 0,
                    );
                }
            }
        }
    }
}

// --------------------------------------------------------------------------
// Proptests

/// A colour space: mostly in or next to the enum, sometimes any `int`.
fn space() -> impl Strategy<Value = c_int> {
    prop_oneof![3 => -2..6_i32, 1 => any::<i32>()]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20_000))]

    #[test]
    fn proptest_decisions(s in space(), c in space(), bits in any::<u32>(), flags in 0..16_u8) {
        check_spaces(s, c);
        check_multiplier(f32::from_bits(bits));
        check_flags(flags & 1 != 0, flags & 2 != 0, flags & 4 != 0, flags & 8 != 0);
    }

    #[test]
    fn proptest_callers(s in space(), c in space(), bits in any::<u32>(), flags in 0..16_u8) {
        check_callers(flags & 1 != 0, flags & 2 != 0, s, c, f32::from_bits(bits), flags & 4 != 0, flags & 8 != 0);
    }
}
