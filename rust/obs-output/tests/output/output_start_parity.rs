//! Tier 3 parity: the shims of `obs_output::ffi::output_start` (and the
//! core behind them) against the oracle, the unchanged
//! `libobs/obs-output-start.c.h` compiled with its functions renamed
//! (`obs_c_oracle::output_start`). The delayed start is `raw_audio`'s,
//! whose oracle is `oracle_obs_output_raw_audio_delay_on`, and the
//! conversion fill `encoder_size`'s, whose oracle is
//! `oracle_obs_encoder_size_audio_defaults`.
//!
//! Every comparison is exact, on all three sides: the constants and the
//! width of `long`; the flag rules over every low 16-bit flag word under 16
//! high words and a stride through all of `u32`; the restart count over
//! -100,000 to 100,000, 100,000 values at each extreme of `long` and a
//! stride through all of it; the delay over 0 to 1,000,000, the top
//! 100,000 and a stride through all of `u32`; the conversion fill over
//! every combination of 12 formats, 12 rates and 12 speakers for the
//! conversion against 4 mixes; the callers on the stand-in output of
//! `test_output_start_steps` (from Tier 1) with the oracle's gates and with
//! the shims, through every caller for every low flag word with 2 high
//! words, each of six atomics set and clear, three restart counts, two
//! delays and each of twelve call results failing alone; and proptests of
//! every decision over full-range values and of the stand-in over call
//! sequences. No input is excluded: nothing in the C divides or overflows.

use std::ffi::c_long;

use obs_c_oracle::encoder_size::{AudioFill as OracleFill, oracle_obs_encoder_size_audio_defaults};
use obs_c_oracle::output_raw_audio::oracle_obs_output_raw_audio_delay_on;
use obs_c_oracle::output_start::{
    CONSTANT_NAMES, oracle_obs_output_start_drop_restart_ref, oracle_obs_output_start_init_audio,
    oracle_obs_output_start_init_video, oracle_obs_output_start_pair,
    oracle_obs_output_start_reset_raw, oracle_obs_output_start_service,
    oracle_output_start_constant,
};
use obs_output::ffi::encoder_size::{obs_encoder_audio_fill, obs_encoder_size_audio_defaults};
use obs_output::ffi::output_raw_audio::obs_output_raw_audio_delay_on;
use obs_output::ffi::output_start::{
    obs_output_start_drop_restart_ref, obs_output_start_init_audio, obs_output_start_init_video,
    obs_output_start_pair, obs_output_start_reset_raw, obs_output_start_service,
};
use obs_output::output_start::{
    self as core, AudioFill, OBS_OUTPUT_AUDIO, OBS_OUTPUT_ENCODED, OBS_OUTPUT_SERVICE,
    OBS_OUTPUT_VIDEO,
};
use proptest::prelude::*;

use super::output_start::{
    CORE, FORMAT_16BIT, FORMAT_FLOAT_PLANAR, FORMAT_UNKNOWN, Gates, SPEAKERS_STEREO,
    SPEAKERS_UNKNOWN, apply, audio_fill, fill, run_all, stand_for, step,
};

/// A restart count as the C's `long`: the same value where it fits, else
/// the nearest end, which keeps "not 0" (`long` is 32 bits on Windows).
#[allow(clippy::unnecessary_fallible_conversions)]
fn c_refs(r: i64) -> c_long {
    c_long::try_from(r).unwrap_or(if r < 0 { c_long::MIN } else { c_long::MAX })
}

#[allow(clippy::useless_conversion)]
fn wide(r: c_long) -> i64 {
    i64::from(r)
}

fn to_oracle(f: AudioFill) -> OracleFill {
    OracleFill {
        format: f.format,
        samples_per_sec: f.samples_per_sec,
        speakers: f.speakers,
    }
}

fn from_oracle(f: OracleFill) -> AudioFill {
    fill(f.format, f.samples_per_sec, f.speakers)
}

fn to_shim(f: AudioFill) -> obs_encoder_audio_fill {
    obs_encoder_audio_fill {
        format: f.format,
        samples_per_sec: f.samples_per_sec,
        speakers: f.speakers,
    }
}

fn from_shim(f: obs_encoder_audio_fill) -> AudioFill {
    fill(f.format, f.samples_per_sec, f.speakers)
}

fn oracle_fill(conv: AudioFill, mix: AudioFill) -> AudioFill {
    let (c, m) = (to_oracle(conv), to_oracle(mix));
    // SAFETY: both point to live fills for the call.
    from_oracle(unsafe {
        oracle_obs_encoder_size_audio_defaults(
            std::ptr::from_ref(&c).cast(),
            std::ptr::from_ref(&m).cast(),
        )
    })
}

fn shim_fill(conv: AudioFill, mix: AudioFill) -> AudioFill {
    let (c, m) = (to_shim(conv), to_shim(mix));
    // SAFETY: both point to live fills for the call.
    from_shim(unsafe { obs_encoder_size_audio_defaults(&c, &m) })
}

/// The oracle's functions as the stand-in's gates.
const C: Gates = Gates {
    // SAFETY (each): the oracle takes and returns plain values.
    service: |f| unsafe { oracle_obs_output_start_service(f) },
    drop_restart_ref: |r| unsafe { oracle_obs_output_start_drop_restart_ref(c_refs(r)) },
    init_video: |f| unsafe { oracle_obs_output_start_init_video(f) },
    init_audio: |f| unsafe { oracle_obs_output_start_init_audio(f) },
    reset_raw: |f| unsafe { oracle_obs_output_start_reset_raw(f) },
    pair: |f| unsafe { oracle_obs_output_start_pair(f) },
    delay_on: |d| unsafe { oracle_obs_output_raw_audio_delay_on(d) },
    audio_defaults: oracle_fill,
};

/// The shims as the stand-in's gates.
const SHIM: Gates = Gates {
    service: |f| obs_output_start_service(f),
    drop_restart_ref: |r| obs_output_start_drop_restart_ref(c_refs(r)),
    init_video: |f| obs_output_start_init_video(f),
    init_audio: |f| obs_output_start_init_audio(f),
    reset_raw: |f| obs_output_start_reset_raw(f),
    pair: |f| obs_output_start_pair(f),
    delay_on: |d| obs_output_raw_audio_delay_on(d),
    audio_defaults: shim_fill,
};

fn every_u32() -> impl Iterator<Item = u32> {
    (0..=1_000_000)
        .chain(u32::MAX - 100_000..=u32::MAX)
        .chain((0..=u32::MAX).step_by(65_521))
}

fn every_long() -> impl Iterator<Item = c_long> {
    (-100_000..=100_000)
        .chain(c_long::MIN..c_long::MIN + 100_000)
        .chain(c_long::MAX - 100_000..=c_long::MAX)
        .chain(
            (c_long::MIN..=c_long::MAX).step_by(usize::try_from(c_long::MAX / 32_749).unwrap_or(1)),
        )
}

fn check_flags(f: u32) {
    // SAFETY (each): the oracle takes and returns plain values.
    let c = unsafe {
        [
            oracle_obs_output_start_service(f),
            oracle_obs_output_start_init_video(f),
            oracle_obs_output_start_init_audio(f),
            oracle_obs_output_start_reset_raw(f),
            oracle_obs_output_start_pair(f),
        ]
    };
    let shim = [
        obs_output_start_service(f),
        obs_output_start_init_video(f),
        obs_output_start_init_audio(f),
        obs_output_start_reset_raw(f),
        obs_output_start_pair(f),
    ];
    let rust = [
        core::service(f),
        core::init_video(f),
        core::init_audio(f),
        core::reset_raw(f),
        core::pair(f),
    ];
    assert_eq!(shim, c, "{f:#x}");
    assert_eq!(rust, c, "{f:#x}");
}

#[test]
fn constants_match_c() {
    let rust: [i64; 5] = [
        i64::from(OBS_OUTPUT_VIDEO),
        i64::from(OBS_OUTPUT_AUDIO),
        i64::from(OBS_OUTPUT_ENCODED),
        i64::from(OBS_OUTPUT_SERVICE),
        i64::try_from(size_of::<c_long>()).unwrap_or(-1),
    ];
    for (i, (name, rust)) in CONSTANT_NAMES.iter().zip(rust).enumerate() {
        // SAFETY: a plain value lookup.
        let c = unsafe { oracle_output_start_constant(i) };
        assert_eq!(rust, c, "{name}");
    }
    // SAFETY: as above.
    assert_eq!(
        unsafe { oracle_output_start_constant(CONSTANT_NAMES.len()) },
        -1
    );
}

#[test]
fn flag_rules_match_c() {
    for high in 0..16u32 {
        for low in 0..=0xffffu32 {
            check_flags((high << 16) | (high << 28) | low);
        }
    }
    for f in (0..=u32::MAX).step_by(4099) {
        check_flags(f);
    }
    check_flags(u32::MAX);
}

#[test]
fn restart_ref_matches_c() {
    for r in every_long() {
        // SAFETY: a plain value.
        let c = unsafe { oracle_obs_output_start_drop_restart_ref(r) };
        assert_eq!(obs_output_start_drop_restart_ref(r), c, "{r}");
        assert_eq!(core::drop_restart_ref(wide(r)), c, "{r}");
    }
}

#[test]
fn delayed_start_matches_c() {
    for d in every_u32() {
        // SAFETY: a plain value.
        let c = unsafe { oracle_obs_output_raw_audio_delay_on(d) };
        assert_eq!(obs_output_raw_audio_delay_on(d), c, "{d}");
        assert_eq!(core::delay_on(d), c, "{d}");
    }
}

#[test]
fn conversion_fill_matches_c() {
    let formats = [
        i32::MIN,
        -2,
        -1,
        FORMAT_UNKNOWN,
        1,
        FORMAT_16BIT,
        3,
        7,
        FORMAT_FLOAT_PLANAR,
        9,
        0x7fff,
        i32::MAX,
    ];
    let rates = [
        0,
        1,
        2,
        8000,
        22050,
        32000,
        44100,
        48000,
        96000,
        0x7fff_ffff,
        0x8000_0000,
        u32::MAX,
    ];
    let speakers = [
        i32::MIN,
        -1,
        SPEAKERS_UNKNOWN,
        1,
        SPEAKERS_STEREO,
        3,
        4,
        5,
        6,
        8,
        9,
        i32::MAX,
    ];
    let mixes = [
        fill(FORMAT_FLOAT_PLANAR, 48000, SPEAKERS_STEREO),
        fill(FORMAT_16BIT, 44100, 6),
        fill(FORMAT_UNKNOWN, 0, SPEAKERS_UNKNOWN),
        fill(i32::MAX, u32::MAX, i32::MIN),
    ];
    for &f in &formats {
        for &r in &rates {
            for &s in &speakers {
                for &mix in &mixes {
                    let conv = fill(f, r, s);
                    let c = oracle_fill(conv, mix);
                    assert_eq!(shim_fill(conv, mix), c, "{conv:?} {mix:?}");
                    assert_eq!(core::audio_defaults(conv, mix), c, "{conv:?} {mix:?}");
                }
            }
        }
    }
}

#[test]
fn callers_match_c() {
    // every caller for every low flag word with two high words, each
    // atomic set and clear, three restart counts, two delays and each call
    // result failing alone, on the oracle's gates and on the shims
    for high in [0u32, 0xffff_fff0] {
        for low in 0..16u32 {
            let flags = high | low;
            for bits in 0..64u32 {
                for refs in [0, 1, i64::MAX] {
                    for delay_sec in [0, u32::MAX] {
                        for fail in 0..12u32 {
                            let o = stand_for(flags, bits, refs, delay_sec, fail);
                            let want = run_all(&o, &CORE);
                            assert_eq!(run_all(&o, &C), want);
                            assert_eq!(run_all(&o, &SHIM), want);
                        }
                    }
                }
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 20_000,
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    #[test]
    fn decisions_match_c(
        f in any::<u32>(),
        r in any::<c_long>(),
        d in any::<u32>(),
        conv in audio_fill(),
        mix in audio_fill(),
    ) {
        check_flags(f);
        // SAFETY (each): the oracle takes and returns plain values.
        let c = unsafe { oracle_obs_output_start_drop_restart_ref(r) };
        prop_assert_eq!(obs_output_start_drop_restart_ref(r), c);
        prop_assert_eq!(core::drop_restart_ref(wide(r)), c);
        let c = unsafe { oracle_obs_output_raw_audio_delay_on(d) };
        prop_assert_eq!(obs_output_raw_audio_delay_on(d), c);
        prop_assert_eq!(core::delay_on(d), c);
        let c = oracle_fill(conv, mix);
        prop_assert_eq!(shim_fill(conv, mix), c);
        prop_assert_eq!(core::audio_defaults(conv, mix), c);
    }

    #[test]
    fn callers_match_c_over_sequences(
        flags in prop_oneof![0u32..16, any::<u32>()],
        bits in 0u32..64,
        refs in prop_oneof![Just(0i64), Just(1), Just(2), any::<i64>()],
        delay_sec in prop_oneof![Just(0u32), Just(5), any::<u32>()],
        conv in audio_fill(),
        mix in audio_fill(),
        calls in proptest::collection::vec(step(), 1..16),
    ) {
        let mut a = stand_for(flags, bits, refs, delay_sec, 0);
        a.conv = conv;
        a.mix = mix;
        let mut b = a.clone();
        let mut s = a.clone();
        for c in &calls {
            let want = apply(&mut a, &CORE, c);
            prop_assert_eq!(apply(&mut b, &C, c), want);
            prop_assert_eq!(apply(&mut s, &SHIM, c), want);
            prop_assert_eq!(&b, &a);
            prop_assert_eq!(&s, &a);
        }
    }
}
