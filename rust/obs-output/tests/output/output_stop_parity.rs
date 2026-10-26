//! Tier 3 parity: the shims of `obs_output::ffi::output_stop` (and the core
//! behind them) against the oracle, the unchanged
//! `libobs/obs-output-stop.c.h` compiled with its functions renamed
//! (`obs_c_oracle::output_stop`). The interleave and mix tests are
//! `raw_audio`'s, whose oracles are `oracle_obs_output_raw_audio_interleave`
//! and `oracle_obs_output_raw_audio_mix_on`, and the first mixer
//! `video_checks'`, whose oracle is `oracle_obs_output_first_mixer`.
//!
//! Every comparison is exact, on all three sides: the constants and the
//! widths of `long` and `int`; the flag rules over every low 16-bit flag
//! word under 16 high words and a stride through all of `u32`; the delay
//! over 0 to 1,000,000, the top 100,000, a stride through all of `u64`,
//! every power of two and a stride of multiples of 2^32 (not 0, with the
//! low 32 bits clear); the restart count over -100,000 to 100,000, 100,000
//! values at each extreme of `long` and a stride through all of it; the
//! thread result over -100,000 to 100,000, both extremes of `int` and a
//! stride through all of it; the callers on the stand-in output of
//! `test_output_stop_steps` (from Tier 1) with the oracle's gates and with
//! the shims, through every caller for every low flag word with 2 high
//! words, each of eight atomics and the stopping event set and clear, a
//! restart reference or none, a delay or none and each of eight inputs
//! changed alone; and proptests of every decision over full-range values
//! and of the stand-in over call sequences. No input is excluded: nothing in
//! the C divides or overflows, and the mix test is only asked below six, as
//! `stop_raw_audio()` asks it.

use std::ffi::{c_int, c_long};

use obs_c_oracle::output_raw_audio::{
    oracle_obs_output_raw_audio_interleave, oracle_obs_output_raw_audio_mix_on,
};
use obs_c_oracle::output_stop::{
    CONSTANT_NAMES, oracle_obs_output_stop_delay_on, oracle_obs_output_stop_encoded,
    oracle_obs_output_stop_end_delay, oracle_obs_output_stop_has_audio,
    oracle_obs_output_stop_has_video, oracle_obs_output_stop_service,
    oracle_obs_output_stop_thread_failed, oracle_output_stop_constant,
};
use obs_c_oracle::output_video_checks::oracle_obs_output_first_mixer;
use obs_output::ffi::output_raw_audio::{
    obs_output_raw_audio_interleave, obs_output_raw_audio_mix_on,
};
use obs_output::ffi::output_stop::{
    obs_output_stop_delay_on, obs_output_stop_encoded, obs_output_stop_end_delay,
    obs_output_stop_has_audio, obs_output_stop_has_video, obs_output_stop_service,
    obs_output_stop_thread_failed,
};
use obs_output::ffi::output_video_checks::obs_output_first_mixer;
use obs_output::output_stop::{
    self as core, OBS_OUTPUT_AUDIO, OBS_OUTPUT_ENCODED, OBS_OUTPUT_SERVICE, OBS_OUTPUT_VIDEO,
};
use proptest::prelude::*;

use super::output_stop::{CORE, Gates, apply, run_all, stand_for, step};

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

/// The oracle's functions as the stand-in's gates.
const C: Gates = Gates {
    // SAFETY (each): the oracle takes and returns plain values.
    encoded: |f| unsafe { oracle_obs_output_stop_encoded(f) },
    delay_on: |d| unsafe { oracle_obs_output_stop_delay_on(d) },
    has_video: |f| unsafe { oracle_obs_output_stop_has_video(f) },
    has_audio: |f| unsafe { oracle_obs_output_stop_has_audio(f) },
    service: |f| unsafe { oracle_obs_output_stop_service(f) },
    end_delay: |r| unsafe { oracle_obs_output_stop_end_delay(c_refs(r)) },
    thread_failed: |r| unsafe { oracle_obs_output_stop_thread_failed(r) },
    interleave: |v, a| unsafe { oracle_obs_output_raw_audio_interleave(v, a) },
    mix_on: |m, i| unsafe { oracle_obs_output_raw_audio_mix_on(m, i) },
    first_mixer: |m| unsafe { oracle_obs_output_first_mixer(m) },
};

/// The shims as the stand-in's gates.
const SHIM: Gates = Gates {
    encoded: |f| obs_output_stop_encoded(f),
    delay_on: |d| obs_output_stop_delay_on(d),
    has_video: |f| obs_output_stop_has_video(f),
    has_audio: |f| obs_output_stop_has_audio(f),
    service: |f| obs_output_stop_service(f),
    end_delay: |r| obs_output_stop_end_delay(c_refs(r)),
    thread_failed: |r| obs_output_stop_thread_failed(r),
    interleave: |v, a| obs_output_raw_audio_interleave(v, a),
    mix_on: |m, i| obs_output_raw_audio_mix_on(m, i),
    first_mixer: |m| obs_output_first_mixer(m),
};

fn every_u64() -> impl Iterator<Item = u64> {
    (0..=1_000_000)
        .chain(u64::MAX - 100_000..=u64::MAX)
        .chain((0..=u64::MAX).step_by(0x0000_1000_0000_0001))
        .chain((0..64).map(|k| 1u64 << k))
        .chain((1..=0xffff_ffffu64).step_by(65_521).map(|k| k << 32))
}

fn every_long() -> impl Iterator<Item = c_long> {
    (-100_000..=100_000)
        .chain(c_long::MIN..c_long::MIN + 100_000)
        .chain(c_long::MAX - 100_000..=c_long::MAX)
        .chain(
            (c_long::MIN..=c_long::MAX).step_by(usize::try_from(c_long::MAX / 32_749).unwrap_or(1)),
        )
}

fn every_int() -> impl Iterator<Item = c_int> {
    (-100_000..=100_000)
        .chain(c_int::MIN..c_int::MIN + 1000)
        .chain(c_int::MAX - 1000..=c_int::MAX)
        .chain((c_int::MIN..=c_int::MAX).step_by(65_521))
}

fn check_flags(f: u32) {
    // SAFETY (each): the oracle takes and returns plain values.
    let c = unsafe {
        [
            oracle_obs_output_stop_encoded(f),
            oracle_obs_output_stop_has_video(f),
            oracle_obs_output_stop_has_audio(f),
            oracle_obs_output_stop_service(f),
        ]
    };
    let shim = [
        obs_output_stop_encoded(f),
        obs_output_stop_has_video(f),
        obs_output_stop_has_audio(f),
        obs_output_stop_service(f),
    ];
    let rust = [
        core::encoded(f),
        core::has_video(f),
        core::has_audio(f),
        core::service(f),
    ];
    assert_eq!(shim, c, "{f:#x}");
    assert_eq!(rust, c, "{f:#x}");
}

#[test]
fn constants_match_c() {
    let rust: [i64; 6] = [
        i64::from(OBS_OUTPUT_VIDEO),
        i64::from(OBS_OUTPUT_AUDIO),
        i64::from(OBS_OUTPUT_ENCODED),
        i64::from(OBS_OUTPUT_SERVICE),
        i64::try_from(size_of::<c_long>()).unwrap_or(-1),
        i64::try_from(size_of::<c_int>()).unwrap_or(-1),
    ];
    for (i, (name, rust)) in CONSTANT_NAMES.iter().zip(rust).enumerate() {
        // SAFETY: a plain value lookup.
        let c = unsafe { oracle_output_stop_constant(i) };
        assert_eq!(rust, c, "{name}");
    }
    // SAFETY: as above.
    assert_eq!(
        unsafe { oracle_output_stop_constant(CONSTANT_NAMES.len()) },
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
fn delay_matches_c() {
    for d in every_u64() {
        // SAFETY: a plain value.
        let c = unsafe { oracle_obs_output_stop_delay_on(d) };
        assert_eq!(obs_output_stop_delay_on(d), c, "{d}");
        assert_eq!(core::delay_on(d), c, "{d}");
    }
}

#[test]
fn end_delay_matches_c() {
    for r in every_long() {
        // SAFETY: a plain value.
        let c = unsafe { oracle_obs_output_stop_end_delay(r) };
        assert_eq!(obs_output_stop_end_delay(r), c, "{r}");
        assert_eq!(core::end_delay(wide(r)), c, "{r}");
    }
}

#[test]
fn thread_result_matches_c() {
    for r in every_int() {
        // SAFETY: a plain value.
        let c = unsafe { oracle_obs_output_stop_thread_failed(r) };
        assert_eq!(obs_output_stop_thread_failed(r), c, "{r}");
        assert_eq!(core::thread_failed(r), c, "{r}");
    }
}

#[test]
fn callers_match_c() {
    // every caller for every low flag word with two high words, each atomic
    // and the stopping event set and clear, a restart reference or none, a
    // delay or none and each input changed alone, on the oracle's gates and
    // on the shims
    for high in [0u32, 0xffff_fff0] {
        for low in 0..16u32 {
            let flags = high | low;
            for bits in 0..2048u32 {
                for change in 0..9u32 {
                    let o = stand_for(flags, bits, change);
                    let want = run_all(&o, &CORE);
                    assert_eq!(run_all(&o, &C), want);
                    assert_eq!(run_all(&o, &SHIM), want);
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
        d in any::<u64>(),
        r in any::<c_long>(),
        ret in any::<c_int>(),
    ) {
        check_flags(f);
        // SAFETY (each): the oracle takes and returns plain values.
        let c = unsafe { oracle_obs_output_stop_delay_on(d) };
        prop_assert_eq!(obs_output_stop_delay_on(d), c);
        prop_assert_eq!(core::delay_on(d), c);
        let c = unsafe { oracle_obs_output_stop_end_delay(r) };
        prop_assert_eq!(obs_output_stop_end_delay(r), c);
        prop_assert_eq!(core::end_delay(wide(r)), c);
        let c = unsafe { oracle_obs_output_stop_thread_failed(ret) };
        prop_assert_eq!(obs_output_stop_thread_failed(ret), c);
        prop_assert_eq!(core::thread_failed(ret), c);
    }

    #[test]
    fn callers_match_c_over_sequences(
        flags in prop_oneof![0u32..16, any::<u32>()],
        bits in 0u32..2048,
        change in 0u32..9,
        refs in prop_oneof![Just(0i64), Just(1), Just(-1), any::<i64>()],
        mask in prop_oneof![0usize..64, any::<usize>()],
        calls in proptest::collection::vec(step(), 1..16),
    ) {
        let mut a = stand_for(flags, bits, change);
        a.delay_restart_refs = refs;
        a.mixer_mask = mask;
        let mut b = a.clone();
        let mut s = a.clone();
        for c in &calls {
            apply(&mut a, &CORE, c);
            apply(&mut b, &C, c);
            apply(&mut s, &SHIM, c);
            prop_assert_eq!(&b, &a);
            prop_assert_eq!(&s, &a);
        }
    }
}
