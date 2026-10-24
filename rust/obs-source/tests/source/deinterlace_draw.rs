//! Tier 1: `obs_source::deinterlace_draw`, the deinterlace draw decisions.
//!
//! The `steps_*` cases port `test/cmocka/test_deinterlace_draw_plan_steps.c`
//! 1:1, under the same case names, in the same order, with the same inputs
//! and expectations, the float multipliers compared as bits.
//!
//! The `edge_*` cases are new: the mode functions over the ends of `i32`
//! and every value from -1000 to 1000; the technique, the white level read
//! and the multiplier over every pair of 260 spaces around the enum and the
//! ends of `i32`; the multiplier at every special `f32` bit pattern against
//! the one division; frame2 over the special values of the `u64` range
//! against a model of the C's `uint64_t` arithmetic in `i128` with the
//! modulo explicit; every mode change; and proptests of each over full-range
//! values (white levels as any bit pattern).

use obs_source::deinterlace_draw::{
    Effect, GS_CS_709_EXTENDED, GS_CS_709_SCRGB, GS_CS_SRGB, GS_CS_SRGB_16F, MODE_BLEND,
    MODE_BLEND_2X, MODE_DISABLE, MODE_DISCARD, MODE_LINEAR, MODE_LINEAR_2X, MODE_RETRO, MODE_YADIF,
    MODE_YADIF_2X, ModeChange, Technique, effect, effect_index, frame2, linear_required,
    linear_srgb, mode_change, mode_known, multiplier, reads_linear_srgb, reads_white_level, skip,
    technique,
};
use obs_source::deinterlace_pacing::TWOX_TOLERANCE;
use proptest::prelude::*;

const TWOX: u64 = TWOX_TOLERANCE;

/// Modes outside `enum obs_deinterlace_mode`, as the C test's
/// `OUTSIDE_MODES`.
const OUTSIDE_MODES: [i32; 5] = [-1, 99, i32::MIN, i32::MAX, 9];

/// Colour spaces outside `enum gs_color_space`, as the C test's
/// `OUTSIDE_SPACES`.
const OUTSIDE_SPACES: [i32; 5] = [-1, 4, 99, i32::MIN, i32::MAX];

const SPACES: [i32; 4] = [
    GS_CS_SRGB,
    GS_CS_SRGB_16F,
    GS_CS_709_EXTENDED,
    GS_CS_709_SCRGB,
];

const ONE: u32 = 0x3f80_0000;

// --- enum values ---------------------------------------------------------

#[test]
fn steps_enum_values() {
    // The C maps these numbers to effects, names and actions.
    assert_eq!(effect_index(MODE_DISABLE), None);
    let effects = [
        (Effect::Discard, 0),
        (Effect::Discard2x, 1),
        (Effect::Blend, 2),
        (Effect::Blend2x, 3),
        (Effect::Linear, 4),
        (Effect::Linear2x, 5),
        (Effect::Yadif, 6),
        (Effect::Yadif2x, 7),
    ];
    for (mode, (effect_value, index)) in (MODE_DISCARD..=MODE_YADIF_2X).zip(effects) {
        assert_eq!(effect(mode), Some(effect_value));
        assert_eq!(effect_index(mode), Some(index));
    }
    assert_eq!(Technique::Draw as i32, 0);
    assert_eq!(Technique::DrawMultiply as i32, 1);
    assert_eq!(Technique::DrawTonemap as i32, 2);
    assert_eq!(Technique::DrawMultiplyTonemap as i32, 3);
    assert_eq!(ModeChange::Same as i32, 0);
    assert_eq!(ModeChange::Enable as i32, 1);
    assert_eq!(ModeChange::Disable as i32, 2);
    assert_eq!(ModeChange::Switch as i32, 3);
    assert_eq!(Effect::Discard as i32, 0);
    assert_eq!(Effect::Yadif2x as i32, 7);
}

// --- effect and linear rule per mode -------------------------------------

#[test]
fn steps_effect_every_mode() {
    assert_eq!(effect(MODE_DISABLE), None);
    assert_eq!(effect(MODE_DISCARD), Some(Effect::Discard));
    // RETRO is discard 2x.
    assert_eq!(effect(MODE_RETRO), Some(Effect::Discard2x));
    assert_eq!(effect(MODE_BLEND), Some(Effect::Blend));
    assert_eq!(effect(MODE_BLEND_2X), Some(Effect::Blend2x));
    assert_eq!(effect(MODE_LINEAR), Some(Effect::Linear));
    assert_eq!(effect(MODE_LINEAR_2X), Some(Effect::Linear2x));
    assert_eq!(effect(MODE_YADIF), Some(Effect::Yadif));
    assert_eq!(effect(MODE_YADIF_2X), Some(Effect::Yadif2x));
}

#[test]
fn steps_effect_outside_enum() {
    for mode in OUTSIDE_MODES {
        assert_eq!(effect(mode), None, "mode {mode}");
        assert_eq!(effect_index(mode), None, "mode {mode}");
    }
}

#[test]
fn steps_linear_required_every_mode() {
    assert!(!linear_required(MODE_DISABLE));
    assert!(!linear_required(MODE_DISCARD));
    assert!(!linear_required(MODE_RETRO));
    assert!(linear_required(MODE_BLEND));
    assert!(linear_required(MODE_BLEND_2X));
    assert!(linear_required(MODE_LINEAR));
    assert!(linear_required(MODE_LINEAR_2X));
    assert!(linear_required(MODE_YADIF));
    assert!(linear_required(MODE_YADIF_2X));
    for mode in OUTSIDE_MODES {
        assert!(!linear_required(mode), "mode {mode}");
    }
}

// --- the early return ------------------------------------------------------

#[test]
fn steps_skip_without_texture_or_size() {
    let sizes = [0, 1, 1080, u32::MAX];
    for cur in [false, true] {
        for prev in [false, true] {
            for width in sizes {
                for height in sizes {
                    let draws = cur && prev && width != 0 && height != 0;
                    assert_eq!(
                        skip(cur, prev, width, height),
                        !draws,
                        "skip({cur}, {prev}, {width}, {height})"
                    );
                }
            }
        }
    }
    assert!(!skip(true, true, 1920, 1080));
    assert!(!skip(true, true, 1, 1));
    assert!(skip(false, true, 1920, 1080));
    assert!(skip(true, false, 1920, 1080));
    assert!(skip(true, true, 0, 1080));
    assert!(skip(true, true, 1920, 0));
}

// --- linear sRGB -------------------------------------------------------------

#[test]
fn steps_reads_linear_srgb_every_space() {
    assert!(reads_linear_srgb(GS_CS_SRGB));
    assert!(!reads_linear_srgb(GS_CS_SRGB_16F));
    assert!(!reads_linear_srgb(GS_CS_709_EXTENDED));
    assert!(!reads_linear_srgb(GS_CS_709_SCRGB));
    for space in OUTSIDE_SPACES {
        assert!(!reads_linear_srgb(space), "space {space}");
    }
}

#[test]
fn steps_linear_srgb_every_input() {
    for mode in MODE_DISABLE..=MODE_YADIF_2X {
        let required = mode >= MODE_BLEND;
        for gs in [false, true] {
            assert_eq!(linear_srgb(GS_CS_SRGB, gs, mode), gs || required);
            for space in SPACES.into_iter().skip(1).chain(OUTSIDE_SPACES) {
                assert!(linear_srgb(space, gs, mode), "{space} {gs} {mode}");
            }
        }
    }
    for mode in OUTSIDE_MODES {
        assert!(!linear_srgb(GS_CS_SRGB, false, mode));
        assert!(linear_srgb(GS_CS_SRGB, true, mode));
        assert!(linear_srgb(GS_CS_709_SCRGB, false, mode));
    }
}

// --- technique and multiplier per space pair -------------------------------

/// `[source][current]`, in `SPACES` order.
const TECHNIQUES: [[Technique; 4]; 4] = [
    // GS_CS_SRGB into SRGB, SRGB_16F, 709_EXTENDED, 709_SCRGB
    [
        Technique::Draw,
        Technique::Draw,
        Technique::Draw,
        Technique::DrawMultiply,
    ],
    // GS_CS_SRGB_16F
    [
        Technique::Draw,
        Technique::Draw,
        Technique::Draw,
        Technique::DrawMultiply,
    ],
    // GS_CS_709_EXTENDED
    [
        Technique::DrawTonemap,
        Technique::DrawTonemap,
        Technique::Draw,
        Technique::DrawMultiply,
    ],
    // GS_CS_709_SCRGB
    [
        Technique::DrawMultiplyTonemap,
        Technique::DrawMultiplyTonemap,
        Technique::DrawMultiply,
        Technique::Draw,
    ],
];

const WHITES: [f32; 3] = [80.0, 300.0, 0.0];

/// The multiplier at a white level of 80, 300 and 0, as bits:
/// `[source][current][white]`.
const MULTIPLIERS: [[[u32; 3]; 4]; 4] = [
    // GS_CS_SRGB
    [
        [ONE, ONE, ONE],
        [ONE, ONE, ONE],
        [ONE, ONE, ONE],
        // 80 / 80, 300 / 80 = 3.75, 0 / 80
        [ONE, 0x4070_0000, 0x0000_0000],
    ],
    // GS_CS_SRGB_16F
    [
        [ONE, ONE, ONE],
        [ONE, ONE, ONE],
        [ONE, ONE, ONE],
        [ONE, 0x4070_0000, 0x0000_0000],
    ],
    // GS_CS_709_EXTENDED: tonemapped into SDR at 1
    [
        [ONE, ONE, ONE],
        [ONE, ONE, ONE],
        [ONE, ONE, ONE],
        [ONE, 0x4070_0000, 0x0000_0000],
    ],
    // GS_CS_709_SCRGB: 80 / 80, 80 / 300 rounded, 80 / 0 = +inf
    // (characterized, not endorsed)
    [
        [ONE, 0x3e88_8889, 0x7f80_0000],
        [ONE, 0x3e88_8889, 0x7f80_0000],
        [ONE, 0x3e88_8889, 0x7f80_0000],
        [ONE, ONE, ONE],
    ],
];

#[test]
fn steps_technique_space_pairs() {
    for (s, source) in SPACES.into_iter().enumerate() {
        for (c, current) in SPACES.into_iter().enumerate() {
            let expected = TECHNIQUES[s][c];
            assert_eq!(
                technique(source, current),
                expected,
                "{source} into {current}"
            );
            let multiplies = matches!(
                expected,
                Technique::DrawMultiply | Technique::DrawMultiplyTonemap
            );
            assert_eq!(reads_white_level(source, current), multiplies);
        }
    }
}

#[test]
fn steps_technique_outside_enum() {
    for outside in OUTSIDE_SPACES {
        for space in SPACES {
            assert_eq!(technique(outside, space), Technique::Draw);
            assert_eq!(technique(space, outside), Technique::Draw);
            assert!(!reads_white_level(outside, space));
            assert!(!reads_white_level(space, outside));
            assert_eq!(multiplier(outside, space, 300.0).to_bits(), ONE);
            assert_eq!(multiplier(space, outside, 300.0).to_bits(), ONE);
        }
    }
}

#[test]
fn steps_multiplier_space_pairs() {
    for (s, source) in SPACES.into_iter().enumerate() {
        for (c, current) in SPACES.into_iter().enumerate() {
            for (w, white) in WHITES.into_iter().enumerate() {
                let bits = multiplier(source, current, white).to_bits();
                assert_eq!(
                    bits, MULTIPLIERS[s][c][w],
                    "{source} into {current} at {white}: {bits:#010x}"
                );
            }
        }
    }
}

#[test]
fn steps_multiplier_any_level() {
    assert_eq!(
        multiplier(GS_CS_709_SCRGB, GS_CS_SRGB, -0.0).to_bits(),
        0xff80_0000
    );
    assert_eq!(
        multiplier(GS_CS_SRGB, GS_CS_709_SCRGB, -0.0).to_bits(),
        0x8000_0000
    );
    assert_eq!(
        multiplier(GS_CS_709_SCRGB, GS_CS_709_EXTENDED, f32::INFINITY).to_bits(),
        0
    );
    assert_eq!(
        multiplier(GS_CS_709_EXTENDED, GS_CS_709_SCRGB, f32::INFINITY).to_bits(),
        0x7f80_0000
    );
    assert_eq!(
        multiplier(GS_CS_SRGB_16F, GS_CS_709_SCRGB, -80.0).to_bits(),
        0xbf80_0000
    );
    assert_eq!(
        multiplier(GS_CS_709_SCRGB, GS_CS_SRGB_16F, -80.0).to_bits(),
        0xbf80_0000
    );
    assert_eq!(
        multiplier(GS_CS_SRGB, GS_CS_709_SCRGB, 480.0).to_bits(),
        0x40c0_0000
    );
    assert_eq!(
        multiplier(GS_CS_709_SCRGB, GS_CS_SRGB, 480.0).to_bits(),
        0x3e2a_aaab
    );
    assert_eq!(
        multiplier(GS_CS_709_SCRGB, GS_CS_SRGB, f32::from_bits(1)).to_bits(),
        0x7f80_0000
    );
    assert!(multiplier(GS_CS_709_SCRGB, GS_CS_SRGB, f32::NAN).is_nan());
    assert!(multiplier(GS_CS_SRGB, GS_CS_709_SCRGB, f32::NAN).is_nan());
    assert_eq!(multiplier(GS_CS_SRGB, GS_CS_SRGB, f32::NAN).to_bits(), ONE);
    assert_eq!(
        multiplier(GS_CS_709_EXTENDED, GS_CS_SRGB, 0.0).to_bits(),
        ONE
    );
}

// --- the second field ------------------------------------------------------

#[test]
fn steps_frame2_at_threshold() {
    let frame_ts = 5_000_000_000_u64;
    let offset = 995_000_000_000_u64;
    let half = 20_000_000_u32;
    let at = frame_ts + offset + u64::from(half) - TWOX;
    assert!(!frame2(at - 1, frame_ts, offset, half));
    assert!(frame2(at, frame_ts, offset, half));
    assert!(frame2(at + 1, frame_ts, offset, half));
    assert!(!frame2(0, frame_ts, offset, half));
    assert!(frame2(u64::MAX, frame_ts, offset, half));
    let far = frame_ts + offset + u64::from(u32::MAX) - TWOX;
    assert!(!frame2(far - 1, frame_ts, offset, u32::MAX));
    assert!(frame2(far, frame_ts, offset, u32::MAX));
    let twox = u32::try_from(TWOX).unwrap();
    assert!(!frame2(frame_ts + offset - 1, frame_ts, offset, twox));
    assert!(frame2(frame_ts + offset, frame_ts, offset, twox));
}

#[test]
fn steps_frame2_sum_wraps() {
    let frame_ts = u64::MAX - 5;
    let offset = 10;
    let half = u32::try_from(TWOX).unwrap();
    assert!(!frame2(3, frame_ts, offset, half));
    assert!(frame2(4, frame_ts, offset, half));
    assert!(frame2(5, frame_ts, offset, half));
    assert!(!frame2(19_999_998, 1_000_000, u64::MAX, 20_000_000));
    assert!(frame2(19_999_999, 1_000_000, u64::MAX, 20_000_000));
    // 2 * (2^64 - 1) + (2^32 - 1) - TWOX, modulo 2^64
    let at = u64::MAX
        .wrapping_add(u64::MAX)
        .wrapping_add(u64::from(u32::MAX))
        .wrapping_sub(TWOX);
    assert!(!frame2(at - 1, u64::MAX, u64::MAX, u32::MAX));
    assert!(frame2(at, u64::MAX, u64::MAX, u32::MAX));
}

#[test]
fn steps_frame2_below_tolerance_wraps() {
    let twox = u32::try_from(TWOX).unwrap();
    assert!(!frame2(0, 0, 0, 0));
    assert!(!frame2(u64::MAX - TWOX, 0, 0, 0));
    assert!(frame2(u64::MAX - TWOX + 1, 0, 0, 0));
    assert!(!frame2(u64::MAX - 1, 0, 0, twox - 1));
    assert!(frame2(u64::MAX, 0, 0, twox - 1));
    assert!(frame2(0, 1, 0, twox - 1));
    assert!(frame2(0, 0, 500_000, 500_000));
    assert!(!frame2(0, 0, 499_999, 500_000));
    assert!(!frame2(4_000_000 - TWOX - 1, 1_000_000, 3_000_000, 0));
    assert!(frame2(4_000_000 - TWOX, 1_000_000, 3_000_000, 0));
}

// --- obs_source_set_deinterlace_mode() --------------------------------------

/// The C test's `expected_change()`.
fn expected_change(current: i32, mode: i32) -> ModeChange {
    let mode = if (MODE_DISABLE..=MODE_YADIF_2X).contains(&mode) {
        mode
    } else {
        MODE_DISABLE
    };
    if current == mode {
        ModeChange::Same
    } else if current == MODE_DISABLE {
        ModeChange::Enable
    } else if mode == MODE_DISABLE {
        ModeChange::Disable
    } else {
        ModeChange::Switch
    }
}

#[test]
fn steps_mode_change_every_pair() {
    for current in MODE_DISABLE..=MODE_YADIF_2X {
        for mode in MODE_DISABLE..=MODE_YADIF_2X {
            assert_eq!(
                mode_change(current, mode),
                expected_change(current, mode),
                "{current} to {mode}"
            );
        }
    }
    assert_eq!(mode_change(MODE_DISABLE, MODE_DISABLE), ModeChange::Same);
    assert_eq!(mode_change(MODE_DISABLE, MODE_YADIF_2X), ModeChange::Enable);
    assert_eq!(
        mode_change(MODE_YADIF_2X, MODE_DISABLE),
        ModeChange::Disable
    );
    assert_eq!(mode_change(MODE_BLEND, MODE_YADIF_2X), ModeChange::Switch);
    assert_eq!(mode_change(MODE_RETRO, MODE_RETRO), ModeChange::Same);
}

#[test]
fn steps_mode_change_outside_enum() {
    for mode in OUTSIDE_MODES {
        assert_eq!(mode_change(MODE_DISABLE, mode), ModeChange::Same);
        for current in MODE_DISCARD..=MODE_YADIF_2X {
            assert_eq!(mode_change(current, mode), ModeChange::Disable);
        }
    }
}

#[test]
fn steps_mode_change_current_outside_enum() {
    for mode in OUTSIDE_MODES {
        assert_eq!(mode_change(mode, mode), ModeChange::Disable);
        assert_eq!(mode_change(mode, MODE_DISABLE), ModeChange::Disable);
        assert_eq!(mode_change(mode, MODE_YADIF), ModeChange::Switch);
    }
}

// --- edge cases ---------------------------------------------------------------

/// Modes around the enum and the ends of `i32`.
fn edge_modes() -> impl Iterator<Item = i32> {
    (-1000..=1000).chain([
        i32::MIN,
        i32::MIN + 1,
        i32::MAX - 1,
        i32::MAX,
        1 << 8,
        (1 << 8) + 1,
        1 << 16,
        (1 << 16) + 8,
        i32::MAX - 8,
    ])
}

/// Spaces: -32 to 223 and four near the ends of `i32`, 260 values.
fn edge_spaces() -> Vec<i32> {
    let mut spaces: Vec<i32> = (-32..224).collect();
    spaces.extend([i32::MIN, i32::MIN + 3, i32::MAX - 3, i32::MAX]);
    spaces
}

#[test]
fn edge_mode_functions() {
    for mode in edge_modes() {
        let in_enum = (MODE_DISABLE..=MODE_YADIF_2X).contains(&mode);
        assert_eq!(mode_known(mode), in_enum, "mode {mode}");
        assert_eq!(effect(mode).is_some(), in_enum && mode != MODE_DISABLE);
        assert_eq!(
            effect_index(mode).map(i32::from),
            effect(mode).map(|e| e as i32)
        );
        if let Some(index) = effect_index(mode) {
            assert_eq!(i32::from(index), mode - 1, "the index is mode - 1");
        }
        assert_eq!(
            linear_required(mode),
            (MODE_BLEND..=MODE_YADIF_2X).contains(&mode)
        );
    }
}

#[test]
fn edge_space_pairs() {
    let spaces = edge_spaces();
    for &source in &spaces {
        let source_in = (GS_CS_SRGB..=GS_CS_709_SCRGB).contains(&source);
        assert_eq!(reads_linear_srgb(source), source == GS_CS_SRGB);
        for &current in &spaces {
            let current_in = (GS_CS_SRGB..=GS_CS_709_SCRGB).contains(&current);
            let t = technique(source, current);
            if source_in && current_in {
                let (s, c) = (
                    usize::try_from(source).unwrap(),
                    usize::try_from(current).unwrap(),
                );
                assert_eq!(t, TECHNIQUES[s][c]);
            } else {
                assert_eq!(t, Technique::Draw, "{source} into {current}");
                assert!(!reads_white_level(source, current));
                assert_eq!(multiplier(source, current, 300.0).to_bits(), ONE);
            }
        }
    }
}

/// Special `f32` bit patterns: zeros, subnormals, the normal ends, the
/// levels OBS uses, the infinities, quiet and signalling NaNs of both
/// signs with payloads.
const SPECIAL_WHITES: [u32; 26] = [
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
    0x42a0_0001,
    0x429f_ffff,
    0x4396_0000,
    0x43f0_0000,
    0x4348_0000,
    0xc2a0_0000,
    0x7f80_0000,
    0xff80_0000,
    0x7fc0_0000,
    0xffc0_0000,
    0x7fc1_2345,
    0xffff_ffff,
    0x7f80_0001,
    0xff80_0001,
    0x7fbf_ffff,
    0x3c23_d70a,
];

/// The C's one division, written out: what the multiplier must be.
fn model_multiplier(source: i32, current: i32, white: f32) -> f32 {
    match (technique(source, current), source) {
        (Technique::DrawMultiply | Technique::DrawMultiplyTonemap, GS_CS_709_SCRGB) => 80.0 / white,
        (Technique::DrawMultiply | Technique::DrawMultiplyTonemap, _) => white / 80.0,
        _ => 1.0,
    }
}

#[test]
fn edge_multiplier_special_levels() {
    for source in SPACES.into_iter().chain(OUTSIDE_SPACES) {
        for current in SPACES.into_iter().chain(OUTSIDE_SPACES) {
            for bits in SPECIAL_WHITES {
                let white = f32::from_bits(bits);
                let got = multiplier(source, current, white);
                let want = model_multiplier(source, current, white);
                assert_eq!(
                    got.to_bits(),
                    want.to_bits(),
                    "{source} into {current} at {bits:#010x}"
                );
                if reads_white_level(source, current) {
                    assert_eq!(got.is_nan(), white.is_nan());
                } else {
                    assert_eq!(got.to_bits(), ONE);
                }
            }
        }
    }
}

/// `frame_ts + offset + half - TWOX` modulo 2^64, in `i128`.
fn model_frame2(video_time: u64, frame_ts: u64, offset: u64, half: u32) -> bool {
    let sum = i128::from(frame_ts) + i128::from(offset) + i128::from(half) - i128::from(TWOX);
    let threshold = sum.rem_euclid(1_i128 << 64);
    i128::from(video_time) >= threshold
}

const EDGE_U64: [u64; 16] = [
    0,
    1,
    999_999,
    1_000_000,
    1_000_001,
    2_000_000,
    16_666_667,
    20_000_000,
    0xffff_ffff,
    0x1_0000_0000,
    0x7fff_ffff_ffff_ffff,
    0x8000_0000_0000_0000,
    u64::MAX - 1_000_000,
    u64::MAX - 999_999,
    u64::MAX - 1,
    u64::MAX,
];

const EDGE_U32: [u32; 8] = [
    0,
    1,
    999_999,
    1_000_000,
    1_000_001,
    20_000_000,
    0x7fff_ffff,
    u32::MAX,
];

#[test]
fn edge_frame2_special_values() {
    for video_time in EDGE_U64 {
        for frame_ts in EDGE_U64 {
            for offset in EDGE_U64 {
                for half in EDGE_U32 {
                    assert_eq!(
                        frame2(video_time, frame_ts, offset, half),
                        model_frame2(video_time, frame_ts, offset, half),
                        "frame2({video_time}, {frame_ts}, {offset}, {half})"
                    );
                }
            }
        }
    }
}

#[test]
fn edge_frame2_around_threshold() {
    for frame_ts in EDGE_U64 {
        for offset in EDGE_U64 {
            for half in EDGE_U32 {
                let at = frame_ts
                    .wrapping_add(offset)
                    .wrapping_add(u64::from(half))
                    .wrapping_sub(TWOX);
                for delta in [-2_i64, -1, 0, 1, 2] {
                    let video_time = at.wrapping_add_signed(delta);
                    assert_eq!(
                        frame2(video_time, frame_ts, offset, half),
                        model_frame2(video_time, frame_ts, offset, half)
                    );
                }
                assert!(frame2(at, frame_ts, offset, half), "due at the threshold");
            }
        }
    }
}

#[test]
fn edge_mode_change_every_pair() {
    let modes: Vec<i32> = (-3..=12).chain([99, i32::MIN, i32::MAX, 1 << 16]).collect();
    for &current in &modes {
        for &mode in &modes {
            assert_eq!(
                mode_change(current, mode),
                expected_change(current, mode),
                "{current} to {mode}"
            );
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4000))]

    #[test]
    fn edge_prop_frame2(
        video_time in any::<u64>(),
        frame_ts in any::<u64>(),
        offset in any::<u64>(),
        half in any::<u32>(),
        near in any::<bool>(),
        delta in -3_i64..=3,
    ) {
        let video_time = if near {
            frame_ts
                .wrapping_add(offset)
                .wrapping_add(u64::from(half))
                .wrapping_sub(TWOX)
                .wrapping_add_signed(delta)
        } else {
            video_time
        };
        prop_assert_eq!(
            frame2(video_time, frame_ts, offset, half),
            model_frame2(video_time, frame_ts, offset, half)
        );
    }

    #[test]
    fn edge_prop_multiplier(
        source in prop_oneof![0_i32..4, any::<i32>()],
        current in prop_oneof![0_i32..4, any::<i32>()],
        bits in any::<u32>(),
    ) {
        let white = f32::from_bits(bits);
        prop_assert_eq!(
            multiplier(source, current, white).to_bits(),
            model_multiplier(source, current, white).to_bits()
        );
    }

    #[test]
    fn edge_prop_modes(current in prop_oneof![-1_i32..10, any::<i32>()],
                       mode in prop_oneof![-1_i32..10, any::<i32>()]) {
        prop_assert_eq!(mode_change(current, mode), expected_change(current, mode));
        prop_assert_eq!(mode_known(mode), (MODE_DISABLE..=MODE_YADIF_2X).contains(&mode));
        prop_assert_eq!(linear_required(mode), (MODE_BLEND..=MODE_YADIF_2X).contains(&mode));
    }

    #[test]
    fn edge_prop_linear_srgb(source in prop_oneof![0_i32..4, any::<i32>()],
                             gs in any::<bool>(),
                             mode in prop_oneof![-1_i32..10, any::<i32>()]) {
        prop_assert_eq!(
            linear_srgb(source, gs, mode),
            source != GS_CS_SRGB || gs || linear_required(mode)
        );
    }
}
