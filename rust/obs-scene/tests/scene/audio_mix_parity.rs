//! Tier 3 parity: the shims of `obs_scene::ffi::scene_audio_mix` (and the
//! safe core under them) against the original
//! `libobs/obs-scene-audio-mix.c.h`, compiled as
//! `oracle_obs_scene_audio_mix_*` (obs-c-oracle, oracle/scene_audio_mix.c,
//! built without floating-point contraction like every oracle).
//!
//! Every call runs the shim, the C and the core on copies of the same
//! output buffer and compares every sample of the whole buffer bit for bit,
//! so a write outside the window shows up too. A NaN matches any NaN: where
//! two NaNs meet (a NaN sample and a NaN product, or the NaN of `inf * 0`),
//! IEEE 754 leaves the payload open, the add commutes, and each compiler
//! orders the operands as it likes; x86 then returns the first operand's
//! NaN, so the C and the Rust can carry different NaNs there.
//!
//! Runs: every special pairing (0, -0, 1, -1, 0.5, 2, the smallest normal and
//! a subnormal, the largest float, infinities and two NaN payloads) in the
//! output, the input and the gain; whole ticks and windows at every 64th
//! position with every count from 0 to 70 and the tick's last frames, on
//! inputs with random bit patterns; and proptests over any window of a
//! tick with any `f32`.
//!
//! No intentional differences.
//!
//! `test_scene_audio_mix_steps` runs the C test against the C;
//! `test_scene_audio_actions` runs the real libobs scene mix against the
//! hybrid libobs, where these shims replace the C.

#![allow(clippy::as_conversions, clippy::cast_possible_truncation)]

use obs_c_oracle::scene_audio_mix::{
    oracle_obs_scene_audio_mix_add, oracle_obs_scene_audio_mix_with_buf,
};
use obs_scene::audio_mix as core;
use obs_scene::ffi::scene_audio_mix::{obs_scene_audio_mix_add, obs_scene_audio_mix_with_buf};
use proptest::prelude::*;

const FRAMES: usize = 1024;

const SPECIALS: [f32; 14] = [
    0.0,
    -0.0,
    1.0,
    -1.0,
    0.5,
    2.0,
    f32::MIN_POSITIVE,
    1.0e-40,
    f32::MAX,
    -f32::MAX,
    f32::INFINITY,
    f32::NEG_INFINITY,
    f32::NAN,
    f32::from_bits(0xFFC0_1234),
];

/// The bits of each sample, every NaN as one value.
fn bits(v: &[f32]) -> Vec<u32> {
    v.iter()
        .map(|f| {
            if f.is_nan() {
                f32::NAN.to_bits()
            } else {
                f.to_bits()
            }
        })
        .collect()
}

/// Runs both kernels three ways on `out` and compares the whole buffers.
fn check(out: &[f32], input: &[f32], gain: &[f32], pos: usize, count: usize) {
    assert!(pos + count <= out.len() && count <= input.len() && count <= gain.len());

    let (mut shim, mut c, mut safe) = (out.to_vec(), out.to_vec(), out.to_vec());
    // SAFETY: the window and the inputs are inside their buffers, which do
    // not overlap.
    unsafe {
        obs_scene_audio_mix_with_buf(shim.as_mut_ptr(), input.as_ptr(), gain.as_ptr(), pos, count);
        oracle_obs_scene_audio_mix_with_buf(
            c.as_mut_ptr(),
            input.as_ptr(),
            gain.as_ptr(),
            pos,
            count,
        );
    }
    core::mix_with_buf(&mut safe[pos..pos + count], &input[..count], &gain[..count]);
    assert_eq!(
        bits(&shim),
        bits(&c),
        "with_buf shim, pos {pos} count {count}"
    );
    assert_eq!(
        bits(&safe),
        bits(&c),
        "with_buf core, pos {pos} count {count}"
    );

    let (mut shim, mut c, mut safe) = (out.to_vec(), out.to_vec(), out.to_vec());
    // SAFETY: as above.
    unsafe {
        obs_scene_audio_mix_add(shim.as_mut_ptr(), input.as_ptr(), pos, count);
        oracle_obs_scene_audio_mix_add(c.as_mut_ptr(), input.as_ptr(), pos, count);
    }
    core::mix_add(&mut safe[pos..pos + count], &input[..count]);
    assert_eq!(bits(&shim), bits(&c), "add shim, pos {pos} count {count}");
    assert_eq!(bits(&safe), bits(&c), "add core, pos {pos} count {count}");
}

/// splitmix64 bit patterns as floats: every class of `f32` occurs.
fn random_floats(n: usize, seed: u64) -> Vec<f32> {
    let mut state = seed;
    (0..n)
        .map(|_| {
            state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = state;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            f32::from_bits((z ^ (z >> 31)) as u32)
        })
        .collect()
}

#[test]
fn every_special_triple_matches_c() {
    for &o in &SPECIALS {
        for &i in &SPECIALS {
            for &g in &SPECIALS {
                check(&[o], &[i], &[g], 0, 1);
            }
        }
    }
}

#[test]
fn windows_match_c() {
    let out = random_floats(FRAMES, 1);
    let input = random_floats(FRAMES, 2);
    let gain = random_floats(FRAMES, 3);
    let mut windows = 0;
    for pos in (0..=FRAMES).step_by(64) {
        for count in (0..=70).chain([FRAMES - pos]) {
            if pos + count <= FRAMES {
                check(&out, &input, &gain, pos, count);
                windows += 1;
            }
        }
    }
    for pos in FRAMES - 8..=FRAMES {
        check(&out, &input, &gain, pos, FRAMES - pos);
        windows += 1;
    }
    assert!(windows > 1000);
}

#[test]
fn whole_ticks_of_audio_match_c() {
    // Samples in [-1, 1) and a 0/1 gain, as the scene mixes them.
    let wave = |seed: u64| -> Vec<f32> {
        random_floats(FRAMES, seed)
            .into_iter()
            .map(|f| (f.to_bits() >> 8) as f32 / 8_388_608.0 - 1.0)
            .collect()
    };
    let gain: Vec<f32> = (0..FRAMES)
        .map(|i| if i < 333 { 1.0 } else { 0.0 })
        .collect();
    for seed in 0..32 {
        check(&wave(seed), &wave(seed + 100), &gain, 0, FRAMES);
        check(&wave(seed), &wave(seed + 200), &wave(seed + 300), 0, FRAMES);
    }
}

fn sample() -> impl Strategy<Value = f32> {
    prop_oneof![
        any::<u32>().prop_map(f32::from_bits),
        prop::sample::select(SPECIALS.to_vec()),
        -1.0_f32..1.0,
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(256)))]

    #[test]
    fn any_window_matches_c(
        (out, input, gain, pos) in (0..=FRAMES).prop_flat_map(|count| (
            prop::collection::vec(sample(), FRAMES),
            prop::collection::vec(sample(), count),
            prop::collection::vec(sample(), count),
            0..=FRAMES - count,
        )),
    ) {
        let count = input.len();
        check(&out, &input, &gain, pos, count);
    }
}
