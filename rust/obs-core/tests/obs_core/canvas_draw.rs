//! Tier 1: `obs_core::canvas_draw`, the canvas texture draw and display
//! clear decisions.
//!
//! The `steps_*` cases port `test/cmocka/test_canvas_draw_steps.c` 1:1: its
//! 12 cases under the same names, in the same order, with the same values
//! and expectations, the "characterized, not endorsed" ones included. The C
//! calls the hidden `obs_canvas_draw_*` and `obs_display_clear_*`
//! functions; here the safe core is called directly, and the C's technique
//! names are [`Technique::name`].
//!
//! The `edge_*` cases are new: proptests over every colour space `int` and
//! every `f32` white level (taken as bit patterns, so NaNs, infinities and
//! denormals come up) against models written from the C's switch and
//! expressions.

use core::ffi::c_int;

use obs_core::canvas_draw::{
    CLEAR_COLOR, CLEAR_DEPTH, CLEAR_STENCIL, GS_CS_709_EXTENDED, GS_CS_709_SCRGB, GS_CS_SRGB,
    GS_CS_SRGB_16F, SCRGB_WHITE, Technique, clear_draws_solid, clear_flags, clear_linear,
    clear_reads_space, multiplier, reads_white_level, skip, technique,
};
use proptest::prelude::*;

/// Colour spaces outside `enum gs_color_space`.
const OUTSIDE_SPACES: [c_int; 5] = [-1, 4, 99, c_int::MIN, c_int::MAX];

const SPACES: [c_int; 4] = [
    GS_CS_SRGB,
    GS_CS_SRGB_16F,
    GS_CS_709_EXTENDED,
    GS_CS_709_SCRGB,
];

/// SDR white levels: OBS's default, a common one, and 0.
const WHITES: [f32; 3] = [80.0, 300.0, 0.0];

const D: Technique = Technique::Draw;
const M: Technique = Technique::DrawMultiply;
const T: Technique = Technique::DrawTonemap;

fn check_bits(what: &str, got: f32, want: u32) {
    assert_eq!(
        got.to_bits(),
        want,
        "{what}: {got} ({:#010x}), expected {} ({want:#010x})",
        got.to_bits(),
        f32::from_bits(want)
    );
}

// ---------------------------------------------------------------------------
// constants

#[test]
fn steps_constants() {
    assert_eq!(Technique::Draw.value(), 0);
    assert_eq!(Technique::DrawMultiply.value(), 1);
    assert_eq!(Technique::DrawTonemap.value(), 2);
    assert_eq!(
        size_of::<Technique>(),
        1,
        "the safe enum; the shim returns the C's int"
    );
    assert_eq!(CLEAR_COLOR, 1);
    assert_eq!(CLEAR_DEPTH, 2);
    assert_eq!(CLEAR_STENCIL, 4);
    assert_eq!(D.name(), "Draw");
    assert_eq!(M.name(), "DrawMultiply");
    assert_eq!(T.name(), "DrawTonemap");
    check_bits("SCRGB_WHITE", SCRGB_WHITE, 0x42a0_0000);
}

// ---------------------------------------------------------------------------
// canvas: the early return

/// Both early returns skip; no mix with a rendered flag, which obs.c never
/// passes, also skips (characterized, not endorsed).
#[test]
fn steps_skip_every_input() {
    assert!(skip(false, false));
    assert!(skip(false, true));
    assert!(skip(true, false));
    assert!(!skip(true, true));
}

// ---------------------------------------------------------------------------
// canvas: technique

/// `[source][current]`.
const TECHNIQUE: [[Technique; 4]; 4] = [[D, D, D, M], [D, D, D, M], [T, T, D, M], [D, D, D, M]];

#[test]
fn steps_technique_space_pairs() {
    for (s, row) in SPACES.iter().zip(TECHNIQUE) {
        for (c, want) in SPACES.iter().zip(row) {
            assert_eq!(technique(*s, *c), want, "source {s} current {c}");
        }
    }
}

#[test]
fn steps_technique_outside_enum() {
    for o in OUTSIDE_SPACES {
        for s in SPACES {
            assert_eq!(technique(s, o), D, "source {s} current {o}");
        }
        for p in OUTSIDE_SPACES {
            assert_eq!(technique(p, o), D, "source {p} current {o}");
        }
        assert_eq!(technique(o, GS_CS_SRGB), D);
        assert_eq!(technique(o, GS_CS_SRGB_16F), D);
        assert_eq!(technique(o, GS_CS_709_EXTENDED), D);
        assert_eq!(technique(o, GS_CS_709_SCRGB), M);
    }
}

// ---------------------------------------------------------------------------
// canvas: white level and multiplier

#[test]
fn steps_reads_white_level_spaces() {
    assert!(!reads_white_level(GS_CS_SRGB));
    assert!(!reads_white_level(GS_CS_SRGB_16F));
    assert!(!reads_white_level(GS_CS_709_EXTENDED));
    assert!(reads_white_level(GS_CS_709_SCRGB));
    for o in OUTSIDE_SPACES {
        assert!(!reads_white_level(o), "{o}");
    }
}

#[test]
fn steps_multiplier_white_levels() {
    check_bits("80", multiplier(80.0), 0x3f80_0000);
    check_bits("300", multiplier(300.0), 0x4070_0000);
    check_bits("0", multiplier(0.0), 0x0000_0000);
    check_bits("203", multiplier(203.0), (203.0_f32 / 80.0).to_bits());
    check_bits("10000", multiplier(10000.0), 0x42fa_0000);
}

/// Characterized, not endorsed: -0 stays -0, a negative level gives a
/// negative multiplier, the infinities stay infinite, a NaN stays a NaN, and
/// a denormal underflows to 0.
#[test]
fn steps_multiplier_any_level() {
    check_bits("-0", multiplier(-0.0), 0x8000_0000);
    check_bits("-80", multiplier(-80.0), 0xbf80_0000);
    check_bits("inf", multiplier(f32::INFINITY), 0x7f80_0000);
    check_bits("-inf", multiplier(f32::NEG_INFINITY), 0xff80_0000);
    check_bits("1e-45", multiplier(1e-45), 0x0000_0000);
    check_bits("FLT_MAX", multiplier(f32::MAX), (f32::MAX / 80.0).to_bits());
    assert!(multiplier(f32::NAN).is_nan());
    // every multiple of 80 up to 80 * 2^16 divides exactly
    let mut k = 0.0_f32;
    while k <= 65536.0 {
        check_bits("80 k", multiplier(80.0 * k), k.to_bits());
        k += 1.0;
    }
}

#[test]
fn steps_canvas_plan_every_input() {
    const NAMES: [[&str; 4]; 4] = [
        ["Draw", "Draw", "Draw", "DrawMultiply"],
        ["Draw", "Draw", "Draw", "DrawMultiply"],
        ["DrawTonemap", "DrawTonemap", "Draw", "DrawMultiply"],
        ["Draw", "Draw", "Draw", "DrawMultiply"],
    ];
    const MULTIPLIERS: [u32; 3] = [0x3f80_0000, 0x4070_0000, 0x0000_0000];

    for (s, names) in SPACES.iter().zip(NAMES) {
        for (c, want_name) in SPACES.iter().zip(names) {
            for (w, want_multiplier) in WHITES.iter().zip(MULTIPLIERS) {
                let name = technique(*s, *c).name();
                let mut got = 1.0_f32;
                if reads_white_level(*c) {
                    got = multiplier(*w);
                }
                assert_eq!(name, want_name, "source {s} current {c}");
                let want = if *c == GS_CS_709_SCRGB {
                    want_multiplier
                } else {
                    0x3f80_0000
                };
                check_bits("multiplier", got, want);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// display: clear colour

#[test]
fn steps_clear_reads_space_every_input() {
    assert!(clear_reads_space(false));
    assert!(!clear_reads_space(true));
}

#[test]
fn steps_clear_linear_spaces() {
    assert!(clear_linear(GS_CS_SRGB));
    assert!(!clear_linear(GS_CS_SRGB_16F));
    assert!(!clear_linear(GS_CS_709_EXTENDED));
    assert!(!clear_linear(GS_CS_709_SCRGB));
    for o in OUTSIDE_SPACES {
        assert!(!clear_linear(o), "{o}");
    }
}

#[test]
fn steps_clear_colour_every_input() {
    for always in [false, true] {
        for space in SPACES.into_iter().chain(OUTSIDE_SPACES) {
            let linear = clear_reads_space(always) && clear_linear(space);
            assert_eq!(
                linear,
                !always && space == GS_CS_SRGB,
                "always {always} space {space}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// display: clear flags and the workaround

#[test]
fn steps_clear_every_input() {
    assert_eq!(
        clear_flags(false),
        CLEAR_COLOR | CLEAR_DEPTH | CLEAR_STENCIL
    );
    assert_eq!(clear_flags(true), CLEAR_DEPTH | CLEAR_STENCIL);
    assert!(!clear_draws_solid(false));
    assert!(clear_draws_solid(true));
    for workaround in [false, true] {
        let flags = clear_flags(workaround);
        assert_ne!(
            flags & CLEAR_COLOR != 0,
            clear_draws_solid(workaround),
            "{workaround}"
        );
    }
}

// ---------------------------------------------------------------------------
// edge cases (new)

/// `obs_canvas_draw_technique()`'s switch, written out.
fn model_technique(source: c_int, current: c_int) -> Technique {
    match current {
        GS_CS_SRGB | GS_CS_SRGB_16F if source == GS_CS_709_EXTENDED => T,
        GS_CS_709_SCRGB => M,
        _ => D,
    }
}

/// A colour space: mostly in or next to the enum, sometimes any `int`.
fn space() -> impl Strategy<Value = c_int> {
    prop_oneof![3 => -2..6_i32, 1 => any::<i32>()]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4096))]

    #[test]
    fn edge_technique_any_space(source in space(), current in space()) {
        prop_assert_eq!(technique(source, current), model_technique(source, current));
        prop_assert_eq!(reads_white_level(current), current == GS_CS_709_SCRGB);
        prop_assert_eq!(clear_linear(current), current == GS_CS_SRGB);
    }

    /// Every white level as a bit pattern (#1951: no wide `Range<f32>`):
    /// one division by 80, bit for bit, NaN for NaN.
    #[test]
    fn edge_multiplier_any_bits(bits in any::<u32>()) {
        let level = f32::from_bits(bits);
        let got = multiplier(level);
        let want = level / 80.0;
        if want.is_nan() {
            prop_assert!(got.is_nan());
        } else {
            prop_assert_eq!(got.to_bits(), want.to_bits());
        }
    }

    #[test]
    fn edge_skip_and_clear_any_flags(has_mix: bool, rendered: bool, always: bool, workaround: bool) {
        prop_assert_eq!(skip(has_mix, rendered), !has_mix || !rendered);
        prop_assert_eq!(clear_reads_space(always), !always);
        let colour = if workaround { 0 } else { CLEAR_COLOR };
        prop_assert_eq!(clear_flags(workaround), CLEAR_DEPTH | CLEAR_STENCIL | colour);
        prop_assert_eq!(clear_draws_solid(workaround), workaround);
    }
}
