//! Tier 1: `obs_source::volume_mul`, the volume multiply kernels of
//! `obs-source.c`.
//!
//! The `steps_*` cases port `test/cmocka/test_source_volume_mul_steps.c`
//! 1:1: its 6 cases under the same names, in the same order, with the same
//! buffers (8 consecutive channels of 1024 frames per mix, then the next
//! mix, a canary after every count), values and expectations, the
//! "characterized, not endorsed" ones included. The C calls the hidden
//! `obs_source_volume_mul_*` functions; here the safe core is called on
//! slices of the same buffer.
//!
//! The `edge_*` cases are new: `mul_data` with slices of different lengths
//! (the shorter one counts), and proptests of both kernels against one
//! multiply per sample, and of any input without a panic.
//!
//! A NaN sample times a NaN volume carries either payload (the C does not
//! pin it either), so [`same`] takes any NaN there.

#![allow(clippy::as_conversions, clippy::cast_precision_loss)]

use obs_source::volume_mul::{mul_block, mul_data, mul_data_grouped, mul_data_zipped};
use proptest::prelude::*;

/// `AUDIO_OUTPUT_FRAMES` and `MAX_AUDIO_CHANNELS`.
const FRAMES: usize = 1024;
const CHANNELS: usize = 8;
const MIX: usize = FRAMES * CHANNELS;

/// A value no multiply below gives.
const CANARY: f32 = -123.25;

/// A sample of channel `ch`, frame `i`: distinct, signed, some above 1.
fn sample(ch: usize, i: usize) -> f32 {
    ((i % 97) as f32 - 48.0) / 16.0 + ch as f32 * 0.125
}

/// Two mixes, the first filled as `fill()` in the C test, then the canary.
fn fill() -> Vec<f32> {
    let mut buf = vec![CANARY; 2 * MIX];
    for ch in 0..CHANNELS {
        for i in 0..FRAMES {
            buf[ch * FRAMES + i] = sample(ch, i);
        }
    }
    buf
}

/// The product as the C test's `expected[i] *= vol` gives it, or, for a
/// NaN times a NaN, any NaN.
fn same(got: f32, want: f32, a: f32, b: f32) -> bool {
    if a.is_nan() && b.is_nan() {
        got.is_nan()
    } else {
        got.to_bits() == want.to_bits()
    }
}

fn check(what: &str, got: &[f32], want: &[f32]) {
    for (i, (g, w)) in got.iter().zip(want).enumerate() {
        assert_eq!(
            g.to_bits(),
            w.to_bits(),
            "{what}: sample {i} is {g} ({:08x}), expected {w} ({:08x})",
            g.to_bits(),
            w.to_bits()
        );
    }
}

const SPECIALS: [f32; 9] = [
    0.0,
    -0.0,
    1.0,
    -1.0,
    f32::INFINITY,
    f32::NEG_INFINITY,
    f32::NAN,
    1e-45,
    3.4e38,
];

#[test]
fn steps_block_channels() {
    for channels in [1, 2, 8] {
        for vol in [0.0_f32, 1.0, 0.5, 20.0, 0.316_227_77] {
            let mut buf = fill();
            let mut expected = buf.clone();
            for e in &mut expected[..FRAMES * channels] {
                *e *= vol;
            }
            mul_block(&mut buf[..FRAMES * channels], vol);
            check("block", &buf, &expected);
        }
    }
}

#[test]
fn steps_block_counts() {
    for count in [0, 1, 3, 4, 5, 7, 8, 15, 16, 17, 1023, 1025] {
        let mut buf = fill();
        let mut expected = buf.clone();
        for e in &mut expected[..count] {
            *e *= 0.5;
        }
        mul_block(&mut buf[..count], 0.5);
        check("block count", &buf, &expected);
    }
}

#[test]
fn steps_block_special() {
    for vol in SPECIALS {
        let mut buf = fill();
        buf[..SPECIALS.len()].copy_from_slice(&SPECIALS);
        let before = buf.clone();
        mul_block(&mut buf[..FRAMES], vol);
        for i in 0..2 * MIX {
            let want = if i < FRAMES {
                before[i] * vol
            } else {
                before[i]
            };
            let a = before[i];
            let b = if i < FRAMES { vol } else { 0.0 };
            assert!(same(buf[i], want, a, b), "block special {vol}: sample {i}");
        }
    }

    // 0 * inf is NaN, a NaN sample keeps its payload.
    let mut buf = fill();
    buf[0] = f32::INFINITY;
    buf[1] = f32::from_bits(0x7fc0_0123);
    mul_block(&mut buf[..2], 0.0);
    assert!(buf[0].is_nan());
    assert_eq!(buf[1].to_bits(), 0x7fc0_0123);
}

#[test]
fn steps_data_ramp() {
    let vol: Vec<f32> = (0..FRAMES)
        .map(|i| i as f32 / (FRAMES - 1) as f32)
        .collect();
    for channels in [1, 2, 8] {
        let mut buf = fill();
        let mut expected = buf.clone();
        for ch in 0..channels {
            for i in 0..FRAMES {
                expected[ch * FRAMES + i] *= vol[i];
            }
            mul_data(&mut buf[ch * FRAMES..(ch + 1) * FRAMES], &vol);
        }
        check("data ramp", &buf, &expected);
    }
}

#[test]
fn steps_data_step() {
    let vol: Vec<f32> = (0..FRAMES)
        .map(|i| if i < 300 { 0.75 } else { 0.0 })
        .collect();
    let mut buf = fill();
    let mut expected = buf.clone();
    for i in 0..FRAMES {
        expected[i] *= vol[i];
    }
    mul_data(&mut buf[..FRAMES], &vol);
    check("data step", &buf, &expected);
    // A negative sample after the mute is -0.0: sample(0, 300) is -2.4375.
    assert_eq!(buf[300].to_bits(), 0x8000_0000);
}

#[test]
fn steps_data_counts_and_special() {
    for (c, count) in [0, 1, 3, 4, 5, 8, 17, 1023].into_iter().enumerate() {
        let vol: Vec<f32> = (0..FRAMES)
            .map(|i| SPECIALS[(i * 7 + c) % SPECIALS.len()])
            .collect();
        let mut buf = fill();
        let mut expected = buf.clone();
        for i in 0..count {
            expected[i] *= vol[i];
        }
        mul_data(&mut buf[..count], &vol[..count]);
        check("data count", &buf, &expected);
    }
}

#[test]
fn edge_data_shorter_slice_counts() {
    let mut out = [2.0_f32, 3.0, 4.0, 5.0];
    mul_data(&mut out, &[0.5, 0.25]);
    assert_eq!(out, [1.0, 0.75, 4.0, 5.0]);

    let mut out = [2.0_f32, 3.0];
    mul_data(&mut out, &[0.5, 0.25, 9.0, 9.0]);
    assert_eq!(out, [1.0, 0.75]);

    let mut out: [f32; 0] = [];
    mul_data(&mut out, &[1.0]);
    mul_block(&mut out, f32::NAN);
}

fn any_f32() -> impl Strategy<Value = f32> {
    prop_oneof![
        prop::sample::select(SPECIALS.to_vec()),
        any::<u32>().prop_map(f32::from_bits),
        -100.0_f32..100.0,
    ]
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(1024),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    #[test]
    fn edge_block_is_one_multiply_per_sample(
        samples in prop::collection::vec(any_f32(), 0..40),
        vol in any_f32(),
    ) {
        let mut out = samples.clone();
        mul_block(&mut out, vol);
        for (i, (&got, &x)) in out.iter().zip(&samples).enumerate() {
            prop_assert!(same(got, x * vol, x, vol), "sample {i}: {x} * {vol} gave {got}");
        }
    }

    /// Both loop forms of `mul_data` (4-sample groups on x86_64, a zip loop
    /// elsewhere) agree on every host, any NaN matching any NaN, the
    /// shortest slice deciding, with any length (so every group remainder).
    #[test]
    fn edge_both_data_forms_agree(
        samples in prop::collection::vec(any_f32(), 0..40),
        vol in prop::collection::vec(any_f32(), 0..40),
    ) {
        let (mut grouped, mut zipped, mut chosen) = (samples.clone(), samples.clone(), samples.clone());
        mul_data_grouped(&mut grouped, &vol);
        mul_data_zipped(&mut zipped, &vol);
        mul_data(&mut chosen, &vol);
        for (i, ((&g, &z), &c)) in grouped.iter().zip(&zipped).zip(&chosen).enumerate() {
            prop_assert!(
                (g.is_nan() && z.is_nan() && c.is_nan()) || (g.to_bits() == z.to_bits() && c.to_bits() == z.to_bits()),
                "sample {i}: grouped {g}, zipped {z}, chosen {c}"
            );
        }
    }

    #[test]
    fn edge_data_is_one_multiply_per_sample(
        pairs in prop::collection::vec((any_f32(), any_f32()), 0..40),
    ) {
        let samples: Vec<f32> = pairs.iter().map(|p| p.0).collect();
        let vol: Vec<f32> = pairs.iter().map(|p| p.1).collect();
        let mut out = samples.clone();
        mul_data(&mut out, &vol);
        for (i, ((&got, &x), &v)) in out.iter().zip(&samples).zip(&vol).enumerate() {
            prop_assert!(same(got, x * v, x, v), "sample {i}: {x} * {v} gave {got}");
        }
    }

    #[test]
    fn edge_no_panic(
        out in prop::collection::vec(any::<u32>(), 0..40),
        vol in prop::collection::vec(any::<u32>(), 0..40),
        v: u32,
    ) {
        let mut out: Vec<f32> = out.into_iter().map(f32::from_bits).collect();
        let vol: Vec<f32> = vol.into_iter().map(f32::from_bits).collect();
        mul_block(&mut out, f32::from_bits(v));
        mul_data(&mut out, &vol);
    }
}
