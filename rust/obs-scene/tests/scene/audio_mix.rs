//! Tier 1: `obs_scene::audio_mix`, the two mix kernels of
//! `scene_audio_render()` in `obs-scene.c`.
//!
//! The `steps_*` cases port `test/cmocka/test_scene_audio_mix_steps.c` 1:1:
//! its 6 cases under the same names, in the same order, with the same
//! values and expectations. The C calls the hidden `obs_scene_audio_mix_*`
//! functions with `pos` and `count`; here the safe core gets the slices the
//! shim cuts, `out[pos .. pos + count]` and `input[..count]`.
//!
//! The `edge_*` cases are new: proptests of both kernels against the C's
//! expression, sample by sample and bit for bit (any NaN matching any NaN),
//! on random windows of a tick with any `f32`, and slices of different
//! lengths (the shortest decides).

#![allow(clippy::as_conversions, clippy::cast_possible_truncation)]

use obs_scene::audio_mix::{mix_add, mix_add_grouped, mix_add_zipped, mix_with_buf};
use proptest::prelude::*;

const FRAMES: usize = 1024;
const GUARD: usize = 16;
const LEN: usize = GUARD + FRAMES + GUARD;

fn check_floats(what: &str, got: &[f32], want: &[f32]) {
    for (i, (g, w)) in got.iter().zip(want).enumerate() {
        assert_eq!(
            g.to_bits(),
            w.to_bits(),
            "{what}: sample {i} is {g:e}, expected {w:e}"
        );
    }
    assert_eq!(got.len(), want.len());
}

/// A value per position that is exact in float and differs everywhere.
fn base(i: usize) -> f32 {
    ((i % 97) as i32 - 48) as f32 / 64.0
}

fn in_value(i: usize) -> f32 {
    ((i % 31) as i32 - 15) as f32 / 8.0
}

fn filled(f: fn(usize) -> f32, n: usize) -> Vec<f32> {
    (0..n).map(f).collect()
}

/// The C call: the window of `out` from `pos`, `count` samples of the
/// input and the gain.
fn call_add(out: &mut [f32], input: &[f32], pos: usize, count: usize) {
    mix_add(&mut out[pos..pos + count], &input[..count]);
}

fn call_with_buf(out: &mut [f32], input: &[f32], gain: &[f32], pos: usize, count: usize) {
    mix_with_buf(&mut out[pos..pos + count], &input[..count], &gain[..count]);
}

#[test]
fn steps_add_windows() {
    const CASES: [(usize, usize); 9] = [
        (0, 0),
        (0, 1),
        (0, 3),
        (0, FRAMES),
        (300, 0),
        (300, 1),
        (300, 724),
        (FRAMES - 1, 1),
        (FRAMES, 0),
    ];
    let input = filled(in_value, FRAMES);

    for (pos, count) in CASES {
        let mut out = filled(base, LEN);
        let mut want = filled(base, LEN);
        for i in 0..count {
            want[GUARD + pos + i] += input[i];
        }

        call_add(&mut out[GUARD..], &input, pos, count);
        check_floats("add", &out, &want);
    }
}

#[test]
fn steps_with_buf_gains() {
    const GAINS: [f32; 5] = [1.0, 0.0, 0.5, 2.0, -1.0];
    let input = filled(in_value, FRAMES);

    for gain in GAINS {
        for pos in [0, 300] {
            let count = FRAMES - pos;
            let buf: Vec<f32> = (0..FRAMES)
                .map(|i| if i % 2 == 1 { gain } else { 1.0 })
                .collect();
            let mut out = filled(base, LEN);
            let mut want = filled(base, LEN);
            for i in 0..count {
                want[GUARD + pos + i] += input[i] * buf[i];
            }

            call_with_buf(&mut out[GUARD..], &input, &buf, pos, count);
            check_floats("with_buf", &out, &want);
        }
    }
}

#[test]
fn steps_with_buf_step() {
    let input = filled(in_value, FRAMES);
    let buf: Vec<f32> = (0..FRAMES)
        .map(|i| if i < 512 { 1.0 } else { 0.0 })
        .collect();

    for pos in [0, 301] {
        let count = FRAMES - pos;
        let mut out = filled(base, LEN);
        let mut want = filled(base, LEN);
        for i in 0..count {
            want[GUARD + pos + i] = base(GUARD + pos + i) + if i < 512 { input[i] } else { 0.0 };
        }

        call_with_buf(&mut out[GUARD..], &input, &buf, pos, count);
        check_floats("step", &out, &want);
    }
}

#[test]
fn steps_specials() {
    const SPECIALS: [f32; 7] = [
        0.0,
        -0.0,
        1.0,
        -1.0,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NAN,
    ];

    for a in SPECIALS {
        for b in SPECIALS {
            for c in SPECIALS {
                let mut out = [a];
                let want = a + b * c;
                mix_with_buf(&mut out, &[b], &[c]);
                if want.is_nan() {
                    assert!(
                        out[0].is_nan(),
                        "with_buf {a:e} + {b:e} * {c:e} = {:e}",
                        out[0]
                    );
                } else {
                    assert_eq!(
                        out[0].to_bits(),
                        want.to_bits(),
                        "with_buf {a:e} + {b:e} * {c:e}"
                    );
                }
            }
            let mut out = [a];
            let want = a + b;
            mix_add(&mut out, &[b]);
            if want.is_nan() {
                assert!(out[0].is_nan(), "add {a:e} + {b:e} = {:e}", out[0]);
            } else {
                assert_eq!(out[0].to_bits(), want.to_bits(), "add {a:e} + {b:e}");
            }
        }
    }
}

#[test]
fn steps_count_zero() {
    let input = filled(in_value, FRAMES);
    let buf = filled(in_value, FRAMES);
    let mut out = filled(base, LEN);
    let want = filled(base, LEN);

    call_add(&mut out[GUARD..], &input, 0, 0);
    call_with_buf(&mut out[GUARD..], &input, &buf, 500, 0);
    call_with_buf(&mut out[GUARD..], &input, &buf, FRAMES, 0);
    check_floats("count 0", &out, &want);
}

#[test]
fn steps_twice() {
    let input = filled(in_value, FRAMES);
    let mut out = filled(base, LEN);
    let mut want = filled(base, LEN);
    for i in 0..FRAMES {
        want[GUARD + i] = (want[GUARD + i] + input[i]) + input[i];
    }

    call_add(&mut out[GUARD..], &input, 0, FRAMES);
    call_add(&mut out[GUARD..], &input, 0, FRAMES);
    check_floats("twice", &out, &want);
}

// ---------------------------------------------------------------------------
// edge cases

/// Any `f32`, NaN payloads and both zeros included.
fn sample() -> impl Strategy<Value = f32> {
    prop_oneof![
        any::<u32>().prop_map(f32::from_bits),
        prop::sample::select(vec![
            0.0,
            -0.0,
            1.0,
            -1.0,
            0.5,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NAN,
            f32::MIN_POSITIVE,
            f32::MAX,
        ]),
        -2.0_f32..2.0,
    ]
}

/// The bits of a sample, every NaN as one value: where two NaNs meet, the
/// payload is the compiler's choice of operand order (see the parity test).
fn nan_as_one(f: f32) -> u32 {
    if f.is_nan() {
        f32::NAN.to_bits()
    } else {
        f.to_bits()
    }
}

/// A window of a tick: `out` of a whole tick, the input and gain of `count`.
fn window() -> impl Strategy<Value = (Vec<f32>, Vec<f32>, Vec<f32>, usize)> {
    (0..=FRAMES).prop_flat_map(|count| {
        (
            prop::collection::vec(sample(), FRAMES),
            prop::collection::vec(sample(), count),
            prop::collection::vec(sample(), count),
            0..=FRAMES - count,
        )
    })
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(128),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    #[test]
    fn edge_with_buf_is_the_c_expression((out, input, gain, pos) in window()) {
        let mut got = out.clone();
        let count = input.len();
        call_with_buf(&mut got, &input, &gain, pos, count);
        for (i, (g, o)) in got.iter().zip(&out).enumerate() {
            let want = if (pos..pos + count).contains(&i) {
                o + input[i - pos] * gain[i - pos]
            } else {
                *o
            };
            prop_assert_eq!(nan_as_one(*g), nan_as_one(want), "sample {}", i);
        }
    }

    #[test]
    fn edge_add_is_the_c_expression((out, input, _gain, pos) in window()) {
        let mut got = out.clone();
        let count = input.len();
        call_add(&mut got, &input, pos, count);
        for (i, (g, o)) in got.iter().zip(&out).enumerate() {
            let want = if (pos..pos + count).contains(&i) { o + input[i - pos] } else { *o };
            prop_assert_eq!(nan_as_one(*g), nan_as_one(want), "sample {}", i);
        }
    }

    /// Both loop forms of `mix_add` (4-sample groups on x86_64, a zip loop
    /// elsewhere) give the same bits on every host, the shortest slice
    /// deciding, with any length (so every group remainder).
    #[test]
    fn edge_both_add_forms_agree(
        out in prop::collection::vec(sample(), 0..40),
        input in prop::collection::vec(sample(), 0..40),
    ) {
        let (mut grouped, mut zipped, mut chosen) = (out.clone(), out.clone(), out.clone());
        mix_add_grouped(&mut grouped, &input);
        mix_add_zipped(&mut zipped, &input);
        mix_add(&mut chosen, &input);
        let grouped: Vec<u32> = grouped.iter().map(|f| nan_as_one(*f)).collect();
        let zipped: Vec<u32> = zipped.iter().map(|f| nan_as_one(*f)).collect();
        let chosen: Vec<u32> = chosen.iter().map(|f| nan_as_one(*f)).collect();
        prop_assert_eq!(&grouped, &zipped);
        prop_assert_eq!(&chosen, &zipped);
    }

    #[test]
    fn edge_shortest_slice_decides(
        out_len in 0_usize..40,
        in_len in 0_usize..40,
        gain_len in 0_usize..40,
    ) {
        let mut out = vec![1.0_f32; out_len];
        mix_with_buf(&mut out, &vec![2.0; in_len], &vec![3.0; gain_len]);
        let n = out_len.min(in_len).min(gain_len);
        for (i, v) in out.iter().enumerate() {
            prop_assert_eq!(*v, if i < n { 7.0 } else { 1.0 });
        }
        let mut out = vec![1.0_f32; out_len];
        mix_add(&mut out, &vec![2.0; in_len]);
        let n = out_len.min(in_len);
        for (i, v) in out.iter().enumerate() {
            prop_assert_eq!(*v, if i < n { 3.0 } else { 1.0 });
        }
    }
}
