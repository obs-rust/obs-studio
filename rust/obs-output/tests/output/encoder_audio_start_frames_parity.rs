//! Tier 3 parity: the shims of `obs_output::ffi::encoder_audio_start_frames`
//! (and the safe core under them) against the original
//! `libobs/obs-encoder-audio-start-frames.c.h`, compiled as
//! `oracle_obs_encoder_audio_start_frames_*` (obs-c-oracle,
//! oracle/encoder_audio_start_frames.c). The packet plan and the offset size
//! run as `buffer_audio()` and `calc_offset_size()` run them, on each side:
//! the C oracle of both files, the shims of both, and the safe cores of
//! both.
//!
//! Every output is compared exactly, on all three sides (oracle, shim,
//! core).
//!
//! Runs: the length over every pair of the 16 frame counts and 18 rates;
//! the offset over every pair of 26 special offsets and the 18 rates and 0;
//! the plan over 30 special timestamps around a video start, one 48 kHz
//! packet, one sample, 2^63 and `u64::MAX` (as the packet's start and the
//! video's) with the frame counts and rates; `calc_offset_size()` over the
//! same timestamps at four blocksizes; and proptests over full-range values.
//!
//! The oracle keeps the C's `util_mul_div64()`: on MSVC x64 its
//! `_udiv128()` faults when the quotient does not fit in 64 bits, so the
//! offset is compared only where it fits ([`fits`]); the length's quotient
//! always fits (`frames` is a `u32` and the rate at least 1).
//!
//! Intentional difference: the length at a sample rate of 0, where the C
//! divides by zero, is 0 in the port, so the packet waits when it starts at
//! or before the video and starts from the buffer otherwise. The oracle is
//! never asked the length at a zero rate, which excludes exactly those
//! inputs; Tier 1 pins the port's answer. The offset divides only by 10^9
//! and is compared at rate 0 too. Otherwise no input is excluded.
//! `test_encoder_audio_start_frames_steps` runs the C test against the C;
//! the output and encoder tests of Tier 2 run against the hybrid libobs,
//! where these shims replace the C.

use obs_c_oracle::encoder_audio_start::{
    oracle_obs_encoder_audio_start_offset_ns, oracle_obs_encoder_audio_start_offset_size,
    oracle_obs_encoder_audio_start_packet,
};
use obs_c_oracle::encoder_audio_start_frames::{
    oracle_obs_encoder_audio_start_frames_duration, oracle_obs_encoder_audio_start_frames_offset,
};
use obs_output::encoder_audio_start::{self as rules, PacketStart};
use obs_output::encoder_audio_start_frames::{duration, offset};
use obs_output::ffi::encoder_audio_start::{
    obs_encoder_audio_start_offset_ns, obs_encoder_audio_start_offset_size,
    obs_encoder_audio_start_packet,
};
use obs_output::ffi::encoder_audio_start_frames::{
    obs_encoder_audio_start_frames_duration, obs_encoder_audio_start_frames_offset,
};
use proptest::prelude::*;

const NS: u64 = 1_000_000_000;
const V: u64 = 5 * NS;
const PKT: u64 = 21_333_333;

const SPECIAL_FRAMES: [u32; 16] = [
    0,
    1,
    2,
    441,
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
    65536,
];

const SPECIAL_RATES: [u32; 18] = [
    1,
    2,
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
    2_000_000_000,
    0x7fff_ffff,
    0x8000_0000,
    u32::MAX - 2,
    u32::MAX - 1,
    u32::MAX,
];

const SPECIAL_OFFSETS: [u64; 26] = [
    0,
    1,
    2,
    999,
    20833,
    20834,
    PKT - 1,
    PKT,
    PKT + 1,
    NS - 1,
    NS,
    NS + 1,
    60 * NS,
    3600 * NS,
    86_400 * NS,
    0xffff_ffff,
    0x1_0000_0000,
    1 << 40,
    1 << 53,
    384_307_168_202_283,
    4_000_000_000_000_000_000,
    1 << 63,
    (1 << 63) + 1,
    u64::MAX - NS,
    u64::MAX - 1,
    u64::MAX,
];

const SPECIAL_TS: [u64; 30] = [
    0,
    1,
    2,
    V - NS,
    V - 2 * PKT,
    V - PKT - 1,
    V - PKT,
    V - PKT + 1,
    V - 20834,
    V - 20833,
    V - 20832,
    V - 2,
    V - 1,
    V,
    V + 1,
    V + 2,
    V + PKT,
    V + NS,
    (1 << 63) - 1,
    1 << 63,
    (1 << 63) + 1,
    u64::MAX - NS,
    u64::MAX - PKT - 1,
    u64::MAX - PKT,
    u64::MAX - PKT + 1,
    u64::MAX - 20833,
    u64::MAX - 2,
    u64::MAX - 1,
    u64::MAX,
    0x0123_4567_89ab_cdef,
];

const BLOCKSIZES: [usize; 4] = [1, 4, 8, 32];

/// `offset_ns * samplerate / 10^9` fits in 64 bits, so the C's
/// `util_mul_div64()` does not fault on MSVC x64.
fn fits(offset_ns: u64, samplerate: u32) -> bool {
    u128::from(offset_ns) * u128::from(samplerate) / u128::from(NS) <= u128::from(u64::MAX)
}

fn plan_of(p: PacketStart) -> (bool, bool, bool, bool) {
    match p {
        PacketStart::Wait => (true, false, false, false),
        PacketStart::Start {
            truncate,
            clear,
            from_buffer,
        } => (false, truncate, clear, from_buffer),
    }
}

#[track_caller]
fn check_duration(f: u32, r: u32) {
    assert_ne!(r, 0, "the oracle divides by the rate");
    // SAFETY: plain values; the rate is not 0.
    let c = unsafe { oracle_obs_encoder_audio_start_frames_duration(f, r) };
    assert_eq!(
        obs_encoder_audio_start_frames_duration(f, r),
        c,
        "shim duration({f}, {r})"
    );
    assert_eq!(duration(f, r), c, "core duration({f}, {r})");
}

#[track_caller]
fn check_offset(o: u64, r: u32) {
    if !fits(o, r) {
        return;
    }
    // SAFETY: plain values; the quotient fits in 64 bits.
    let c = unsafe { oracle_obs_encoder_audio_start_frames_offset(o, r) };
    assert_eq!(
        obs_encoder_audio_start_frames_offset(o, r),
        c,
        "shim offset({o}, {r})"
    );
    assert_eq!(offset(o, r), c, "core offset({o}, {r})");
}

/// `buffer_audio()`'s plan, the length then the packet rule, on each side.
#[track_caller]
fn check_plan(ts: u64, f: u32, r: u32, v: u64) {
    // SAFETY: plain values; the rate is not 0.
    let c = unsafe {
        oracle_obs_encoder_audio_start_packet(
            ts,
            oracle_obs_encoder_audio_start_frames_duration(f, r),
            v,
        )
    };
    let s = obs_encoder_audio_start_packet(ts, obs_encoder_audio_start_frames_duration(f, r), v);
    let k = plan_of(rules::packet_start(ts, duration(f, r), v));
    let c = (c.wait, c.truncate, c.clear, c.from_buffer);
    assert_eq!(
        (s.wait, s.truncate, s.clear, s.from_buffer),
        c,
        "shim plan({ts}, {f}, {r}, {v})"
    );
    assert_eq!(k, c, "core plan({ts}, {f}, {r}, {v})");
}

/// `calc_offset_size()`'s three steps, on each side.
#[track_caller]
fn check_offset_size(v: u64, a: u64, r: u32, b: usize) {
    let n = v.wrapping_sub(a);
    if !fits(n, r) {
        return;
    }
    // SAFETY: plain values; the quotient fits in 64 bits.
    let c = unsafe {
        let n = oracle_obs_encoder_audio_start_offset_ns(v, a);
        let n = oracle_obs_encoder_audio_start_frames_offset(n, r);
        oracle_obs_encoder_audio_start_offset_size(n, b)
    };
    let s = obs_encoder_audio_start_offset_size(
        obs_encoder_audio_start_frames_offset(obs_encoder_audio_start_offset_ns(v, a), r),
        b,
    );
    let k = rules::offset_size(offset(rules::offset_ns(v, a), r), b);
    assert_eq!(s, c, "shim offset_size({v}, {a}, {r}, {b})");
    assert_eq!(k, c, "core offset_size({v}, {a}, {r}, {b})");
}

#[test]
fn duration_special_grid() {
    for f in SPECIAL_FRAMES {
        for r in SPECIAL_RATES {
            check_duration(f, r);
        }
    }
}

#[test]
fn offset_special_grid() {
    for o in SPECIAL_OFFSETS {
        check_offset(o, 0);
        for r in SPECIAL_RATES {
            check_offset(o, r);
        }
    }
}

#[test]
fn plan_special_grid() {
    let mut pair = 0u64;
    for ts in SPECIAL_TS {
        // a video start of 0 means "not started": buffer_audio() never asks
        for v in SPECIAL_TS.into_iter().filter(|&v| v != 0) {
            pair += 1;
            // every start against the 5 s video start on every level
            if v != V && !obs_c_oracle::parity::keep(pair) {
                continue;
            }
            for f in SPECIAL_FRAMES {
                for r in SPECIAL_RATES {
                    check_plan(ts, f, r, v);
                }
            }
        }
    }
}

#[test]
fn offset_size_special_grid() {
    for v in SPECIAL_TS {
        for a in SPECIAL_TS {
            for r in SPECIAL_RATES {
                for b in BLOCKSIZES {
                    check_offset_size(v, a, r, b);
                }
            }
        }
    }
}

fn rate() -> impl Strategy<Value = u32> {
    prop_oneof![
        4 => 1u32..=384_000,
        2 => 1u32..=1_000_000_000,
        1 => 1u32..=u32::MAX,
    ]
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(4096),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    #[test]
    fn duration_matches(f in any::<u32>(), r in rate()) {
        check_duration(f, r);
    }

    #[test]
    fn offset_matches(o in any::<u64>(), r in any::<u32>()) {
        check_offset(o, r);
    }

    #[test]
    fn plan_matches(
        v in any::<u64>(),
        back in 0u64..=10_000_000_000,
        ahead in any::<bool>(),
        f in any::<u32>(),
        r in rate(),
    ) {
        let ts = if ahead { v.saturating_add(back) } else { v.saturating_sub(back) };
        check_plan(ts, f, r, v);
    }

    #[test]
    fn offset_size_matches(
        v in any::<u64>(),
        back in 0u64..=100_000_000_000,
        r in rate(),
        b in 1usize..=64,
    ) {
        check_offset_size(v, v.saturating_sub(back), r, b);
    }
}
