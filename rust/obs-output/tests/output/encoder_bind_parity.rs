//! Tier 2 layout and Tier 3 parity: the shims of
//! `obs_output::ffi::encoder_bind` (and the core behind them) against the
//! oracle, the unchanged `libobs/obs-encoder-bind.c.h` compiled with its
//! functions renamed (`obs_c_oracle::encoder_bind`). The video type refusal
//! is `encoder_audio_start`'s, whose oracle is
//! `oracle_obs_encoder_audio_start_not_video`.
//!
//! Every comparison is exact, on all three sides: the constant and the
//! timebase struct's layout; the type refusals and trylock results over
//! -100,000 to 100,000, 100,000 values at each extreme and a stride through
//! all of `i32`; the timebases over every pair of frame rates below 512 and
//! every pair of 24 special values, with and without a video, and every
//! rate to 1,000,000 and a stride through all of `u32`, with and without an
//! audio; the divisor over 0 to 1,000,000 and a stride through all of
//! `u32`; the shutdown truth table; the callers on the stand-in encoder of
//! `test_encoder_bind_steps` (from Tier 1) with the oracle's gates and with
//! the shims, through every caller for every type, active and initialized
//! flag, media, divisor, trylock pair and output list; and proptests of
//! every decision over full-range values and of the stand-in over call
//! sequences. No input is excluded: nothing in the C divides or overflows.

use std::ffi::c_int;
use std::mem::{align_of, offset_of, size_of};

use obs_c_oracle::encoder_audio_start::oracle_obs_encoder_audio_start_not_video;
use obs_c_oracle::encoder_bind::{
    CONSTANT_NAMES, LAYOUT_NAMES, OracleTimebase, oracle_encoder_bind_constant,
    oracle_encoder_bind_layout, oracle_obs_encoder_bind_audio_timebase,
    oracle_obs_encoder_bind_divisor_override, oracle_obs_encoder_bind_locked,
    oracle_obs_encoder_bind_not_audio, oracle_obs_encoder_bind_shutdown_unstarted,
    oracle_obs_encoder_bind_video_timebase,
};
use obs_output::encoder_bind::{self as core, OBS_ENCODER_AUDIO, Timebase};
use obs_output::ffi::encoder_audio_start::obs_encoder_audio_start_not_video;
use obs_output::ffi::encoder_bind::{
    obs_encoder_bind_audio_timebase, obs_encoder_bind_divisor_override, obs_encoder_bind_locked,
    obs_encoder_bind_not_audio, obs_encoder_bind_shutdown_unstarted, obs_encoder_bind_timebase,
    obs_encoder_bind_video_timebase,
};
use proptest::prelude::*;

use super::encoder_bind::{
    A_48K, Gates, Media, Stand, TYPE_AUDIO, TYPE_VIDEO, V_60, V_FPS0, V_NTSC, apply, call,
};

fn from_oracle(t: OracleTimebase) -> Timebase {
    Timebase {
        num: t.num,
        den: t.den,
    }
}

fn from_shim(t: obs_encoder_bind_timebase) -> Timebase {
    Timebase {
        num: t.num,
        den: t.den,
    }
}

/// The oracle's functions as the stand-in's gates.
const C: Gates = Gates {
    // SAFETY (each): the oracle takes and returns plain values.
    not_video: |t| unsafe { oracle_obs_encoder_audio_start_not_video(t) },
    not_audio: |t| unsafe { oracle_obs_encoder_bind_not_audio(t) },
    video_timebase: |h, n, d| {
        from_oracle(unsafe { oracle_obs_encoder_bind_video_timebase(h, n, d) })
    },
    audio_timebase: |h, r| from_oracle(unsafe { oracle_obs_encoder_bind_audio_timebase(h, r) }),
    divisor_override: |d| unsafe { oracle_obs_encoder_bind_divisor_override(d) },
    locked: |r| unsafe { oracle_obs_encoder_bind_locked(r) },
    shutdown_unstarted: |i, n| unsafe { oracle_obs_encoder_bind_shutdown_unstarted(i, n) },
};

/// The shims as the stand-in's gates.
const SHIM: Gates = Gates {
    not_video: |t| obs_encoder_audio_start_not_video(t),
    not_audio: |t| obs_encoder_bind_not_audio(t),
    video_timebase: |h, n, d| from_shim(obs_encoder_bind_video_timebase(h, n, d)),
    audio_timebase: |h, r| from_shim(obs_encoder_bind_audio_timebase(h, r)),
    divisor_override: |d| obs_encoder_bind_divisor_override(d),
    locked: |r| obs_encoder_bind_locked(r),
    shutdown_unstarted: |i, n| obs_encoder_bind_shutdown_unstarted(i, n),
};

const SPECIAL_U32: [u32; 24] = [
    0,
    1,
    2,
    3,
    24,
    25,
    30,
    50,
    60,
    1000,
    1001,
    24000,
    30000,
    44100,
    48000,
    60000,
    0x7fff_ffff,
    0x8000_0000,
    0xffff,
    0x1_0000,
    u32::MAX - 2,
    u32::MAX - 1,
    u32::MAX,
    0xdead_beef,
];

fn every_i32() -> impl Iterator<Item = c_int> {
    (-100_000..=100_000)
        .chain(c_int::MIN..c_int::MIN + 100_000)
        .chain(c_int::MAX - 100_000..=c_int::MAX)
        .chain((c_int::MIN..=c_int::MAX).step_by(65_521))
}

fn every_u32() -> impl Iterator<Item = u32> {
    (0..=1_000_000)
        .chain(u32::MAX - 100_000..=u32::MAX)
        .chain((0..=u32::MAX).step_by(65_521))
}

fn check_type(t: c_int) {
    // SAFETY: plain values.
    let (oa, ov) = unsafe {
        (
            oracle_obs_encoder_bind_not_audio(t),
            oracle_obs_encoder_audio_start_not_video(t),
        )
    };
    assert_eq!(obs_encoder_bind_not_audio(t), oa, "{t}");
    assert_eq!(core::not_audio(t), oa, "{t}");
    assert_eq!(obs_encoder_audio_start_not_video(t), ov, "{t}");
    assert_eq!(core::not_video(t), ov, "{t}");
}

fn check_video_timebase(has: bool, num: u32, den: u32) {
    // SAFETY: plain values.
    let c = from_oracle(unsafe { oracle_obs_encoder_bind_video_timebase(has, num, den) });
    assert_eq!(
        from_shim(obs_encoder_bind_video_timebase(has, num, den)),
        c,
        "{has} {num} {den}"
    );
    assert_eq!(core::video_timebase(has, num, den), c, "{has} {num} {den}");
}

fn check_audio_timebase(has: bool, rate: u32) {
    // SAFETY: plain values.
    let c = from_oracle(unsafe { oracle_obs_encoder_bind_audio_timebase(has, rate) });
    assert_eq!(
        from_shim(obs_encoder_bind_audio_timebase(has, rate)),
        c,
        "{has} {rate}"
    );
    assert_eq!(core::audio_timebase(has, rate), c, "{has} {rate}");
}

#[test]
fn layout_matches_c() {
    let rust = [
        size_of::<obs_encoder_bind_timebase>(),
        align_of::<obs_encoder_bind_timebase>(),
        offset_of!(obs_encoder_bind_timebase, num),
        offset_of!(obs_encoder_bind_timebase, den),
    ];
    for (i, (name, rust)) in LAYOUT_NAMES.iter().zip(rust).enumerate() {
        // SAFETY: a plain value lookup.
        let c = unsafe { oracle_encoder_bind_layout(i) };
        assert_eq!(rust, c, "{name}");
    }
    // SAFETY: as above.
    assert_eq!(
        unsafe { oracle_encoder_bind_layout(LAYOUT_NAMES.len()) },
        usize::MAX
    );
    // The oracle's mirror of the returned struct has the same layout.
    assert_eq!(
        size_of::<OracleTimebase>(),
        size_of::<obs_encoder_bind_timebase>()
    );
    assert_eq!(
        offset_of!(OracleTimebase, den),
        offset_of!(obs_encoder_bind_timebase, den)
    );
}

#[test]
fn constants_match_c() {
    let rust: [i64; 1] = [i64::from(OBS_ENCODER_AUDIO)];
    for (i, (name, rust)) in CONSTANT_NAMES.iter().zip(rust).enumerate() {
        // SAFETY: a plain value lookup.
        let c = unsafe { oracle_encoder_bind_constant(i) };
        assert_eq!(rust, c, "{name}");
    }
    // SAFETY: as above.
    assert_eq!(
        unsafe { oracle_encoder_bind_constant(CONSTANT_NAMES.len()) },
        -1
    );
}

#[test]
fn type_refusals_match_c() {
    for t in every_i32() {
        check_type(t);
    }
}

#[test]
fn video_timebase_matches_c() {
    for has in [false, true] {
        for num in 0..512 {
            for den in 0..512 {
                check_video_timebase(has, num, den);
            }
        }
        for num in SPECIAL_U32 {
            for den in SPECIAL_U32 {
                check_video_timebase(has, num, den);
            }
        }
    }
}

#[test]
fn audio_timebase_matches_c() {
    for rate in every_u32() {
        check_audio_timebase(true, rate);
        check_audio_timebase(false, rate);
    }
}

#[test]
fn divisor_and_trylock_match_c() {
    for d in every_u32() {
        // SAFETY: plain values.
        let c = unsafe { oracle_obs_encoder_bind_divisor_override(d) };
        assert_eq!(obs_encoder_bind_divisor_override(d), c, "{d}");
        assert_eq!(core::divisor_override(d), c, "{d}");
    }
    for r in every_i32() {
        // SAFETY: plain values.
        let c = unsafe { oracle_obs_encoder_bind_locked(r) };
        assert_eq!(obs_encoder_bind_locked(r), c, "{r}");
        assert_eq!(core::locked(r), c, "{r}");
    }
    for init in [false, true] {
        for needed in [false, true] {
            // SAFETY: plain values.
            let c = unsafe { oracle_obs_encoder_bind_shutdown_unstarted(init, needed) };
            assert_eq!(obs_encoder_bind_shutdown_unstarted(init, needed), c);
            assert_eq!(core::shutdown_unstarted(init, needed), c);
        }
    }
}

#[test]
fn callers_match_c() {
    // every type, active and initialized flag, media, divisor, trylock pair
    // and output list, through every caller
    let videos = [V_NTSC, V_60];
    let medias = [
        None,
        Some(Media::Video(V_60)),
        Some(Media::Video(V_FPS0)),
        Some(Media::Video(V_NTSC)),
        Some(Media::Audio(A_48K)),
    ];
    let mut encoders = 0u32;
    for encoder_type in [i32::MIN, -1, TYPE_AUDIO, TYPE_VIDEO, 2, i32::MAX] {
        for active in [false, true] {
            for init in [false, true] {
                for media in medias {
                    for divisor in [0, 1, 2, u32::MAX] {
                        for (ot, it) in [(0, 0), (16, 0), (0, 16), (16, 16), (-1, 0)] {
                            for needs in [vec![], vec![false], vec![true, false], vec![false; 3]] {
                                let mut a = Stand::new(encoder_type, active, init);
                                a.media = media;
                                a.frame_rate_divisor = divisor;
                                a.needs = needs;
                                let mut b = a.clone();
                                let run = |e: &mut Stand, g: &Gates| {
                                    let got = [e.video(g), e.parent_video(g), e.audio(g)];
                                    e.set_video(g, Some(V_NTSC));
                                    e.set_audio(g, Some(A_48K));
                                    e.shutdown_unstarted(g, ot, it);
                                    let unbound = e.unbind_video(g, &videos);
                                    e.rebind_video(g, Some(V_60));
                                    let again = [e.video(g), e.parent_video(g), e.audio(g)];
                                    e.set_audio(g, None);
                                    e.set_video(g, None);
                                    (got, unbound, again)
                                };
                                assert_eq!(run(&mut a, &C), run(&mut b, &SHIM));
                                assert_eq!(a, b);
                                encoders += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(encoders, 9600);
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 20_000,
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    #[test]
    fn decisions_match_c(
        t in any::<i32>(),
        num in any::<u32>(),
        den in any::<u32>(),
        rate in any::<u32>(),
        has in any::<bool>(),
        r in any::<i32>(),
    ) {
        check_type(t);
        check_video_timebase(has, num, den);
        check_audio_timebase(has, rate);
        // SAFETY: plain values.
        let (d, l) = unsafe {
            (oracle_obs_encoder_bind_divisor_override(den), oracle_obs_encoder_bind_locked(r))
        };
        prop_assert_eq!(obs_encoder_bind_divisor_override(den), d);
        prop_assert_eq!(obs_encoder_bind_locked(r), l);
    }

    #[test]
    fn callers_match_c_over_sequences(
        encoder_type in prop_oneof![Just(TYPE_AUDIO), Just(TYPE_VIDEO), any::<i32>()],
        active in any::<bool>(),
        init in any::<bool>(),
        divisor in 0u32..3,
        needs in proptest::collection::vec(any::<bool>(), 0..5),
        calls in proptest::collection::vec(call(), 1..16),
    ) {
        let mut a = Stand::new(encoder_type, active, init);
        a.frame_rate_divisor = divisor;
        a.needs = needs;
        let mut b = a.clone();
        for c in &calls {
            prop_assert_eq!(apply(&mut a, &C, c), apply(&mut b, &SHIM, c));
            prop_assert_eq!(&a, &b);
        }
    }
}
