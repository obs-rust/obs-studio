//! Tier 1: `obs_source::audio_input`, the audio input steps of
//! `process_audio()` and `reset_resampler()`.
//!
//! The `steps_*` cases port `test/cmocka/test_source_audio_input_steps.c`
//! 1:1, under the same case names, in the same order, with the same inputs
//! and expectations, the "characterized, not endorsed" cases included.
//!
//! The `edge_*` cases are new: the decisions against models written
//! another way over full-range `u64` timestamps, any formats and any `f32`
//! bit pattern, and the loops against per-sample models over random
//! samples, frame counts and plane counts.

use obs_source::audio_input::{
    FORCE_MONO, Format, LINEAR, Law, SINE_LAW, SMOOTHING_THRESHOLD, SPEAKERS_STEREO, SQUARE_LAW,
    add_plane, balance, balance_needed, copy_plane, discontinuous, downmix, mono_needed,
    resampler_needed, reset_needed, scale_plane, sine_gains,
};
use proptest::prelude::*;

const MSEC: u64 = 1_000_000;
const THRESHOLD: u64 = SMOOTHING_THRESHOLD;

/// `enum audio_format` and `enum speaker_layout` (`media-io/audio-io.h`).
const FORMAT_FLOAT_PLANAR: i32 = 8;
const FORMAT_16BIT: i32 = 2;
const SPEAKERS_MONO: i32 = 1;
const SPEAKERS_5POINT1: i32 = 6;

fn format_of(format: i32, samples_per_sec: u32, speakers: i32) -> Format {
    Format {
        format,
        samples_per_sec,
        speakers,
    }
}

fn next_up(value: f32) -> f32 {
    f32::from_bits(value.to_bits() + 1)
}

fn next_down(value: f32) -> f32 {
    f32::from_bits(value.to_bits() - 1)
}

#[track_caller]
fn check_bits(actual: f32, expected: f32, what: &str, index: usize) {
    assert_eq!(
        actual.to_bits(),
        expected.to_bits(),
        "{what}[{index}] is {actual:e} ({:#010x}), expected {expected:e} ({:#010x})",
        actual.to_bits(),
        expected.to_bits()
    );
}

// ---- the discontinuity (#546) ----

#[test]
fn steps_discontinuity_at_the_threshold() {
    let next = 3_600_000 * MSEC;

    // later than expected
    assert!(!discontinuous(true, next, next));
    assert!(!discontinuous(true, next, next + THRESHOLD - 1));
    assert!(discontinuous(true, next, next + THRESHOLD));
    assert!(discontinuous(true, next, next + THRESHOLD + 1));

    // earlier than expected
    assert!(!discontinuous(true, next, next - THRESHOLD + 1));
    assert!(discontinuous(true, next, next - THRESHOLD));
    assert!(discontinuous(true, next, next - THRESHOLD - 1));

    // the resampler restart test's gaps: 20 ms continues, 100 ms and 1 s
    // restart
    assert!(!discontinuous(true, next, next + 20 * MSEC));
    assert!(discontinuous(true, next, next + 100 * MSEC));
    assert!(discontinuous(true, next, next + 1000 * MSEC));
}

#[test]
fn steps_discontinuity_needs_a_resampler_and_an_end() {
    let next = 3_600_000 * MSEC;

    // no resampler: nothing is held back
    assert!(!discontinuous(false, next, next + THRESHOLD));
    assert!(!discontinuous(false, next, 0));
    assert!(!discontinuous(false, next, u64::MAX));

    // next_resample_in_ts == 0: no packet with a rate yet
    assert!(!discontinuous(true, 0, 0));
    assert!(!discontinuous(true, 0, THRESHOLD));
    assert!(!discontinuous(true, 0, next));
    assert!(!discontinuous(true, 0, u64::MAX));
    assert!(!discontinuous(false, 0, u64::MAX));

    // next_resample_in_ts == 1 is set
    assert!(discontinuous(true, 1, 1 + THRESHOLD));
    assert!(!discontinuous(true, 1, 0));
}

/// Characterized, not endorsed: the distance is not modular.
#[test]
fn steps_discontinuity_distance_is_not_modular() {
    assert!(discontinuous(true, u64::MAX, 0));
    assert!(discontinuous(true, 1, u64::MAX));
    assert!(!discontinuous(true, u64::MAX, u64::MAX - THRESHOLD + 1));
    assert!(discontinuous(true, u64::MAX, u64::MAX - THRESHOLD));
    // at the low end the expected end is never 0
    assert!(!discontinuous(true, THRESHOLD - 1, 0));
    assert!(discontinuous(true, THRESHOLD, 0));
}

// ---- the reset condition ----

#[test]
fn steps_reset_on_each_trigger_alone() {
    let current = format_of(FORMAT_FLOAT_PLANAR, 44100, SPEAKERS_STEREO);

    // nothing changed
    assert!(!reset_needed(current, current, false, false, false));
    assert!(!reset_needed(current, current, true, true, false));

    // each trigger on its own
    assert!(reset_needed(
        current,
        format_of(FORMAT_FLOAT_PLANAR, 48000, SPEAKERS_STEREO),
        false,
        false,
        false
    ));
    assert!(reset_needed(
        current,
        format_of(FORMAT_16BIT, 44100, SPEAKERS_STEREO),
        false,
        false,
        false
    ));
    assert!(reset_needed(
        current,
        format_of(FORMAT_FLOAT_PLANAR, 44100, SPEAKERS_5POINT1),
        false,
        false,
        false
    ));
    // the clock drift compensation turned on, and off
    assert!(reset_needed(current, current, false, true, false));
    assert!(reset_needed(current, current, true, false, false));
    // the discontinuity (#546)
    assert!(reset_needed(current, current, false, false, true));
    assert!(reset_needed(current, current, true, true, true));

    // everything at once
    assert!(reset_needed(
        current,
        format_of(FORMAT_16BIT, 48000, SPEAKERS_MONO),
        true,
        false,
        true
    ));
}

#[test]
fn steps_reset_from_the_zeroed_source() {
    let zero = format_of(0, 0, 0);

    assert!(reset_needed(
        zero,
        format_of(FORMAT_FLOAT_PLANAR, 48000, SPEAKERS_STEREO),
        false,
        false,
        false
    ));
    assert!(reset_needed(zero, format_of(0, 1, 0), false, false, false));
    assert!(reset_needed(zero, format_of(1, 0, 0), false, false, false));
    assert!(reset_needed(zero, format_of(0, 0, 1), false, false, false));
    // characterized, not endorsed: a packet of all zeros does not
    assert!(!reset_needed(zero, zero, false, false, false));
    // values outside the enums are compared as they are
    assert!(reset_needed(zero, format_of(-1, 0, 0), false, false, false));
    assert!(reset_needed(
        zero,
        format_of(0, u32::MAX, 0),
        false,
        false,
        false
    ));
    assert!(reset_needed(zero, format_of(0, 0, 7), false, false, false));
}

// ---- the "no resampler needed" rule (the clock drift fix) ----

#[test]
fn steps_resampler_needed_rule() {
    let output = format_of(FORMAT_FLOAT_PLANAR, 48000, SPEAKERS_STEREO);

    assert!(!resampler_needed(output, output, false));
    assert!(resampler_needed(output, output, true));

    assert!(resampler_needed(
        format_of(FORMAT_FLOAT_PLANAR, 44100, SPEAKERS_STEREO),
        output,
        false
    ));
    assert!(resampler_needed(
        format_of(FORMAT_16BIT, 48000, SPEAKERS_STEREO),
        output,
        false
    ));
    assert!(resampler_needed(
        format_of(FORMAT_FLOAT_PLANAR, 48000, SPEAKERS_MONO),
        output,
        false
    ));
    assert!(resampler_needed(
        format_of(FORMAT_16BIT, 44100, SPEAKERS_MONO),
        output,
        true
    ));

    // characterized, not endorsed
    assert!(!resampler_needed(
        format_of(0, 0, 0),
        format_of(0, 0, 0),
        false
    ));
    assert!(resampler_needed(format_of(0, 0, 0), output, false));
}

// ---- the gates ----

#[test]
fn steps_balance_gate_at_the_edges() {
    assert!(!balance_needed(false, SPEAKERS_STEREO, 0.49));
    assert!(!balance_needed(false, SPEAKERS_STEREO, 0.50));
    assert!(!balance_needed(false, SPEAKERS_STEREO, 0.51));
    assert!(!balance_needed(false, SPEAKERS_STEREO, 0.495));
    assert!(!balance_needed(false, SPEAKERS_STEREO, next_up(0.49)));
    assert!(!balance_needed(false, SPEAKERS_STEREO, next_down(0.51)));

    assert!(balance_needed(false, SPEAKERS_STEREO, next_down(0.49)));
    assert!(balance_needed(false, SPEAKERS_STEREO, next_up(0.51)));
    assert!(balance_needed(false, SPEAKERS_STEREO, 0.25));
    assert!(balance_needed(false, SPEAKERS_STEREO, 0.75));

    assert!(balance_needed(false, SPEAKERS_STEREO, 0.0));
    assert!(balance_needed(false, SPEAKERS_STEREO, -0.0));
    assert!(balance_needed(false, SPEAKERS_STEREO, 1.0));
}

/// Characterized, not endorsed: a NaN is not balanced; values outside 0 to
/// 1 are.
#[test]
fn steps_balance_gate_outside_the_range() {
    assert!(!balance_needed(false, SPEAKERS_STEREO, f32::NAN));
    assert!(!balance_needed(false, SPEAKERS_STEREO, -f32::NAN));
    assert!(!balance_needed(
        false,
        SPEAKERS_STEREO,
        f32::from_bits(0x7f80_0001)
    ));
    assert!(balance_needed(false, SPEAKERS_STEREO, -1.0));
    assert!(balance_needed(false, SPEAKERS_STEREO, 2.0));
    assert!(balance_needed(false, SPEAKERS_STEREO, f32::INFINITY));
    assert!(balance_needed(false, SPEAKERS_STEREO, f32::NEG_INFINITY));
}

#[test]
fn steps_balance_gate_needs_stereo_into_more_than_mono() {
    assert!(!balance_needed(true, SPEAKERS_STEREO, 0.25));
    assert!(!balance_needed(true, SPEAKERS_STEREO, 0.0));
    assert!(!balance_needed(true, SPEAKERS_STEREO, 2.0));

    for speakers in [0, SPEAKERS_MONO, 3, 4, 5, SPEAKERS_5POINT1, 8, -1, 7] {
        assert!(!balance_needed(false, speakers, 0.25), "{speakers}");
        assert!(!balance_needed(false, speakers, 0.0), "{speakers}");
    }
}

#[test]
fn steps_mono_gate() {
    assert!(!mono_needed(false, 0));
    assert!(mono_needed(false, FORCE_MONO));
    assert!(mono_needed(false, u32::MAX));
    assert!(!mono_needed(false, 1));
    assert!(!mono_needed(false, !FORCE_MONO));
    assert!(mono_needed(false, FORCE_MONO | 1 | (1 << 31)));
    assert!(!mono_needed(true, FORCE_MONO));
    assert!(!mono_needed(true, u32::MAX));
    assert!(!mono_needed(true, 0));
}

// ---- the balance laws ----

const FRAMES: usize = 8;
const GUARD: usize = 2;
const GUARD_VALUE: f32 = 42.0;

#[allow(clippy::excessive_precision)]
const LEFT_IN: [f32; FRAMES] = [1.0, -1.0, 0.5, 0.0, -0.0, 0.123_456_789, 1e-40, 3.0e38];
#[allow(clippy::excessive_precision)]
const RIGHT_IN: [f32; FRAMES] = [0.25, -0.75, 1.0, -0.0, 0.0, -0.987_654_321, -1e-40, -3.0e38];

struct Planes {
    left: [f32; FRAMES + GUARD],
    right: [f32; FRAMES + GUARD],
    third: [f32; FRAMES + GUARD],
}

fn fill_planes() -> Planes {
    let mut p = Planes {
        left: [GUARD_VALUE; FRAMES + GUARD],
        right: [GUARD_VALUE; FRAMES + GUARD],
        third: [GUARD_VALUE; FRAMES + GUARD],
    };
    p.left[..FRAMES].copy_from_slice(&LEFT_IN);
    p.right[..FRAMES].copy_from_slice(&RIGHT_IN);
    p
}

/// The C's `obs_source_audio_input_balance(data, frames, ...)` over the
/// test's planes: the first `frames` of the left and right ones.
fn apply(p: &mut Planes, frames: usize, value: f32, law: i32) {
    if let Some(law) = Law::from_c(law) {
        balance(&mut p.left[..frames], &mut p.right[..frames], value, law);
    }
}

/// The two gains, with the C's mix of float and double, written out here.
fn sine_left_gain(value: f32) -> f32 {
    ((f64::from(1.0f32 - value) * (std::f64::consts::PI / 2.0)) as f32).sin()
}

fn sine_right_gain(value: f32) -> f32 {
    ((f64::from(value) * (std::f64::consts::PI / 2.0)) as f32).sin()
}

#[track_caller]
fn check_balanced(p: &Planes, left_gain: f32, right_gain: f32, frames: usize) {
    for i in 0..FRAMES + GUARD {
        let done = i < frames;
        let l = if i < FRAMES { LEFT_IN[i] } else { GUARD_VALUE };
        let r = if i < FRAMES { RIGHT_IN[i] } else { GUARD_VALUE };
        check_bits(p.left[i], if done { l * left_gain } else { l }, "left", i);
        check_bits(
            p.right[i],
            if done { r * right_gain } else { r },
            "right",
            i,
        );
        check_bits(p.third[i], GUARD_VALUE, "third", i);
    }
}

#[test]
fn steps_sine_law() {
    for value in [
        0.25,
        0.75,
        0.0,
        1.0,
        0.1,
        0.9,
        0.49,
        0.51,
        f32::from_bits(0x3a91_7d6b),
    ] {
        let mut p = fill_planes();
        apply(&mut p, FRAMES, value, SINE_LAW);
        check_balanced(&p, sine_left_gain(value), sine_right_gain(value), FRAMES);
        assert_eq!(
            sine_gains(value),
            (sine_left_gain(value), sine_right_gain(value))
        );
    }

    assert_eq!(sine_left_gain(0.0).to_bits(), 1.0f32.to_bits());
    assert_eq!(sine_right_gain(0.0).to_bits(), 0.0f32.to_bits());
    assert_eq!(sine_left_gain(1.0).to_bits(), 0.0f32.to_bits());
    assert_eq!(sine_right_gain(1.0).to_bits(), 1.0f32.to_bits());

    let mut p = fill_planes();
    apply(&mut p, 3, 0.25, SINE_LAW);
    check_balanced(&p, sine_left_gain(0.25), sine_right_gain(0.25), 3);
}

#[test]
fn steps_sine_law_keeps_the_double_product() {
    let value = f32::from_bits(0x3a91_7d6b);
    let through_double = (f64::from(1.0f32 - value) * (std::f64::consts::PI / 2.0)) as f32;
    let float_only = (1.0f32 - value) * ((std::f64::consts::PI / 2.0) as f32);

    assert_ne!(through_double.to_bits(), float_only.to_bits());

    let mut p = fill_planes();
    apply(&mut p, FRAMES, value, SINE_LAW);
    check_bits(p.left[0], LEFT_IN[0] * through_double.sin(), "left", 0);
}

#[test]
fn steps_square_law() {
    for value in [0.25f32, 0.75, 0.0, 1.0, 0.1] {
        let mut p = fill_planes();
        apply(&mut p, FRAMES, value, SQUARE_LAW);
        check_balanced(&p, (1.0 - value).sqrt(), value.sqrt(), FRAMES);
    }

    let mut p = fill_planes();
    apply(&mut p, FRAMES, 0.25, SQUARE_LAW);
    check_bits(p.left[0], f32::from_bits(0x3f5d_b3d7), "left", 0);
    check_bits(p.right[0], f32::from_bits(0x3e00_0000), "right", 0);
    check_bits(p.right[2], 0.5, "right", 2);
}

#[test]
fn steps_linear_law() {
    for value in [0.25f32, 0.75, 0.0, 1.0, 0.1] {
        let mut p = fill_planes();
        apply(&mut p, FRAMES, value, LINEAR);
        check_balanced(&p, 1.0 - value, value, FRAMES);
    }

    let mut p = fill_planes();
    apply(&mut p, FRAMES, 0.25, LINEAR);
    check_bits(p.left[0], 0.75, "left", 0);
    check_bits(p.left[1], -0.75, "left", 1);
    check_bits(p.left[2], 0.375, "left", 2);
    check_bits(p.right[0], 0.0625, "right", 0);
    check_bits(p.right[2], 0.25, "right", 2);
    check_bits(p.right[3], -0.0, "right", 3);
}

/// Characterized, not endorsed: out-of-range balances.
#[test]
fn steps_laws_outside_the_range() {
    for law in [SINE_LAW, SQUARE_LAW, LINEAR] {
        for value in [-1.0f32, 2.0, f32::INFINITY, f32::NEG_INFINITY] {
            let mut p = fill_planes();
            apply(&mut p, FRAMES, value, law);
            let (lg, rg) = match law {
                SINE_LAW => (sine_left_gain(value), sine_right_gain(value)),
                SQUARE_LAW => ((1.0 - value).sqrt(), value.sqrt()),
                _ => (1.0 - value, value),
            };
            for f in 0..FRAMES {
                let le = LEFT_IN[f] * lg;
                let re = RIGHT_IN[f] * rg;
                if le.is_nan() {
                    assert!(p.left[f].is_nan());
                } else {
                    check_bits(p.left[f], le, "left", f);
                }
                if re.is_nan() {
                    assert!(p.right[f].is_nan());
                } else {
                    check_bits(p.right[f], re, "right", f);
                }
            }
        }
    }

    let mut p = fill_planes();
    apply(&mut p, FRAMES, -1.0, LINEAR);
    check_bits(p.left[0], 2.0, "left", 0);
    check_bits(p.right[0], -0.25, "right", 0);
    let mut p = fill_planes();
    apply(&mut p, FRAMES, 2.0, SQUARE_LAW);
    assert!(p.left[0].is_nan());
    check_bits(p.right[0], 0.25 * 2.0f32.sqrt(), "right", 0);
}

#[test]
fn steps_unknown_law_changes_nothing() {
    for law in [3, -1, 100, i32::MIN, i32::MAX] {
        assert_eq!(Law::from_c(law), None, "{law}");
        let mut p = fill_planes();
        apply(&mut p, FRAMES, 0.25, law);
        check_balanced(&p, 1.0, 1.0, 0);
    }
}

#[test]
fn steps_balance_of_no_frames() {
    for law in [SINE_LAW, SQUARE_LAW, LINEAR, 3] {
        let mut p = fill_planes();
        apply(&mut p, 0, 0.25, law);
        check_balanced(&p, 1.0, 1.0, 0);
        if let Some(law) = Law::from_c(law) {
            balance(&mut [], &mut [], 0.25, law);
        }
    }
}

// ---- the downmix ----

const MAX_CHANNELS: usize = 8;

fn input() -> [[f32; FRAMES]; MAX_CHANNELS] {
    let mut input = [[0.0f32; FRAMES]; MAX_CHANNELS];
    for (c, plane) in input.iter_mut().enumerate() {
        for (f, sample) in plane.iter_mut().enumerate() {
            let magnitude = (c + 1) as f32 * 0.1 - f as f32 * 0.37;
            *sample = if c % 2 == 1 { -magnitude } else { magnitude };
        }
    }
    input
}

fn fill_mix() -> [[f32; FRAMES + GUARD]; MAX_CHANNELS] {
    let input = input();
    let mut m = [[GUARD_VALUE; FRAMES + GUARD]; MAX_CHANNELS];
    for c in 0..MAX_CHANNELS {
        m[c][..FRAMES].copy_from_slice(&input[c]);
    }
    m
}

/// The C's `obs_source_audio_input_downmix(data, channels, frames)` over
/// the test's planes.
fn mix(m: &mut [[f32; FRAMES + GUARD]; MAX_CHANNELS], channels: usize, frames: usize) {
    let count = channels.max(1);
    let mut planes: Vec<&mut [f32]> = m
        .iter_mut()
        .take(count)
        .map(|plane| &mut plane[..frames])
        .collect();
    downmix(&mut planes, channels);
}

fn mean_of(channels: usize, frame: usize) -> f32 {
    let input = input();
    let mut sum = input[0][frame];
    for plane in input.iter().take(channels).skip(1) {
        sum += plane[frame];
    }
    sum * (1.0f32 / channels as f32)
}

#[track_caller]
fn check_downmix(m: &[[f32; FRAMES + GUARD]; MAX_CHANNELS], channels: usize, frames: usize) {
    let input = input();
    for c in 0..MAX_CHANNELS {
        for f in 0..FRAMES + GUARD {
            let original = if f < FRAMES { input[c][f] } else { GUARD_VALUE };
            let done = c < channels && f < frames;
            check_bits(
                m[c][f],
                if done { mean_of(channels, f) } else { original },
                "plane",
                c * 100 + f,
            );
        }
    }
}

#[test]
fn steps_downmix_2_6_and_8_channels() {
    for channels in [2, 6, 8] {
        let mut m = fill_mix();
        mix(&mut m, channels, FRAMES);
        check_downmix(&m, channels, FRAMES);
    }

    let mut m = fill_mix();
    m[0][0] = 0.5;
    m[1][0] = 0.25;
    mix(&mut m, 2, 1);
    check_bits(m[0][0], 0.375, "plane", 0);
    check_bits(m[1][0], 0.375, "plane", 100);

    let mut m = fill_mix();
    mix(&mut m, 6, 3);
    check_downmix(&m, 6, 3);
}

#[test]
fn steps_downmix_sums_in_plane_order() {
    let mut m = fill_mix();
    for plane in m.iter_mut().take(6) {
        plane[0] = 0.0;
    }
    m[0][0] = 1.0;
    m[1][0] = 1e8;
    m[2][0] = -1e8;

    mix(&mut m, 6, 1);
    for (c, plane) in m.iter().enumerate().take(6) {
        check_bits(plane[0], 0.0, "plane", c * 100);
    }
}

#[test]
fn steps_downmix_scales_by_the_reciprocal() {
    let mut m = fill_mix();
    for plane in m.iter_mut().take(6) {
        plane[0] = 0.0;
    }
    m[0][0] = f32::from_bits(0x3e12_4925);

    mix(&mut m, 6, 1);
    assert_ne!((f32::from_bits(0x3e12_4925) / 6.0).to_bits(), 0x3cc3_0c32);
    for (c, plane) in m.iter().enumerate().take(6) {
        check_bits(plane[0], f32::from_bits(0x3cc3_0c32), "plane", c * 100);
    }
}

#[test]
fn steps_downmix_of_no_frames() {
    for channels in [0, 1, 2, 6, 8] {
        let mut m = fill_mix();
        mix(&mut m, channels, 0);
        check_downmix(&m, channels, 0);
        downmix(&mut [], channels);
    }
}

/// Characterized, not endorsed: one channel scales by 1, none by +inf.
#[test]
fn steps_downmix_of_one_and_no_channels() {
    let mut m = fill_mix();
    mix(&mut m, 1, FRAMES);
    check_downmix(&m, 1, FRAMES);

    let input = input();
    let mut m = fill_mix();
    m[0][0] = 1.0;
    m[0][1] = -2.0;
    m[0][2] = 0.0;
    mix(&mut m, 0, 3);
    check_bits(m[0][0], f32::INFINITY, "plane", 0);
    check_bits(m[0][1], f32::NEG_INFINITY, "plane", 1);
    assert!(m[0][2].is_nan());
    for f in 3..FRAMES {
        check_bits(m[0][f], input[0][f], "plane", f);
    }
    for c in 1..MAX_CHANNELS {
        for f in 0..FRAMES {
            check_bits(m[c][f], input[c][f], "plane", c * 100 + f);
        }
    }
}

// ---- edge cases ----

/// Any `f32` bit pattern, with the special values drawn often: ±0, ±inf,
/// NaNs, the gate's edges and their neighbours, and the slider's ends.
fn any_f32() -> impl Strategy<Value = f32> {
    prop_oneof![
        3 => any::<u32>().prop_map(f32::from_bits),
        1 => prop::sample::select(vec![
            0.0f32,
            -0.0,
            1.0,
            -1.0,
            0.5,
            0.49,
            0.51,
            next_down(0.49),
            next_up(0.49),
            next_down(0.51),
            next_up(0.51),
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NAN,
            -f32::NAN,
            f32::from_bits(0x7f80_0001),
            f32::from_bits(0xffff_ffff),
            f32::MIN_POSITIVE,
            f32::from_bits(1),
            f32::MAX,
            f32::MIN,
        ]),
        // in the slider's range, uniformly over the bits of [0, 1]
        2 => (0u32..=0x3f80_0000).prop_map(f32::from_bits),
    ]
}

/// A full-range `u64`, with values near 0, `u64::MAX` and the threshold.
fn any_u64() -> impl Strategy<Value = u64> {
    prop_oneof![
        3 => any::<u64>(),
        1 => prop::sample::select(vec![
            0u64,
            1,
            THRESHOLD - 1,
            THRESHOLD,
            THRESHOLD + 1,
            u64::MAX,
            u64::MAX - THRESHOLD,
            u64::MAX - THRESHOLD + 1,
            1 << 63,
        ]),
    ]
}

fn any_format() -> impl Strategy<Value = Format> {
    (
        prop_oneof![0i32..=10, any::<i32>()],
        prop_oneof![
            prop::sample::select(vec![0u32, 8000, 44100, 48000, u32::MAX]),
            any::<u32>()
        ],
        prop_oneof![0i32..=8, any::<i32>()],
    )
        .prop_map(|(format, samples_per_sec, speakers)| Format {
            format,
            samples_per_sec,
            speakers,
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4000))]

    /// The discontinuity against the C's rule written another way: within
    /// `THRESHOLD` of the end on either side continues.
    #[test]
    fn edge_discontinuity_model(
        has_resampler in any::<bool>(),
        next in any_u64(),
        delta in prop_oneof![any::<u64>(), 0u64..=2 * THRESHOLD],
        later in any::<bool>(),
    ) {
        let timestamp = if later { next.wrapping_add(delta) } else { next.wrapping_sub(delta) };
        let distance = next.abs_diff(timestamp);
        prop_assert_eq!(
            discontinuous(has_resampler, next, timestamp),
            has_resampler && next != 0 && distance >= THRESHOLD
        );
    }

    /// The reset condition is "anything differs, or a discontinuity".
    #[test]
    fn edge_reset_model(
        current in any_format(),
        packet in any_format(),
        same in any::<bool>(),
        last_compensation in any::<bool>(),
        compensation in any::<bool>(),
        discontinuity in any::<bool>(),
    ) {
        let packet = if same { current } else { packet };
        prop_assert_eq!(
            reset_needed(current, packet, last_compensation, compensation, discontinuity),
            current != packet || last_compensation != compensation || discontinuity
        );
    }

    /// A resampler unless the format is the output's and no compensation.
    #[test]
    fn edge_resampler_model(
        source in any_format(),
        output in any_format(),
        same in any::<bool>(),
        compensation in any::<bool>(),
    ) {
        let source = if same { output } else { source };
        prop_assert_eq!(
            resampler_needed(source, output, compensation),
            source != output || compensation
        );
    }

    /// The balance gate: balanced exactly when outside [0.49f, 0.51f] and
    /// not NaN, for a stereo source into an output that is not mono.
    #[test]
    fn edge_balance_gate_model(
        mono_output in any::<bool>(),
        speakers in prop_oneof![Just(SPEAKERS_STEREO), any::<i32>()],
        value in any_f32(),
    ) {
        let centred = (0.49f32..=0.51f32).contains(&value);
        prop_assert_eq!(
            balance_needed(mono_output, speakers, value),
            !mono_output && speakers == SPEAKERS_STEREO && !value.is_nan() && !centred
        );
    }

    /// The Force Mono gate: only its bit.
    #[test]
    fn edge_mono_gate_model(mono_output in any::<bool>(), flags in any::<u32>()) {
        prop_assert_eq!(mono_needed(mono_output, flags), !mono_output && flags & 2 != 0);
    }

    /// Each law against a per-sample model, over any samples, any balance
    /// and any frame count, with the left and right planes of different
    /// lengths.
    #[test]
    fn edge_laws_model(
        law in prop::sample::select(vec![SINE_LAW, SQUARE_LAW, LINEAR]),
        value in any_f32(),
        left in prop::collection::vec(any_f32(), 0..40),
        right in prop::collection::vec(any_f32(), 0..40),
    ) {
        let (lg, rg) = match law {
            SINE_LAW => (sine_left_gain(value), sine_right_gain(value)),
            SQUARE_LAW => ((1.0 - value).sqrt(), value.sqrt()),
            _ => (1.0 - value, value),
        };
        let mut l = left.clone();
        let mut r = right.clone();
        balance(&mut l, &mut r, value, Law::from_c(law).unwrap());
        for (out, sample) in l.iter().zip(&left) {
            let expected = sample * lg;
            prop_assert!(out.to_bits() == expected.to_bits() || (out.is_nan() && expected.is_nan()));
        }
        for (out, sample) in r.iter().zip(&right) {
            let expected = sample * rg;
            prop_assert!(out.to_bits() == expected.to_bits() || (out.is_nan() && expected.is_nan()));
        }
    }

    /// The downmix against a per-sample model over 1 to 8 channels and 0 to
    /// 40 frames of any samples.
    #[test]
    fn edge_downmix_model(
        channels in 1usize..=8,
        planes in prop::collection::vec(prop::collection::vec(any_f32(), 40), 8),
        frames in 0usize..=40,
    ) {
        let mut out: Vec<Vec<f32>> = planes.clone();
        {
            let mut slices: Vec<&mut [f32]> =
                out.iter_mut().take(channels).map(|p| &mut p[..frames]).collect();
            downmix(&mut slices, channels);
        }
        for f in 0..40 {
            let mut sum = planes[0][f];
            for plane in planes.iter().take(channels).skip(1) {
                sum += plane[f];
            }
            let mean = sum * (1.0 / channels as f32);
            for c in 0..8 {
                let expected = if c < channels && f < frames { mean } else { planes[c][f] };
                let actual = out[c][f];
                prop_assert!(
                    actual.to_bits() == expected.to_bits() || (actual.is_nan() && expected.is_nan()),
                    "channel {} frame {}: {:e} != {:e}", c, f, actual, expected
                );
            }
        }
    }

    /// The three downmix loops one at a time, over planes of different
    /// lengths: only the shorter length is touched.
    #[test]
    fn edge_downmix_steps(
        first in prop::collection::vec(any_f32(), 0..20),
        other in prop::collection::vec(any_f32(), 0..20),
        channels in prop_oneof![0usize..=9, any::<usize>()],
    ) {
        let n = first.len().min(other.len());

        let mut a = first.clone();
        add_plane(&mut a, &other);
        for (i, out) in a.iter().enumerate() {
            let expected = if i < n { first[i] + other[i] } else { first[i] };
            prop_assert!(out.to_bits() == expected.to_bits() || (out.is_nan() && expected.is_nan()));
        }

        let mut s = first.clone();
        scale_plane(&mut s, channels);
        let reciprocal = 1.0f32 / channels as f32;
        for (out, sample) in s.iter().zip(&first) {
            let expected = sample * reciprocal;
            prop_assert!(out.to_bits() == expected.to_bits() || (out.is_nan() && expected.is_nan()));
        }

        let mut c = other.clone();
        copy_plane(&first, &mut c);
        for (i, out) in c.iter().enumerate() {
            let expected = if i < n { first[i] } else { other[i] };
            prop_assert_eq!(out.to_bits(), expected.to_bits());
        }
    }
}

/// The laws known by value, and nothing else.
#[test]
fn edge_law_values() {
    assert_eq!(Law::from_c(SINE_LAW), Some(Law::Sine));
    assert_eq!(Law::from_c(SQUARE_LAW), Some(Law::Square));
    assert_eq!(Law::from_c(LINEAR), Some(Law::Linear));
    for law in [-1, 3, 4, i32::MIN, i32::MAX] {
        assert_eq!(Law::from_c(law), None);
    }
}

/// The gate's edges are the `f32` literals `0.49f` and `0.51f`.
#[test]
fn edge_gate_literals() {
    assert_eq!(0.49f32.to_bits(), 0x3efa_e148);
    assert_eq!(0.51f32.to_bits(), 0x3f02_8f5c);
    assert!(!balance_needed(
        false,
        SPEAKERS_STEREO,
        f32::from_bits(0x3efa_e148)
    ));
    assert!(!balance_needed(
        false,
        SPEAKERS_STEREO,
        f32::from_bits(0x3f02_8f5c)
    ));
    assert!(balance_needed(
        false,
        SPEAKERS_STEREO,
        f32::from_bits(0x3efa_e147)
    ));
    assert!(balance_needed(
        false,
        SPEAKERS_STEREO,
        f32::from_bits(0x3f02_8f5d)
    ));
}

/// Planes past `channels` are neither added nor written (found by the
/// mutation check: the shims never pass more, so only this reaches it).
#[test]
fn edge_downmix_leaves_planes_past_channels() {
    let mut m = fill_mix();
    let input = input();
    {
        let mut planes: Vec<&mut [f32]> = m.iter_mut().map(|p| &mut p[..FRAMES]).collect();
        downmix(&mut planes, 2);
    }
    check_downmix(&m, 2, FRAMES);
    for c in 2..MAX_CHANNELS {
        for f in 0..FRAMES {
            check_bits(m[c][f], input[c][f], "plane", c * 100 + f);
        }
    }
}
