//! Tier 3 parity: the shims of `obs_output::ffi::encoder_lifecycle` (and
//! the core behind them) against the oracle, the unchanged
//! `libobs/obs-encoder-lifecycle.c.h` compiled with its functions renamed
//! (`obs_c_oracle::encoder_lifecycle`). The type tests are `encoder_bind`'s
//! and `encoder_audio_start`'s, whose oracles are
//! `oracle_obs_encoder_bind_not_audio` and
//! `oracle_obs_encoder_audio_start_not_video`.
//!
//! Every comparison is exact, on all three sides: the constants; the counts
//! and indexes over 0 to 1,000,000, the top 1,000,000 of `usize` and a
//! stride through all of it; the group counts over 0 to 1,000,000, the top
//! 1,000,000 of `u32` and a stride through all of it, and ready over every
//! pair below 1,024 and of 24 special values on each side; every cap word of
//! the low 20 bits under four high words; the SEI table over the same sizes;
//! the type tests over -100,000 to 100,000, 100,000 values at each extreme
//! and a stride through all of `i32`; the callers on the stand-in encoder of
//! `test_encoder_lifecycle_steps` (from Tier 1) with the oracle's gates and
//! with the shims, through a fixed run of every caller for every type, cap,
//! mix, request, SEI answer, group and output list; and proptests of every
//! decision over full-range values and of the stand-in over call
//! sequences. No input is excluded: nothing in the C divides or overflows
//! (the steps wrap as `uint32_t` does).

use std::ffi::c_int;

use obs_c_oracle::encoder_audio_start::oracle_obs_encoder_audio_start_not_video;
use obs_c_oracle::encoder_bind::oracle_obs_encoder_bind_not_audio;
use obs_c_oracle::encoder_lifecycle::{
    CONSTANT_NAMES, oracle_encoder_lifecycle_constant, oracle_obs_encoder_lifecycle_first,
    oracle_obs_encoder_lifecycle_found, oracle_obs_encoder_lifecycle_group_idle,
    oracle_obs_encoder_lifecycle_group_ready, oracle_obs_encoder_lifecycle_last,
    oracle_obs_encoder_lifecycle_lcm_unset, oracle_obs_encoder_lifecycle_pass_texture,
    oracle_obs_encoder_lifecycle_prepend_sei, oracle_obs_encoder_lifecycle_started_dec,
    oracle_obs_encoder_lifecycle_started_inc,
};
use obs_output::encoder_lifecycle::{self as core, NOT_FOUND, OBS_ENCODER_CAP_PASS_TEXTURE};
use obs_output::ffi::encoder_audio_start::obs_encoder_audio_start_not_video;
use obs_output::ffi::encoder_bind::obs_encoder_bind_not_audio;
use obs_output::ffi::encoder_lifecycle::{
    obs_encoder_lifecycle_first, obs_encoder_lifecycle_found, obs_encoder_lifecycle_group_idle,
    obs_encoder_lifecycle_group_ready, obs_encoder_lifecycle_last, obs_encoder_lifecycle_lcm_unset,
    obs_encoder_lifecycle_pass_texture, obs_encoder_lifecycle_prepend_sei,
    obs_encoder_lifecycle_started_dec, obs_encoder_lifecycle_started_inc,
};
use proptest::prelude::*;

use super::encoder_lifecycle::{
    CAP_PASS_TEXTURE, Gates, Group, Output, SEIS, Stand, TYPE_AUDIO, TYPE_VIDEO, apply, call,
    fixed_run, stand,
};

/// The oracle's functions as the stand-in's gates.
const C: Gates = Gates {
    // SAFETY (each): the oracle takes and returns plain values.
    first: |n| unsafe { oracle_obs_encoder_lifecycle_first(n) },
    found: |i| unsafe { oracle_obs_encoder_lifecycle_found(i) },
    last: |n| unsafe { oracle_obs_encoder_lifecycle_last(n) },
    not_audio: |t| unsafe { oracle_obs_encoder_bind_not_audio(t) },
    not_video: |t| unsafe { oracle_obs_encoder_audio_start_not_video(t) },
    pass_texture: |c| unsafe { oracle_obs_encoder_lifecycle_pass_texture(c) },
    lcm_unset: |l| unsafe { oracle_obs_encoder_lifecycle_lcm_unset(l) },
    started_inc: |s| unsafe { oracle_obs_encoder_lifecycle_started_inc(s) },
    group_ready: |s, n| unsafe { oracle_obs_encoder_lifecycle_group_ready(s, n) },
    started_dec: |s| unsafe { oracle_obs_encoder_lifecycle_started_dec(s) },
    group_idle: |s| unsafe { oracle_obs_encoder_lifecycle_group_idle(s) },
    prepend_sei: |g, h, s| unsafe { oracle_obs_encoder_lifecycle_prepend_sei(g, h, s) },
};

/// The shims as the stand-in's gates.
const SHIM: Gates = Gates {
    first: |n| obs_encoder_lifecycle_first(n),
    found: |i| obs_encoder_lifecycle_found(i),
    last: |n| obs_encoder_lifecycle_last(n),
    not_audio: |t| obs_encoder_bind_not_audio(t),
    not_video: |t| obs_encoder_audio_start_not_video(t),
    pass_texture: |c| obs_encoder_lifecycle_pass_texture(c),
    lcm_unset: |l| obs_encoder_lifecycle_lcm_unset(l),
    started_inc: |s| obs_encoder_lifecycle_started_inc(s),
    group_ready: |s, n| obs_encoder_lifecycle_group_ready(s, n),
    started_dec: |s| obs_encoder_lifecycle_started_dec(s),
    group_idle: |s| obs_encoder_lifecycle_group_idle(s),
    prepend_sei: |g, h, s| obs_encoder_lifecycle_prepend_sei(g, h, s),
};

const SPECIAL_U32: [u32; 24] = [
    0,
    1,
    2,
    3,
    4,
    5,
    6,
    7,
    8,
    15,
    16,
    255,
    256,
    0xffff,
    0x1_0000,
    0x7fff_ffff,
    0x8000_0000,
    0x8000_0001,
    0xdead_beef,
    u32::MAX - 3,
    u32::MAX - 2,
    u32::MAX - 1,
    u32::MAX,
    1000,
];

fn special_usize() -> impl Iterator<Item = usize> + Clone {
    SPECIAL_U32
        .into_iter()
        .map(|v| v as usize)
        .chain([usize::MAX - 2, usize::MAX - 1, usize::MAX])
        .chain([0x1_0000_0000usize, 0x1_0000_0001])
}

fn every_usize() -> impl Iterator<Item = usize> {
    (0..=1_000_000)
        .chain(usize::MAX - 1_000_000..=usize::MAX)
        .chain((0..=usize::MAX).step_by(usize::MAX / 4099))
        .chain(special_usize())
}

fn every_u32() -> impl Iterator<Item = u32> {
    (0..=1_000_000)
        .chain(u32::MAX - 1_000_000..=u32::MAX)
        .chain((0..=u32::MAX).step_by(65_521))
}

fn every_i32() -> impl Iterator<Item = c_int> {
    (-100_000..=100_000)
        .chain(c_int::MIN..c_int::MIN + 100_000)
        .chain(c_int::MAX - 100_000..=c_int::MAX)
        .chain((c_int::MIN..=c_int::MAX).step_by(65_521))
}

#[test]
fn constants_match_c() {
    let rust = [OBS_ENCODER_CAP_PASS_TEXTURE.into(), NOT_FOUND as u64];
    for (i, name) in CONSTANT_NAMES.iter().enumerate() {
        let mut ok: c_int = 0;
        // SAFETY: plain values and a valid out pointer.
        let c = unsafe { oracle_encoder_lifecycle_constant(i, &raw mut ok) };
        assert_eq!(ok, 1, "{name}");
        assert_eq!(c, rust[i], "{name}");
    }
    let mut ok: c_int = 1;
    // SAFETY: as above.
    unsafe { oracle_encoder_lifecycle_constant(CONSTANT_NAMES.len(), &raw mut ok) };
    assert_eq!(ok, 0);
    assert_eq!(OBS_ENCODER_CAP_PASS_TEXTURE, CAP_PASS_TEXTURE);
}

fn check_counts(n: usize) {
    // SAFETY (each): plain values.
    let (f, i, l) = unsafe {
        (
            oracle_obs_encoder_lifecycle_first(n),
            oracle_obs_encoder_lifecycle_found(n),
            oracle_obs_encoder_lifecycle_last(n),
        )
    };
    assert_eq!(obs_encoder_lifecycle_first(n), f, "{n}");
    assert_eq!(core::first(n), f, "{n}");
    assert_eq!(obs_encoder_lifecycle_found(n), i, "{n}");
    assert_eq!(core::found(n), i, "{n}");
    assert_eq!(obs_encoder_lifecycle_last(n), l, "{n}");
    assert_eq!(core::last(n), l, "{n}");
}

fn check_group(v: u32) {
    // SAFETY (each): plain values.
    let (u, i, inc, dec) = unsafe {
        (
            oracle_obs_encoder_lifecycle_lcm_unset(v),
            oracle_obs_encoder_lifecycle_group_idle(v),
            oracle_obs_encoder_lifecycle_started_inc(v),
            oracle_obs_encoder_lifecycle_started_dec(v),
        )
    };
    assert_eq!(obs_encoder_lifecycle_lcm_unset(v), u, "{v}");
    assert_eq!(core::lcm_unset(v), u, "{v}");
    assert_eq!(obs_encoder_lifecycle_group_idle(v), i, "{v}");
    assert_eq!(core::group_idle(v), i, "{v}");
    assert_eq!(obs_encoder_lifecycle_started_inc(v), inc, "{v}");
    assert_eq!(core::started_inc(v), inc, "{v}");
    assert_eq!(obs_encoder_lifecycle_started_dec(v), dec, "{v}");
    assert_eq!(core::started_dec(v), dec, "{v}");
}

fn check_ready(s: u32, n: usize) {
    // SAFETY: plain values.
    let c = unsafe { oracle_obs_encoder_lifecycle_group_ready(s, n) };
    assert_eq!(obs_encoder_lifecycle_group_ready(s, n), c, "{s} {n}");
    assert_eq!(core::group_ready(s, n), c, "{s} {n}");
}

fn check_sei(got: bool, has: bool, size: usize) {
    // SAFETY: plain values.
    let c = unsafe { oracle_obs_encoder_lifecycle_prepend_sei(got, has, size) };
    assert_eq!(obs_encoder_lifecycle_prepend_sei(got, has, size), c);
    assert_eq!(core::prepend_sei(got, has, size), c);
}

fn check_types(t: c_int) {
    // SAFETY (each): plain values.
    let (a, v) = unsafe {
        (
            oracle_obs_encoder_bind_not_audio(t),
            oracle_obs_encoder_audio_start_not_video(t),
        )
    };
    assert_eq!(obs_encoder_bind_not_audio(t), a, "{t}");
    assert_eq!(core::not_audio(t), a, "{t}");
    assert_eq!(obs_encoder_audio_start_not_video(t), v, "{t}");
    assert_eq!(core::not_video(t), v, "{t}");
}

#[test]
fn counts_and_indexes_match_c() {
    for n in every_usize() {
        check_counts(n);
    }
}

#[test]
fn group_counts_match_c() {
    for v in every_u32().chain(SPECIAL_U32) {
        check_group(v);
    }
    let sizes: Vec<usize> = (0..1024).chain(special_usize()).collect();
    for s in (0..1024).chain(SPECIAL_U32) {
        for &n in &sizes {
            check_ready(s, n);
        }
    }
}

#[test]
fn caps_match_c() {
    for low in 0..(1u32 << 20) {
        for high in [0, 0x1000_0000, 0x8000_0000, 0xfff0_0000] {
            let caps = low | high;
            // SAFETY: plain values.
            let c = unsafe { oracle_obs_encoder_lifecycle_pass_texture(caps) };
            assert_eq!(obs_encoder_lifecycle_pass_texture(caps), c, "{caps}");
            assert_eq!(core::pass_texture(caps), c, "{caps}");
        }
    }
}

#[test]
fn sei_matches_c() {
    for got in [false, true] {
        for has in [false, true] {
            for size in every_usize() {
                check_sei(got, has, size);
            }
        }
    }
}

#[test]
fn types_match_c() {
    for t in every_i32() {
        check_types(t);
    }
}

#[test]
fn callers_match_c() {
    let mut encoders = 0u32;
    for encoder_type in [i32::MIN, -1, TYPE_AUDIO, TYPE_VIDEO, 2, i32::MAX] {
        for caps in [0, 1, CAP_PASS_TEXTURE, CAP_PASS_TEXTURE | 1, u32::MAX] {
            for (has_mix, nv12, p010) in [
                (false, true, true),
                (true, false, false),
                (true, true, false),
                (true, false, true),
                (true, true, true),
            ] {
                for kf in [false, true] {
                    for sei in SEIS {
                        for group in [
                            None,
                            Some((0u32, 1usize, false)),
                            Some((0, 1, true)),
                            Some((1, 2, true)),
                            Some((0, 0, true)),
                            Some((u32::MAX, 3, true)),
                        ] {
                            for outputs in [vec![], vec![true, false], vec![false; 3]] {
                                let mut a = Stand::new(encoder_type);
                                a.caps = caps;
                                a.has_mix = has_mix;
                                a.nv12 = nv12;
                                a.p010 = p010;
                                a.has_request_keyframe = kf;
                                a.sei = sei;
                                a.divisor = 2;
                                a.outputs = outputs
                                    .into_iter()
                                    .map(|active| Output {
                                        active,
                                        ..Output::default()
                                    })
                                    .collect();
                                let mut ga = Group::default();
                                if let Some((started, num, destroy)) = group {
                                    a.in_group = true;
                                    ga.started = started;
                                    ga.num_encoders = num;
                                    ga.destroy_on_stop = destroy;
                                }
                                let mut b = a.clone();
                                let mut gb = ga.clone();
                                fixed_run(&mut a, &C, &mut ga);
                                fixed_run(&mut b, &SHIM, &mut gb);
                                assert_eq!(a, b);
                                assert_eq!(ga, gb);
                                encoders += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(encoders, 32_400);
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 20_000,
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    #[test]
    fn decisions_match_c(
        n in any::<usize>(),
        v in any::<u32>(),
        caps in any::<u32>(),
        t in any::<i32>(),
        got in any::<bool>(),
        has in any::<bool>(),
    ) {
        check_counts(n);
        check_group(v);
        check_ready(v, n);
        check_sei(got, has, n);
        check_types(t);
        // SAFETY: plain values.
        let c = unsafe { oracle_obs_encoder_lifecycle_pass_texture(caps) };
        prop_assert_eq!(obs_encoder_lifecycle_pass_texture(caps), c);
        prop_assert_eq!(core::pass_texture(caps), c);
    }

    #[test]
    fn callers_match_c_over_sequences(
        (e, g) in stand(),
        calls in proptest::collection::vec(call(), 1..16),
    ) {
        let (mut a, mut ga) = (e.clone(), g.clone());
        let (mut b, mut gb) = (e, g);
        for c in &calls {
            apply(&mut a, &C, &mut ga, c);
            apply(&mut b, &SHIM, &mut gb, c);
            prop_assert_eq!(&a, &b);
            prop_assert_eq!(&ga, &gb);
        }
    }
}
