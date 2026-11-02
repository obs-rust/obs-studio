//! Tier 3 parity: the shims of `obs_output::ffi::output_raw_audio_trim`
//! (and the safe core under them) against the original
//! `libobs/obs-output-raw-audio-trim.c.h`, compiled as
//! `oracle_obs_output_raw_audio_trim_*` (obs-c-oracle,
//! oracle/output_raw_audio_trim.c). The drop runs as `prepare_audio()` runs
//! it, the length then `ends_before`, on each side: the C oracle of both
//! files, the shims of both, and the safe cores of both.
//!
//! Every output is compared exactly, on all three sides (oracle, shim,
//! core).
//!
//! Runs: the length over every pair of the 14 frame counts and 16 rates;
//! the drop over every pair of 35 special timestamps around a video
//! start, one 48 kHz packet, one sample, 2^63 and `u64::MAX` (as the start
//! and the video start, the start before it) with 14 special frame counts
//! and 16 special rates from 1 to `usize::MAX`; the cut in frames over 36
//! special cut lengths and the 16 rates and 0; whole trims as
//! `prepare_audio()` runs them (the drop, then for a kept packet the cut in
//! nanoseconds, the new timestamp, the cut in frames, the bytes for each
//! plane and the frames left) over the same grid; and proptests over
//! full-range values.
//!
//! The oracle keeps the C's `util_mul_div64()`: on MSVC x64 its
//! `_udiv128()` faults when the quotient does not fit in 64 bits, so the cut
//! in frames is compared there only where it fits ([`fits`]); the length's
//! quotient always fits (`frames` is a `u32` and the rate at least 1).
//!
//! Intentional difference: the length at a sample rate of 0, where the C
//! divides by zero, is 0 in the port, so a packet that starts before video
//! is dropped. The oracle is never asked the length at a zero rate, which
//! excludes exactly those inputs; Tier 1 pins the port's answer. The cut in frames divides only by 10^9 and is
//! compared at rate 0 too. Otherwise no input is excluded.
//! `test_output_raw_audio_trim_steps` runs the C test against the C;
//! `test_output_before_video` and `test_raw_output_video_reset` run against
//! the hybrid libobs, where these shims replace the C.

use obs_c_oracle::output_raw_audio::{
    oracle_obs_output_raw_audio_cut_bytes, oracle_obs_output_raw_audio_cut_frames,
    oracle_obs_output_raw_audio_cut_ns, oracle_obs_output_raw_audio_cut_ts,
    oracle_obs_output_raw_audio_ends_before,
};
use obs_c_oracle::output_raw_audio_trim::{
    oracle_obs_output_raw_audio_trim_duration, oracle_obs_output_raw_audio_trim_frames,
};
use obs_output::ffi::output_raw_audio::{
    obs_output_raw_audio_cut_bytes, obs_output_raw_audio_cut_frames, obs_output_raw_audio_cut_ns,
    obs_output_raw_audio_cut_ts, obs_output_raw_audio_ends_before,
};
use obs_output::ffi::output_raw_audio_trim::{
    obs_output_raw_audio_trim_duration, obs_output_raw_audio_trim_frames,
};
use obs_output::raw_audio::ends_before;
use obs_output::raw_audio_trim::{duration, frames};
use proptest::prelude::*;

const NS: u64 = 1_000_000_000;
const V: u64 = 5 * NS;
const PKT: u64 = 21_333_333;
const U32_MAX: usize = 4_294_967_295;

const SPECIAL_FRAMES: [u32; 14] = [
    0,
    1,
    2,
    480,
    1023,
    1024,
    1025,
    4096,
    48000,
    0x7fff_ffff,
    0x8000_0000,
    u32::MAX - 2,
    u32::MAX - 1,
    u32::MAX,
];

const SPECIAL_RATES: [usize; 16] = [
    1,
    2,
    3,
    8000,
    22050,
    44100,
    48000,
    96000,
    192_000,
    384_000,
    999_999_999,
    1_000_000_000,
    1_000_000_001,
    U32_MAX,
    1 << 40,
    usize::MAX,
];

/// Timestamps around the edges: 0, 2^63, `u64::MAX`, a video start, one
/// packet and one sample at 48 kHz, a second.
fn special_ts() -> Vec<u64> {
    let mut v = Vec::new();
    for base in [0, V, 0x8000_0000_0000_0000, u64::MAX, PKT, NS, 20833] {
        for d in [0u64, 1, 2] {
            v.push(base.wrapping_add(d));
            v.push(base.wrapping_sub(d));
        }
    }
    v.extend([V - PKT, V - PKT + 1, V - PKT - 1, u64::MAX - PKT]);
    v.sort_unstable();
    v.dedup();
    v
}

/// Cut lengths: up to a second, the longest packet at 48 kHz, and the
/// integer edges.
fn special_cuts() -> Vec<u64> {
    let mut v = vec![
        0,
        1,
        2,
        20833,
        20834,
        22675,
        22676,
        PKT - 1,
        PKT,
        PKT + 1,
        NS - 1,
        NS,
        NS + 1,
        89_478_485_312_499,
        89_478_485_312_500,
        89_478_485_312_501,
        u64::from(u32::MAX),
        u64::from(u32::MAX) + 1,
        0x7fff_ffff_ffff_ffff,
        0x8000_0000_0000_0000,
        u64::MAX - 1,
        u64::MAX,
    ];
    for k in [2u64, 3, 7, 10, 60, 3600, 86400] {
        v.push(k * NS);
        v.push(k * NS + 1);
    }
    v.sort_unstable();
    v.dedup();
    v
}

/// Whether `util_mul_div64(cut_ns, sample_rate, 10^9)` can be asked of the
/// oracle on this target: MSVC x64's `_udiv128()` faults past 64 bits.
fn fits(cut_ns: u64, sample_rate: usize) -> bool {
    !obs_util::platform::MUL_DIV64_IS_UDIV128
        || u128::from(cut_ns) * u128::try_from(sample_rate).expect("usize fits in u128")
            / u128::from(NS)
            <= u128::from(u64::MAX)
}

/// The drop on the oracle side: the C length, then the C `ends_before`.
fn oracle_drops(ts: u64, f: u32, v: u64, r: usize) -> bool {
    assert_ne!(r, 0, "the oracle divides by the rate");
    // SAFETY: plain integers; the rate is not 0.
    unsafe {
        let d = oracle_obs_output_raw_audio_trim_duration(f, r);
        oracle_obs_output_raw_audio_ends_before(ts, d, v)
    }
}

/// The drop through the shims, as `prepare_audio()` calls them.
fn shim_drops(ts: u64, f: u32, v: u64, r: usize) -> bool {
    obs_output_raw_audio_ends_before(ts, obs_output_raw_audio_trim_duration(f, r), v)
}

#[track_caller]
fn same_duration(f: u32, r: usize) {
    assert_ne!(r, 0, "the oracle divides by the rate");
    // SAFETY: plain integers; the rate is not 0.
    let want = unsafe { oracle_obs_output_raw_audio_trim_duration(f, r) };
    assert_eq!(
        obs_output_raw_audio_trim_duration(f, r),
        want,
        "shim duration({f}, {r})"
    );
    assert_eq!(duration(f, r), want, "core duration({f}, {r})");
}

#[track_caller]
fn same_drops(ts: u64, f: u32, v: u64, r: usize) {
    let want = oracle_drops(ts, f, v, r);
    assert_eq!(
        shim_drops(ts, f, v, r),
        want,
        "shim drops({ts}, {f}, {v}, {r})"
    );
    assert_eq!(
        ends_before(ts, duration(f, r), v),
        want,
        "core drops({ts}, {f}, {v}, {r})"
    );
}

#[track_caller]
fn same_frames(cut: u64, r: usize) {
    if !fits(cut, r) {
        return;
    }
    // SAFETY: plain integers; the quotient fits on this target.
    let want = unsafe { oracle_obs_output_raw_audio_trim_frames(cut, r) };
    assert_eq!(
        obs_output_raw_audio_trim_frames(cut, r),
        want,
        "shim frames({cut}, {r})"
    );
    assert_eq!(frames(cut, r), want, "core frames({cut}, {r})");
}

/// One packet through `prepare_audio()`'s cut path, as `obs-output.c` calls
/// it: `None` when dropped, else the timestamp, the bytes each plane
/// advances and the frames left.
type Trim = Option<(u64, usize, u32)>;

fn trim_oracle(ts: u64, f: u32, v: u64, r: usize, audio_size: usize) -> Trim {
    if oracle_drops(ts, f, v, r) {
        return None;
    }
    // SAFETY: plain integers; the rate is not 0 and the cut is shorter than
    // the packet, so the cut in frames fits.
    unsafe {
        let cut = oracle_obs_output_raw_audio_cut_ns(v, ts);
        let new_ts = oracle_obs_output_raw_audio_cut_ts(ts, cut);
        let cut = oracle_obs_output_raw_audio_trim_frames(cut, r);
        Some((
            new_ts,
            oracle_obs_output_raw_audio_cut_bytes(audio_size, cut),
            oracle_obs_output_raw_audio_cut_frames(f, cut),
        ))
    }
}

fn trim_shim(ts: u64, f: u32, v: u64, r: usize, audio_size: usize) -> Trim {
    if shim_drops(ts, f, v, r) {
        return None;
    }
    let cut = obs_output_raw_audio_cut_ns(v, ts);
    let new_ts = obs_output_raw_audio_cut_ts(ts, cut);
    let cut = obs_output_raw_audio_trim_frames(cut, r);
    Some((
        new_ts,
        obs_output_raw_audio_cut_bytes(audio_size, cut),
        obs_output_raw_audio_cut_frames(f, cut),
    ))
}

#[test]
fn duration_matches_c() {
    for f in SPECIAL_FRAMES {
        for r in SPECIAL_RATES {
            same_duration(f, r);
        }
    }
}

#[test]
fn drops_matches_c() {
    let ts = special_ts();
    for &v in &ts {
        for &t in ts.iter().filter(|&&t| t < v) {
            for f in SPECIAL_FRAMES {
                for r in SPECIAL_RATES {
                    same_drops(t, f, v, r);
                }
            }
        }
    }
}

#[test]
fn frames_matches_c() {
    for cut in special_cuts() {
        same_frames(cut, 0);
        for r in SPECIAL_RATES {
            same_frames(cut, r);
        }
    }
}

#[test]
fn trims_match_c() {
    let ts = special_ts();
    for &v in &ts {
        for &t in ts.iter().filter(|&&t| t < v) {
            for f in SPECIAL_FRAMES {
                for r in SPECIAL_RATES {
                    for size in [1, 4, 8, 32] {
                        assert_eq!(
                            trim_shim(t, f, v, r, size),
                            trim_oracle(t, f, v, r, size),
                            "trim({t}, {f}, {v}, {r}, {size})"
                        );
                    }
                }
            }
        }
    }
}

fn rate() -> impl Strategy<Value = usize> {
    prop_oneof![
        4 => 1usize..=384_000,
        2 => 1usize..=U32_MAX,
        1 => 1usize..=usize::MAX,
    ]
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(20_000),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    #[test]
    fn duration_matches_c_anywhere(f in any::<u32>(), r in rate()) {
        same_duration(f, r);
    }

    #[test]
    fn drops_matches_c_anywhere(v in any::<u64>(), t in any::<u64>(), f in any::<u32>(), r in rate()) {
        same_drops(t.min(v.saturating_sub(1)), f, v, r);
    }

    #[test]
    fn drops_matches_c_near_video(
        v in any::<u64>(),
        back in 1u64..=200_000_000,
        f in 0u32..=1_000_000,
        r in rate(),
    ) {
        same_drops(v.saturating_sub(back), f, v, r);
    }

    #[test]
    fn frames_matches_c_anywhere(cut in any::<u64>(), r in prop_oneof![Just(0usize), rate()]) {
        same_frames(cut, r);
    }

    #[test]
    fn trims_match_c_near_video(
        v in any::<u64>(),
        back in 1u64..=200_000_000,
        f in 0u32..=1_000_000,
        r in rate(),
        size in 1usize..=64,
    ) {
        let t = v.saturating_sub(back);
        prop_assert_eq!(trim_shim(t, f, v, r, size), trim_oracle(t, f, v, r, size));
    }
}
