//! Tier 1: `obs_source::draw_plan`, the source draw decisions.
//!
//! The `steps_*` cases port `test/cmocka/test_source_draw_plan_steps.c` 1:1,
//! under the same case names, in the same order, with the same inputs and
//! expectations, the floats compared as bits.
//!
//! The `edge_*` cases are new: the rotation translate for every angle from
//! -1000 to 1000, the ends of `c_long` and (where `long` is 64 bits) angles
//! past 32 bits, against a model that converts the size through `f64`; the
//! sizes around 2^24 and `u32::MAX`; every decision over every pair of 260
//! spaces around the enum and the ends of `i32` with every flag, against a
//! model written from the space classes; the multiplier at every special
//! `f32` bit pattern against the one division; and proptests of each over
//! full-range values (white levels as any bit pattern).

use std::os::raw::c_long;

use obs_source::draw_plan::{
    GS_CS_709_EXTENDED, GS_CS_709_SCRGB, GS_CS_SRGB, GS_CS_SRGB_16F, Technique, async_linear_srgb,
    async_nonlinear_alpha, convert, multiplier, reads_white_level, rotation_x, rotation_y,
    technique,
};
use proptest::prelude::*;

/// Colour spaces outside `enum gs_color_space`, as the C test's
/// `OUTSIDE_SPACES`.
const OUTSIDE_SPACES: [i32; 5] = [-1, 4, 99, i32::MIN, i32::MAX];

const SPACES: [i32; 4] = [
    GS_CS_SRGB,
    GS_CS_SRGB_16F,
    GS_CS_709_EXTENDED,
    GS_CS_709_SCRGB,
];

/// SDR white levels: OBS's default, a common one, and 0.
const WHITES: [f32; 3] = [80.0, 300.0, 0.0];

const ONE: u32 = 0x3f80_0000;

use Technique::{
    Draw as D, DrawMultiply as M, DrawMultiplyTonemap as MT, DrawNonlinearAlpha as NA,
    DrawNonlinearAlphaMultiply as NAM, DrawTonemap as T,
};

fn bits(f: f32) -> u32 {
    f.to_bits()
}

// --- enum values ---------------------------------------------------------

#[test]
fn steps_enum_values() {
    // The C maps these numbers to names; the first four are the
    // deinterlace draw decisions' too.
    let all = [
        (D, 0, "Draw"),
        (M, 1, "DrawMultiply"),
        (T, 2, "DrawTonemap"),
        (MT, 3, "DrawMultiplyTonemap"),
        (NA, 4, "DrawNonlinearAlpha"),
        (NAM, 5, "DrawNonlinearAlphaMultiply"),
    ];
    for (technique, value, name) in all {
        assert_eq!(technique as i32, value);
        assert_eq!(technique.value(), u32::try_from(value).unwrap());
        assert_eq!(technique.name(), name);
    }
    assert_eq!(
        obs_source::deinterlace_draw::Technique::DrawMultiplyTonemap,
        MT
    );
}

// --- rotation ------------------------------------------------------------

#[test]
fn steps_rotation_multiples_of_90() {
    let cases: [(c_long, u32, u32, u32, u32); 12] = [
        (0, 1920, 1080, 0, 0),
        (90, 1920, 1080, 0, 0x44f0_0000),
        (180, 1920, 1080, 0x44f0_0000, 0x4487_0000),
        (270, 1920, 1080, 0x4487_0000, 0),
        (90, 1080, 1920, 0, 0x4487_0000),
        (180, 1080, 1920, 0x4487_0000, 0x44f0_0000),
        (270, 1080, 1920, 0x44f0_0000, 0),
        (0, 1080, 1920, 0, 0),
        (90, 640, 480, 0, 0x4420_0000),
        (180, 640, 480, 0x4420_0000, 0x43f0_0000),
        (270, 640, 480, 0x43f0_0000, 0),
        (0, 640, 480, 0, 0),
    ];
    for (rotation, w, h, x, y) in cases {
        assert_eq!(
            bits(rotation_x(rotation, w, h)),
            x,
            "x at {rotation} for {w}x{h}"
        );
        assert_eq!(
            bits(rotation_y(rotation, w, h)),
            y,
            "y at {rotation} for {w}x{h}"
        );
    }
}

#[test]
fn steps_rotation_other_angles() {
    let angles: [c_long; 17] = [
        1,
        45,
        89,
        91,
        135,
        179,
        181,
        269,
        271,
        359,
        360,
        450,
        -90,
        -180,
        -270,
        c_long::MIN,
        c_long::MAX,
    ];
    for angle in angles {
        for (w, h) in [(1920, 1080), (640, 480)] {
            assert_eq!(bits(rotation_x(angle, w, h)), 0, "x at {angle}");
            assert_eq!(bits(rotation_y(angle, w, h)), 0, "y at {angle}");
        }
    }
}

#[test]
fn steps_rotation_size_to_float() {
    assert_eq!(bits(rotation_y(90, 0, 7)), 0);
    assert_eq!(bits(rotation_x(270, 7, 0)), 0);
    assert_eq!(bits(rotation_y(90, 16_777_217, 3)), 0x4b80_0000);
    assert_eq!(bits(rotation_x(180, 16_777_219, 3)), 0x4b80_0002);
    assert_eq!(bits(rotation_y(180, 3, u32::MAX)), 0x4f80_0000);
    assert_eq!(bits(rotation_x(270, 3, u32::MAX)), 0x4f80_0000);
    assert_eq!(bits(rotation_x(180, 1, 1)), ONE);
}

// --- async video: linear sRGB and non-linear alpha ------------------------

const ASYNC_LINEAR: [[bool; 4]; 4] = [
    [false, true, true, true],
    [false, false, false, false],
    [true, true, false, false],
    [true, true, false, false],
];

#[test]
fn steps_async_linear_srgb_space_pairs() {
    for (s, row) in SPACES.iter().zip(ASYNC_LINEAR) {
        for (c, want) in SPACES.iter().zip(row) {
            assert_eq!(async_linear_srgb(*s, *c, false), want, "{s} into {c}");
            assert!(async_linear_srgb(*s, *c, true), "{s} into {c}, flag on");
        }
    }
}

#[test]
fn steps_async_linear_srgb_outside_enum() {
    for o in OUTSIDE_SPACES {
        for gs in [false, true] {
            for s in SPACES {
                assert_eq!(async_linear_srgb(o, s, gs), gs);
            }
            assert!(async_linear_srgb(GS_CS_SRGB, o, gs));
            assert_eq!(async_linear_srgb(GS_CS_SRGB_16F, o, gs), gs);
            assert_eq!(async_linear_srgb(GS_CS_709_EXTENDED, o, gs), gs);
            assert_eq!(async_linear_srgb(GS_CS_709_SCRGB, o, gs), gs);
        }
    }
}

#[test]
fn steps_async_nonlinear_alpha_every_input() {
    for linear in [false, true] {
        for alpha in [false, true] {
            assert_eq!(
                async_nonlinear_alpha(GS_CS_SRGB, linear, alpha),
                linear && !alpha
            );
            for s in &SPACES[1..] {
                assert!(!async_nonlinear_alpha(*s, linear, alpha));
            }
            for o in OUTSIDE_SPACES {
                assert!(!async_nonlinear_alpha(o, linear, alpha));
            }
        }
    }
}

// --- technique -----------------------------------------------------------

const TECHNIQUES: [[[Technique; 4]; 4]; 2] = [
    [[D, D, D, M], [D, D, D, M], [T, T, D, M], [MT, MT, M, D]],
    [
        [NA, NA, NA, NAM],
        [D, D, D, M],
        [T, T, D, M],
        [MT, MT, M, D],
    ],
];

#[test]
fn steps_technique_space_pairs() {
    for (na, table) in [false, true].into_iter().zip(TECHNIQUES) {
        for (s, row) in SPACES.iter().zip(table) {
            for (c, want) in SPACES.iter().zip(row) {
                assert_eq!(technique(*s, *c, na), want, "{s} into {c}, non-linear {na}");
            }
        }
    }
}

#[test]
fn steps_technique_outside_enum() {
    for o in OUTSIDE_SPACES {
        for na in [false, true] {
            for s in SPACES {
                assert_eq!(technique(o, s, na), D);
                assert_eq!(technique(s, o, na), D);
            }
        }
    }
    assert!(async_nonlinear_alpha(
        GS_CS_SRGB,
        async_linear_srgb(GS_CS_SRGB, 4, false),
        false
    ));
}

// --- white level and multiplier --------------------------------------------

const READS_WHITE: [[bool; 4]; 4] = [
    [false, false, false, true],
    [false, false, false, true],
    [false, false, false, true],
    [true, true, true, false],
];

const MULTIPLIERS: [[[u32; 3]; 4]; 4] = [
    [
        [ONE, ONE, ONE],
        [ONE, ONE, ONE],
        [ONE, ONE, ONE],
        [ONE, 0x4070_0000, 0],
    ],
    [
        [ONE, ONE, ONE],
        [ONE, ONE, ONE],
        [ONE, ONE, ONE],
        [ONE, 0x4070_0000, 0],
    ],
    [
        [ONE, ONE, ONE],
        [ONE, ONE, ONE],
        [ONE, ONE, ONE],
        [ONE, 0x4070_0000, 0],
    ],
    [
        [ONE, 0x3e88_8889, 0x7f80_0000],
        [ONE, 0x3e88_8889, 0x7f80_0000],
        [ONE, 0x3e88_8889, 0x7f80_0000],
        [ONE, ONE, ONE],
    ],
];

#[test]
fn steps_reads_white_level_space_pairs() {
    for (s, row) in SPACES.iter().zip(READS_WHITE) {
        for (c, want) in SPACES.iter().zip(row) {
            assert_eq!(reads_white_level(*s, *c), want, "{s} into {c}");
        }
    }
    for o in OUTSIDE_SPACES {
        for s in SPACES {
            assert!(!reads_white_level(o, s));
            assert!(!reads_white_level(s, o));
        }
    }
}

#[test]
fn steps_multiplier_space_pairs() {
    for (s, plane) in SPACES.iter().zip(MULTIPLIERS) {
        for (c, row) in SPACES.iter().zip(plane) {
            for (w, want) in WHITES.iter().zip(row) {
                assert_eq!(bits(multiplier(*s, *c, *w)), want, "{s} into {c} at {w}");
            }
        }
    }
}

#[test]
fn steps_multiplier_outside_enum() {
    for o in OUTSIDE_SPACES {
        for s in SPACES {
            for w in WHITES {
                assert_eq!(bits(multiplier(o, s, w)), ONE);
                assert_eq!(bits(multiplier(s, o, w)), ONE);
            }
        }
    }
}

#[test]
fn steps_multiplier_any_level() {
    assert_eq!(
        bits(multiplier(GS_CS_709_SCRGB, GS_CS_SRGB, -0.0)),
        0xff80_0000
    );
    assert_eq!(
        bits(multiplier(GS_CS_SRGB, GS_CS_709_SCRGB, -0.0)),
        0x8000_0000
    );
    assert_eq!(
        bits(multiplier(
            GS_CS_709_SCRGB,
            GS_CS_709_EXTENDED,
            f32::INFINITY
        )),
        0
    );
    assert_eq!(
        bits(multiplier(
            GS_CS_709_EXTENDED,
            GS_CS_709_SCRGB,
            f32::INFINITY
        )),
        0x7f80_0000
    );
    assert_eq!(
        bits(multiplier(GS_CS_SRGB_16F, GS_CS_709_SCRGB, -80.0)),
        0xbf80_0000
    );
    assert_eq!(
        bits(multiplier(GS_CS_709_SCRGB, GS_CS_SRGB_16F, -80.0)),
        0xbf80_0000
    );
    assert_eq!(
        bits(multiplier(GS_CS_SRGB, GS_CS_709_SCRGB, 480.0)),
        0x40c0_0000
    );
    assert_eq!(
        bits(multiplier(GS_CS_709_SCRGB, GS_CS_SRGB, 480.0)),
        0x3e2a_aaab
    );
    assert_eq!(
        bits(multiplier(GS_CS_709_EXTENDED, GS_CS_709_SCRGB, 203.0)),
        0x4022_6666
    );
    assert_eq!(
        bits(multiplier(GS_CS_709_SCRGB, GS_CS_SRGB, f32::from_bits(1))),
        0x7f80_0000
    );
    assert_eq!(
        bits(multiplier(GS_CS_SRGB, GS_CS_709_SCRGB, f32::from_bits(1))),
        0
    );
    assert!(multiplier(GS_CS_709_SCRGB, GS_CS_SRGB, f32::NAN).is_nan());
    assert!(multiplier(GS_CS_SRGB, GS_CS_709_SCRGB, f32::NAN).is_nan());
    assert_eq!(bits(multiplier(GS_CS_SRGB, GS_CS_SRGB, f32::NAN)), ONE);
    assert_eq!(bits(multiplier(GS_CS_709_EXTENDED, GS_CS_SRGB, 0.0)), ONE);
}

// --- source_render() colour space conversion --------------------------------

const CONVERTS: [[bool; 4]; 4] = [
    [false, false, true, true],
    [false, false, true, true],
    [true, true, false, true],
    [true, true, true, false],
];

#[test]
fn steps_convert_space_pairs() {
    for (s, row) in SPACES.iter().zip(CONVERTS) {
        for (c, want) in SPACES.iter().zip(row) {
            assert_eq!(convert(*s, *c), want, "{s} into {c}");
        }
    }
}

#[test]
fn steps_convert_outside_enum() {
    for o in OUTSIDE_SPACES {
        for s in SPACES {
            assert!(!convert(o, s));
            assert!(!convert(s, o));
        }
        assert!(!convert(o, o));
    }
}

// --- the decisions in their callers' order ---------------------------------

#[test]
fn steps_async_plan_every_input() {
    for (si, s) in SPACES.iter().enumerate() {
        for (ci, c) in SPACES.iter().enumerate() {
            for gs in [false, true] {
                for alpha in [false, true] {
                    for (wi, w) in WHITES.iter().enumerate() {
                        let linear = async_linear_srgb(*s, *c, gs);
                        let na = async_nonlinear_alpha(*s, linear, alpha);
                        let t = technique(*s, *c, na);
                        let m = if reads_white_level(*s, *c) {
                            multiplier(*s, *c, *w)
                        } else {
                            1.0
                        };
                        let want_linear = gs || ASYNC_LINEAR[si][ci];
                        let want_na = si == 0 && want_linear && !alpha;
                        assert_eq!(linear, want_linear);
                        assert_eq!(na, want_na);
                        assert_eq!(t, TECHNIQUES[usize::from(want_na)][si][ci]);
                        assert_eq!(bits(m), MULTIPLIERS[si][ci][wi]);
                    }
                }
            }
        }
    }
}

#[test]
fn steps_convert_plan_every_input() {
    for (si, s) in SPACES.iter().enumerate() {
        for (ci, c) in SPACES.iter().enumerate() {
            for (wi, w) in WHITES.iter().enumerate() {
                assert_eq!(convert(*s, *c), CONVERTS[si][ci]);
                if !CONVERTS[si][ci] {
                    continue;
                }
                assert_eq!(technique(*s, *c, false), TECHNIQUES[0][si][ci]);
                let m = if reads_white_level(*s, *c) {
                    multiplier(*s, *c, *w)
                } else {
                    1.0
                };
                assert_eq!(bits(m), MULTIPLIERS[si][ci][wi]);
            }
        }
    }
    assert_eq!(technique(GS_CS_SRGB, GS_CS_709_EXTENDED, false), D);
    assert!(!reads_white_level(GS_CS_SRGB_16F, GS_CS_709_EXTENDED));
}

// --- edge cases: models ----------------------------------------------------

/// The C's `(float)uint32_t`, through `f64` (exact for any `u32`) and one
/// rounding to nearest even.
fn model_size(size: u32) -> f32 {
    #[allow(clippy::cast_possible_truncation)]
    let f = f64::from(size) as f32;
    f
}

fn model_rotation(rotation: c_long, w: u32, h: u32) -> (f32, f32) {
    match rotation {
        90 => (0.0, model_size(w)),
        180 => (model_size(w), model_size(h)),
        270 => (model_size(h), 0.0),
        _ => (0.0, 0.0),
    }
}

/// The space class: SDR, 709_EXTENDED, scRGB, or none outside the enum.
fn class(space: i32) -> Option<u8> {
    match space {
        GS_CS_SRGB | GS_CS_SRGB_16F => Some(0),
        GS_CS_709_EXTENDED => Some(1),
        GS_CS_709_SCRGB => Some(2),
        _ => None,
    }
}

fn model_linear(s: i32, c: i32, gs: bool) -> bool {
    gs || (s == GS_CS_SRGB && c != GS_CS_SRGB)
        || (matches!(class(s), Some(1 | 2)) && class(c) == Some(0))
}

fn model_nonlinear(s: i32, linear: bool, alpha: bool) -> bool {
    s == GS_CS_SRGB && linear && !alpha
}

fn model_technique(s: i32, c: i32, na: bool) -> Technique {
    match (class(s), class(c)) {
        (Some(0), Some(0 | 1)) if s == GS_CS_SRGB && na => NA,
        (Some(0), Some(2)) if s == GS_CS_SRGB && na => NAM,
        (Some(0), Some(2)) => M,
        (Some(1), Some(0)) => T,
        (Some(1), Some(2)) => M,
        (Some(2), Some(0)) => MT,
        (Some(2), Some(1)) => M,
        _ => D,
    }
}

fn model_reads_white(s: i32, c: i32) -> bool {
    matches!(
        (class(s), class(c)),
        (Some(0 | 1), Some(2)) | (Some(2), Some(0 | 1))
    )
}

fn model_multiplier(s: i32, c: i32, w: f32) -> f32 {
    match (class(s), class(c)) {
        (Some(0 | 1), Some(2)) => w / 80.0,
        (Some(2), Some(0 | 1)) => 80.0 / w,
        _ => 1.0,
    }
}

fn model_convert(s: i32, c: i32) -> bool {
    matches!((class(s), class(c)), (Some(a), Some(b)) if a != b)
}

fn edge_spaces() -> Vec<i32> {
    let mut v: Vec<i32> = (-120..=120).collect();
    v.extend([
        i32::MIN,
        i32::MIN + 1,
        i32::MIN + 2,
        i32::MAX - 2,
        i32::MAX - 1,
        i32::MAX,
    ]);
    v.extend([1000, -1000, 65_536, -65_536, 1 << 24, 1 << 30, -(1 << 30)]);
    v.extend([0x100, 0x101, 0x102, 0x103, -0x100, 0x7fff_ff00]);
    v
}

/// Special `f32` levels: zeros, subnormals, the normal ends, OBS's levels,
/// infinities, quiet and signalling NaNs of both signs with payloads.
const SPECIAL_LEVELS: [u32; 26] = [
    0x0000_0000,
    0x8000_0000,
    0x0000_0001,
    0x8000_0001,
    0x007f_ffff,
    0x0080_0000,
    0x7f7f_ffff,
    0xff7f_ffff,
    0x3f80_0000,
    0x42a0_0000,
    0x4296_0000,
    0x434b_0000,
    0x4396_0000,
    0x43f0_0000,
    0x42a0_0001,
    0x429f_ffff,
    0xc2a0_0000,
    0x7f80_0000,
    0xff80_0000,
    0x7fc0_0000,
    0xffc0_0000,
    0x7fc1_2345,
    0xffc5_4321,
    0x7fa0_0001,
    0xff80_0001,
    0x7f80_0001,
];

// --- edge cases ----------------------------------------------------------

#[test]
fn edge_rotation_every_angle() {
    let mut angles: Vec<c_long> = (-1000..=1000).collect();
    angles.extend([c_long::MIN, c_long::MIN + 1, c_long::MAX - 1, c_long::MAX]);
    // Angles past 32 bits, where long has them: 90, 180 and 270 plus 2^32
    // are not 90, 180 or 270.
    if let Some(big) = c_long::from(1_u8).checked_shl(32) {
        angles.extend([big + 90, big + 180, big + 270, -big + 90, big]);
    }
    for (w, h) in [(1920, 1080), (1080, 1920), (0, 0), (1, u32::MAX)] {
        for &angle in &angles {
            let (x, y) = model_rotation(angle, w, h);
            assert_eq!(bits(rotation_x(angle, w, h)), bits(x), "x at {angle}");
            assert_eq!(bits(rotation_y(angle, w, h)), bits(y), "y at {angle}");
        }
    }
}

#[test]
fn edge_rotation_sizes() {
    let mut sizes: Vec<u32> = (0..=1024).collect();
    for base in [1_u32 << 24, 1 << 25, 1 << 31, u32::MAX - 300] {
        sizes.extend(base.saturating_sub(300)..=base.saturating_add(300));
    }
    sizes.extend([u32::MAX - 1, u32::MAX]);
    for &size in &sizes {
        let want = bits(model_size(size));
        assert_eq!(bits(rotation_y(90, size, 7)), want, "{size}");
        assert_eq!(bits(rotation_x(180, size, 7)), want, "{size}");
        assert_eq!(bits(rotation_y(180, 7, size)), want, "{size}");
        assert_eq!(bits(rotation_x(270, 7, size)), want, "{size}");
    }
}

#[test]
fn edge_space_pairs() {
    let spaces = edge_spaces();
    for &s in &spaces {
        for &c in &spaces {
            for gs in [false, true] {
                assert_eq!(
                    async_linear_srgb(s, c, gs),
                    model_linear(s, c, gs),
                    "{s} {c} {gs}"
                );
            }
            for na in [false, true] {
                assert_eq!(
                    technique(s, c, na),
                    model_technique(s, c, na),
                    "{s} {c} {na}"
                );
            }
            assert_eq!(reads_white_level(s, c), model_reads_white(s, c), "{s} {c}");
            assert_eq!(convert(s, c), model_convert(s, c), "{s} {c}");
            assert_eq!(
                bits(multiplier(s, c, 300.0)),
                bits(model_multiplier(s, c, 300.0)),
                "{s} {c}"
            );
        }
        for linear in [false, true] {
            for alpha in [false, true] {
                assert_eq!(
                    async_nonlinear_alpha(s, linear, alpha),
                    model_nonlinear(s, linear, alpha)
                );
            }
        }
    }
}

#[test]
fn edge_multiplier_special_levels() {
    let spaces = [-1, 0, 1, 2, 3, 4, 99, i32::MIN, i32::MAX];
    for level in SPECIAL_LEVELS {
        let w = f32::from_bits(level);
        for s in spaces {
            for c in spaces {
                assert_eq!(
                    bits(multiplier(s, c, w)),
                    bits(model_multiplier(s, c, w)),
                    "{s} into {c} at {level:#010x}"
                );
            }
        }
    }
}

#[test]
fn edge_white_level_read_iff_multiplier_moves() {
    // At 300 nits every pair that reads the level gives something other
    // than 1, and every pair that does not gives exactly 1.
    let spaces = edge_spaces();
    for &s in &spaces {
        for &c in &spaces {
            let reads = reads_white_level(s, c);
            assert_eq!(bits(multiplier(s, c, 300.0)) != ONE, reads, "{s} {c}");
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4000))]

    #[test]
    fn edge_prop_rotation(
        rotation in prop_oneof![
            Just(90 as c_long),
            Just(180 as c_long),
            Just(270 as c_long),
            0 as c_long..360,
            any::<c_long>()
        ],
        w in any::<u32>(),
        h in any::<u32>()
    ) {
        let (x, y) = model_rotation(rotation, w, h);
        prop_assert_eq!(bits(rotation_x(rotation, w, h)), bits(x));
        prop_assert_eq!(bits(rotation_y(rotation, w, h)), bits(y));
    }

    #[test]
    fn edge_prop_space_pairs(
        s in prop_oneof![-2_i32..6, any::<i32>()],
        c in prop_oneof![-2_i32..6, any::<i32>()],
        gs in any::<bool>(),
        alpha in any::<bool>(),
        level in any::<u32>()
    ) {
        let linear = async_linear_srgb(s, c, gs);
        prop_assert_eq!(linear, model_linear(s, c, gs));
        let na = async_nonlinear_alpha(s, linear, alpha);
        prop_assert_eq!(na, model_nonlinear(s, linear, alpha));
        prop_assert_eq!(technique(s, c, na), model_technique(s, c, na));
        prop_assert_eq!(reads_white_level(s, c), model_reads_white(s, c));
        prop_assert_eq!(convert(s, c), model_convert(s, c));
        let w = f32::from_bits(level);
        prop_assert_eq!(bits(multiplier(s, c, w)), bits(model_multiplier(s, c, w)));
    }
}
