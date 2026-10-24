//! Tier 3: the `obs_source_draw_plan_*` shims against the original C
//! (`obs-c-oracle`, `oracle/source_draw_plan.c`: the unmodified
//! `libobs/obs-source-draw-plan.c.h` with its functions renamed).
//!
//! Every function takes and returns plain values, so a run compares the
//! bytes of the two results: the C's `bool` as the byte it returns, the
//! technique as its value, the `float` results as their bits.
//!
//! Runs:
//! - the enum values and the type sizes against the C headers (`long` is
//!   [`c_long`] on every target), and the technique names;
//! - the rotation translate for every angle from -70000 to 70000, every
//!   65521st `c_long` over the whole range, the ends and (where `long` is 64
//!   bits) 90, 180 and 270 plus multiples of 2^32, at 6 sizes; and every
//!   size from 0 to 4096 and around 2^24, 2^25, 2^31 and `u32::MAX` at 90,
//!   180 and 270;
//! - space pairs: every decision (both graphics linear flags, both
//!   non-linear alpha flags, the white level read, the conversion and the
//!   multiplier at 300 nits) for every pair of 649 spaces (-320 to 320 and
//!   four near each end of `i32`); every pair of -3 to 6 and the ends at 26
//!   special white levels; the non-linear alpha rule for every space and
//!   flag;
//! - the decisions of `obs_source_render_async_video()` and
//!   `source_render()` in their order (the linear flag, the blend, the
//!   technique, the white level read and the multiplier only when it is
//!   read), one copy on each side;
//! - proptests (20,000 cases each): the rotation over any `c_long` and any
//!   sizes; every decision over any space and any white level bit pattern
//!   (never a float range); both callers' decisions over all of it at once.
//!
//! No input reaches an integer division or a 128-bit multiply on either
//! side (the only division is the `f32` multiplier, which cannot trap), so
//! no input is skipped on MSVC x64. No intentional differences: there is no
//! pointer to check.

use std::ffi::CStr;
use std::os::raw::{c_int, c_long};

use obs_c_oracle::source_draw_plan as oracle;
use obs_source::draw_plan::{
    GS_CS_709_EXTENDED, GS_CS_709_SCRGB, GS_CS_SRGB, GS_CS_SRGB_16F, Technique,
};
use obs_source::ffi::source_draw_plan as shim;
use proptest::prelude::*;

const TECHNIQUES: [Technique; 6] = [
    Technique::Draw,
    Technique::DrawMultiply,
    Technique::DrawTonemap,
    Technique::DrawMultiplyTonemap,
    Technique::DrawNonlinearAlpha,
    Technique::DrawNonlinearAlphaMultiply,
];

#[test]
fn values_match_the_c_headers() {
    let rust: [i64; 16] = [
        GS_CS_SRGB.into(),
        GS_CS_SRGB_16F.into(),
        GS_CS_709_EXTENDED.into(),
        GS_CS_709_SCRGB.into(),
        TECHNIQUES[0].value().into(),
        TECHNIQUES[1].value().into(),
        TECHNIQUES[2].value().into(),
        TECHNIQUES[3].value().into(),
        TECHNIQUES[4].value().into(),
        TECHNIQUES[5].value().into(),
        4,
        4,
        4,
        i64::try_from(std::mem::size_of::<c_long>()).unwrap(),
        1,
        4,
    ];
    for (which, (name, value)) in oracle::VALUE_NAMES.iter().zip(rust).enumerate() {
        // SAFETY: a pure lookup.
        let c = unsafe { oracle::oracle_source_draw_plan_value(which) };
        assert_eq!(value, c, "{name}");
    }
    // SAFETY: a pure lookup, one past the end.
    let past = unsafe { oracle::oracle_source_draw_plan_value(oracle::VALUE_NAMES.len()) };
    assert_eq!(past, i64::MIN, "the list has as many values as names");
    // The names C maps each value the port returns to are the port's.
    for technique in TECHNIQUES {
        let value = c_int::try_from(technique.value()).unwrap();
        // SAFETY: returns a static string for any value.
        let name = unsafe { CStr::from_ptr(oracle::oracle_source_draw_plan_technique_name(value)) };
        assert_eq!(name.to_str().unwrap(), technique.name());
    }
}

// --------------------------------------------------------------------------
// One call on each side

fn c_rotation(rotation: c_long, w: u32, h: u32) -> (u32, u32) {
    // SAFETY: plain values.
    unsafe {
        (
            oracle::oracle_obs_source_draw_plan_rotation_x(rotation, w, h).to_bits(),
            oracle::oracle_obs_source_draw_plan_rotation_y(rotation, w, h).to_bits(),
        )
    }
}

fn rs_rotation(rotation: c_long, w: u32, h: u32) -> (u32, u32) {
    (
        shim::obs_source_draw_plan_rotation_x(rotation, w, h).to_bits(),
        shim::obs_source_draw_plan_rotation_y(rotation, w, h).to_bits(),
    )
}

fn check_rotation(rotation: c_long, w: u32, h: u32) {
    assert_eq!(
        rs_rotation(rotation, w, h),
        c_rotation(rotation, w, h),
        "rotation {rotation} at {w}x{h}"
    );
}

fn c_linear(s: c_int, c: c_int, gs: bool) -> u8 {
    // SAFETY: plain values.
    unsafe { oracle::oracle_obs_source_draw_plan_async_linear_srgb(s, c, gs) }
}

fn c_nonlinear(s: c_int, linear: bool, alpha: bool) -> u8 {
    // SAFETY: plain values.
    unsafe { oracle::oracle_obs_source_draw_plan_async_nonlinear_alpha(s, linear, alpha) }
}

fn c_technique(s: c_int, c: c_int, na: bool) -> u32 {
    // SAFETY: plain values.
    unsafe { oracle::oracle_obs_source_draw_plan_technique(s, c, na) }
}

fn c_reads_white(s: c_int, c: c_int) -> u8 {
    // SAFETY: plain values.
    unsafe { oracle::oracle_obs_source_draw_plan_reads_white_level(s, c) }
}

fn c_multiplier(s: c_int, c: c_int, w: f32) -> u32 {
    // SAFETY: plain values.
    unsafe { oracle::oracle_obs_source_draw_plan_multiplier(s, c, w) }.to_bits()
}

fn c_convert(s: c_int, c: c_int) -> u8 {
    // SAFETY: plain values.
    unsafe { oracle::oracle_obs_source_draw_plan_convert(s, c) }
}

fn check_pair(s: c_int, c: c_int, w: f32) {
    for gs in [false, true] {
        assert_eq!(
            u8::from(shim::obs_source_draw_plan_async_linear_srgb(s, c, gs)),
            c_linear(s, c, gs),
            "linear {s} into {c}, flag {gs}"
        );
    }
    for na in [false, true] {
        assert_eq!(
            shim::obs_source_draw_plan_technique(s, c, na),
            c_technique(s, c, na),
            "technique {s} into {c}, non-linear {na}"
        );
    }
    assert_eq!(
        u8::from(shim::obs_source_draw_plan_reads_white_level(s, c)),
        c_reads_white(s, c),
        "reads white {s} into {c}"
    );
    assert_eq!(
        u8::from(shim::obs_source_draw_plan_convert(s, c)),
        c_convert(s, c),
        "convert {s} into {c}"
    );
    assert_eq!(
        shim::obs_source_draw_plan_multiplier(s, c, w).to_bits(),
        c_multiplier(s, c, w),
        "multiplier {s} into {c} at {:#010x}",
        w.to_bits()
    );
}

// --------------------------------------------------------------------------
// Inputs

fn spaces() -> Vec<c_int> {
    let mut v: Vec<c_int> = (-320..=320).collect();
    v.extend([
        c_int::MIN,
        c_int::MIN + 1,
        c_int::MIN + 2,
        c_int::MIN + 3,
        c_int::MAX - 3,
        c_int::MAX - 2,
        c_int::MAX - 1,
        c_int::MAX,
    ]);
    v
}

fn small_spaces() -> Vec<c_int> {
    let mut v: Vec<c_int> = (-3..=6).collect();
    v.extend([c_int::MIN, c_int::MAX]);
    v
}

/// Special `f32` white levels: zeros, subnormals, the normal ends, 80 to 480
/// nits, infinities, quiet and signalling NaNs of both signs with payloads.
const WHITES: [u32; 26] = [
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

const SIZES: [(u32, u32); 6] = [
    (1920, 1080),
    (1080, 1920),
    (0, 0),
    (16_777_217, 3),
    (u32::MAX, 1),
    (7, u32::MAX),
];

fn sweep_rotations() -> Vec<c_long> {
    let mut v: Vec<c_long> = (-70_000..=70_000).collect();
    let mut r = c_long::MIN;
    // A stride over the whole range, as far as it goes before overflowing.
    let stride: c_long = if std::mem::size_of::<c_long>() == 8 {
        c_long::MAX / 65_521
    } else {
        65_521
    };
    while let Some(next) = r.checked_add(stride) {
        v.push(r);
        r = next;
    }
    v.extend([c_long::MIN, c_long::MIN + 1, c_long::MAX - 1, c_long::MAX]);
    if let Some(big) = c_long::from(1_u8).checked_shl(32) {
        for k in [-3, -1, 1, 2, 7] {
            for base in [0, 90, 180, 270] {
                v.push(big * k + base);
            }
        }
    }
    v
}

// --------------------------------------------------------------------------
// Sweeps

#[test]
fn rotation_sweep() {
    let rotations = sweep_rotations();
    for (w, h) in SIZES {
        for &r in &rotations {
            check_rotation(r, w, h);
        }
    }
}

#[test]
fn rotation_every_size() {
    let mut sizes: Vec<u32> = (0..=4096).collect();
    for base in [1_u32 << 24, 1 << 25, 1 << 31, u32::MAX - 2000] {
        sizes.extend(base.saturating_sub(2000)..=base.saturating_add(2000));
    }
    sizes.push(u32::MAX);
    for r in [0, 90, 180, 270, 45] {
        for &size in &sizes {
            check_rotation(r, size, 7);
            check_rotation(r, 7, size);
            check_rotation(r, size, size);
        }
    }
}

#[test]
fn space_pairs_sweep() {
    let spaces = spaces();
    for &s in &spaces {
        for &c in &spaces {
            check_pair(s, c, 300.0);
        }
    }
}

#[test]
fn space_pairs_at_special_levels() {
    let spaces = small_spaces();
    for bits in WHITES {
        let w = f32::from_bits(bits);
        for &s in &spaces {
            for &c in &spaces {
                check_pair(s, c, w);
            }
        }
    }
}

#[test]
fn nonlinear_alpha_every_space_and_flag() {
    for s in spaces() {
        for linear in [false, true] {
            for alpha in [false, true] {
                assert_eq!(
                    u8::from(shim::obs_source_draw_plan_async_nonlinear_alpha(
                        s, linear, alpha
                    )),
                    c_nonlinear(s, linear, alpha),
                    "{s} {linear} {alpha}"
                );
            }
        }
    }
}

// --------------------------------------------------------------------------
// The callers' decisions in their order

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AsyncDraw {
    linear: u8,
    nonlinear: u8,
    technique: u32,
    reads_white: u8,
    multiplier: u32,
    x: u32,
    y: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ConvertDraw {
    convert: u8,
    technique: u32,
    reads_white: u8,
    multiplier: u32,
}

#[derive(Debug, Clone, Copy)]
struct DrawIn {
    s: c_int,
    c: c_int,
    gs: bool,
    alpha: bool,
    white: u32,
    rotation: c_long,
    w: u32,
    h: u32,
}

/// `obs_source_render_async_video()` and `rotate_async_video()`: the
/// linear flag, the blend, the technique, the white level only when read,
/// and the translate only for a rotation other than 0.
fn async_c(i: DrawIn) -> AsyncDraw {
    let linear = c_linear(i.s, i.c, i.gs);
    let nonlinear = c_nonlinear(i.s, linear != 0, i.alpha);
    let technique = c_technique(i.s, i.c, nonlinear != 0);
    let reads_white = c_reads_white(i.s, i.c);
    let multiplier = if reads_white != 0 {
        c_multiplier(i.s, i.c, f32::from_bits(i.white))
    } else {
        1.0_f32.to_bits()
    };
    let (x, y) = if i.rotation != 0 {
        c_rotation(i.rotation, i.w, i.h)
    } else {
        (u32::MAX, u32::MAX)
    };
    AsyncDraw {
        linear,
        nonlinear,
        technique,
        reads_white,
        multiplier,
        x,
        y,
    }
}

fn async_rust(i: DrawIn) -> AsyncDraw {
    let linear = shim::obs_source_draw_plan_async_linear_srgb(i.s, i.c, i.gs);
    let nonlinear = shim::obs_source_draw_plan_async_nonlinear_alpha(i.s, linear, i.alpha);
    let technique = shim::obs_source_draw_plan_technique(i.s, i.c, nonlinear);
    let reads_white = shim::obs_source_draw_plan_reads_white_level(i.s, i.c);
    let multiplier = if reads_white {
        shim::obs_source_draw_plan_multiplier(i.s, i.c, f32::from_bits(i.white)).to_bits()
    } else {
        1.0_f32.to_bits()
    };
    let (x, y) = if i.rotation != 0 {
        rs_rotation(i.rotation, i.w, i.h)
    } else {
        (u32::MAX, u32::MAX)
    };
    AsyncDraw {
        linear: u8::from(linear),
        nonlinear: u8::from(nonlinear),
        technique,
        reads_white: u8::from(reads_white),
        multiplier,
        x,
        y,
    }
}

/// `source_render()`: whether it converts, and when it does, the
/// technique (never with non-linear alpha) and the multiplier.
fn convert_c(i: DrawIn) -> ConvertDraw {
    let convert = c_convert(i.s, i.c);
    let technique = c_technique(i.s, i.c, false);
    let reads_white = c_reads_white(i.s, i.c);
    let multiplier = if reads_white != 0 {
        c_multiplier(i.s, i.c, f32::from_bits(i.white))
    } else {
        1.0_f32.to_bits()
    };
    ConvertDraw {
        convert,
        technique,
        reads_white,
        multiplier,
    }
}

fn convert_rust(i: DrawIn) -> ConvertDraw {
    let convert = shim::obs_source_draw_plan_convert(i.s, i.c);
    let technique = shim::obs_source_draw_plan_technique(i.s, i.c, false);
    let reads_white = shim::obs_source_draw_plan_reads_white_level(i.s, i.c);
    let multiplier = if reads_white {
        shim::obs_source_draw_plan_multiplier(i.s, i.c, f32::from_bits(i.white)).to_bits()
    } else {
        1.0_f32.to_bits()
    };
    ConvertDraw {
        convert: u8::from(convert),
        technique,
        reads_white: u8::from(reads_white),
        multiplier,
    }
}

#[test]
fn callers_every_space_flag_level_and_rotation() {
    let spaces = [-1, 0, 1, 2, 3, 4, 99, c_int::MIN, c_int::MAX];
    let rotations: [c_long; 7] = [0, 90, 180, 270, 45, -90, c_long::MAX];
    let mut runs = 0_u32;
    for s in spaces {
        for c in spaces {
            for gs in [false, true] {
                for alpha in [false, true] {
                    for white in WHITES {
                        for rotation in rotations {
                            for (w, h) in [(1920, 1080), (u32::MAX, 3)] {
                                let i = DrawIn {
                                    s,
                                    c,
                                    gs,
                                    alpha,
                                    white,
                                    rotation,
                                    w,
                                    h,
                                };
                                assert_eq!(async_rust(i), async_c(i), "{i:?}");
                                assert_eq!(convert_rust(i), convert_c(i), "{i:?}");
                                runs += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(runs, 9 * 9 * 2 * 2 * 26 * 7 * 2);
}

// --------------------------------------------------------------------------
// Proptests

fn any_space() -> impl Strategy<Value = c_int> {
    prop_oneof![
        -3_i32..7,
        any::<c_int>(),
        proptest::sample::select(vec![c_int::MIN, c_int::MAX])
    ]
}

fn any_white_bits() -> impl Strategy<Value = u32> {
    prop_oneof![any::<u32>(), proptest::sample::select(WHITES.to_vec())]
}

fn any_rotation() -> impl Strategy<Value = c_long> {
    prop_oneof![
        proptest::sample::select(vec![0 as c_long, 90, 180, 270]),
        0 as c_long..360,
        any::<c_long>()
    ]
}

fn any_draw() -> impl Strategy<Value = DrawIn> {
    (
        any_space(),
        any_space(),
        any::<bool>(),
        any::<bool>(),
        any_white_bits(),
        any_rotation(),
        any::<u32>(),
        any::<u32>(),
    )
        .prop_map(|(s, c, gs, alpha, white, rotation, w, h)| DrawIn {
            s,
            c,
            gs,
            alpha,
            white,
            rotation,
            w,
            h,
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20_000))]

    #[test]
    fn prop_rotation(rotation in any_rotation(), w in any::<u32>(), h in any::<u32>()) {
        prop_assert_eq!(rs_rotation(rotation, w, h), c_rotation(rotation, w, h));
    }

    #[test]
    fn prop_space_pair(s in any_space(), c in any_space(), white in any_white_bits()) {
        check_pair(s, c, f32::from_bits(white));
    }

    #[test]
    fn prop_callers(i in any_draw()) {
        prop_assert_eq!(async_rust(i), async_c(i));
        prop_assert_eq!(convert_rust(i), convert_c(i));
    }
}
