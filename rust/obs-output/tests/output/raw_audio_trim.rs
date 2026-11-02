//! Tier 1: `obs_output::raw_audio_trim`, the two sample-rate conversions of
//! `prepare_audio()`.
//!
//! The `steps_*` cases port `test/cmocka/test_output_raw_audio_trim_steps.c`
//! 1:1: its 7 cases under the same names, in the same order, with the same
//! values and expectations, the "characterized, not endorsed" ones included.
//! Like the C, each case is also checked against the expressions
//! `obs-output.c` had before the move ([`master_drops`], [`master_frames`]),
//! here with `obs_util`'s `mul_div64`, the C's `util_mul_div64()` on each
//! platform. The drop cases run the port's length and `ends_before`, as
//! `prepare_audio()` does ([`drops`]).
//!
//! The `edge_*` cases are new: the length alone, the zero sample rate (the
//! intentional difference: the length is 0 where the C divides by zero, so
//! a packet that starts before video is dropped), rates
//! past `u32::MAX`, the bounds the cut path keeps (a kept packet's cut in
//! frames is below its frames, so the C's `uint32_t` casts never truncate),
//! and proptests of both functions over full-range inputs against the
//! expressions.

use obs_output::raw_audio::{cut_frames, cut_ns, ends_before};
use obs_output::raw_audio_trim::{duration, frames};
use obs_util::util_uint64::mul_div64;
use proptest::prelude::*;

const NS: u64 = 1_000_000_000;
/// One 1024-frame packet at 48 kHz: `util_mul_div64(1024, NS, 48000)`.
const PKT: u64 = 21_333_333;
const U32_MAX: usize = 4_294_967_295;

/// `usize` to `u64`, lossless on every supported target.
fn wide(n: usize) -> u64 {
    u64::try_from(n).expect("usize fits in u64")
}

/// The drop as `prepare_audio()` runs it: the port's length, then
/// `raw_audio`'s `ends_before`.
fn drops(timestamp: u64, frames: u32, video_start_ts: u64, sample_rate: usize) -> bool {
    ends_before(timestamp, duration(frames, sample_rate), video_start_ts)
}

/// The drop as `prepare_audio()` wrote it before the move. `sample_rate`
/// must not be 0.
fn master_drops(timestamp: u64, frames: u32, video_start_ts: u64, sample_rate: usize) -> bool {
    let duration = mul_div64(u64::from(frames), NS, wide(sample_rate));
    ends_before(timestamp, duration, video_start_ts)
}

/// The cut in frames as `prepare_audio()` wrote it before the move.
fn master_frames(cut_ns: u64, sample_rate: usize) -> u64 {
    mul_div64(cut_ns, wide(sample_rate), NS)
}

#[track_caller]
fn check_drops(timestamp: u64, frames: u32, video_start_ts: u64, sample_rate: usize, expect: bool) {
    let got = drops(timestamp, frames, video_start_ts, sample_rate);
    assert_eq!(
        got, expect,
        "drops({timestamp}, {frames}, {video_start_ts}, {sample_rate})"
    );
    assert_eq!(
        got,
        master_drops(timestamp, frames, video_start_ts, sample_rate),
        "drops({timestamp}, {frames}, {video_start_ts}, {sample_rate}) against the C"
    );
}

#[track_caller]
fn check_frames(cut_ns: u64, sample_rate: usize, expect: u64) {
    let got = frames(cut_ns, sample_rate);
    assert_eq!(got, expect, "frames({cut_ns}, {sample_rate})");
    assert_eq!(
        got,
        master_frames(cut_ns, sample_rate),
        "frames({cut_ns}, {sample_rate}) against the C"
    );
}

// ---------------------------------------------------------------------------
// test_output_raw_audio_trim_steps.c, 1:1

#[test]
fn steps_drops_at_48k() {
    let v = 5 * NS;

    // ends before the video start, exactly at it (dropped), one ns into it
    check_drops(v - PKT - 1, 1024, v, 48000, true);
    check_drops(v - PKT, 1024, v, 48000, true);
    check_drops(v - PKT + 1, 1024, v, 48000, false);
    // starts a nanosecond before the video start
    check_drops(v - 1, 1024, v, 48000, false);
    // the length rounds down: 1 frame is 20833 ns
    check_drops(v - 20833, 1, v, 48000, true);
    check_drops(v - 20832, 1, v, 48000, false);
}

#[test]
fn steps_drops_other_rates() {
    let v = 5 * NS;

    // 1024 frames: 23219954 ns at 44.1 kHz, 10666666 ns at 96 kHz
    check_drops(v - 23_219_954, 1024, v, 44100, true);
    check_drops(v - 23_219_953, 1024, v, 44100, false);
    check_drops(v - 10_666_666, 1024, v, 96000, true);
    check_drops(v - 10_666_665, 1024, v, 96000, false);
    // 1 Hz: a frame is a second
    check_drops(v - NS, 1, v, 1, true);
    check_drops(v - NS + 1, 1, v, 1, false);
    // over 1 GHz a frame lasts 0 ns, so one frame before video is dropped
    // (characterized, not endorsed); 1024 frames at 2 GHz are 512 ns
    check_drops(v - 1, 1, v, 2_000_000_000, true);
    check_drops(v - 512, 1024, v, 2_000_000_000, true);
    check_drops(v - 511, 1024, v, 2_000_000_000, false);
    // the highest rate: u32::MAX - 1 frames are 999999999 ns
    check_drops(v - 999_999_999, u32::MAX - 1, v, U32_MAX, true);
    check_drops(v - 999_999_998, u32::MAX - 1, v, U32_MAX, false);
    check_drops(v - NS, u32::MAX, v, U32_MAX, true);
    check_drops(v - 1, u32::MAX, v, U32_MAX, false);
}

#[test]
fn steps_drops_edges() {
    // no frames: 0 ns long, dropped (characterized, not endorsed)
    check_drops(0, 0, 1, 48000, true);
    // the most frames: 89478485312500 ns at 48 kHz
    check_drops(0, u32::MAX, 89_478_485_312_500, 48000, true);
    check_drops(0, u32::MAX, 89_478_485_312_499, 48000, false);
    // at the top of the clock: ending exactly at the video start is
    // dropped, a nanosecond past it is kept
    check_drops(u64::MAX - PKT, 1024, u64::MAX, 48000, true);
    check_drops(u64::MAX - PKT - 1, 1024, u64::MAX - 1, 48000, true);
    check_drops(u64::MAX - PKT - 1, 1024, u64::MAX - 2, 48000, false);
    // the end wraps past u64::MAX, so it is dropped although it ends after
    // the video start (characterized, not endorsed)
    check_drops(u64::MAX - PKT + 1, 1024, u64::MAX, 48000, true);
    check_drops(u64::MAX - 1, 1024, u64::MAX, 48000, true);
}

#[test]
fn steps_frames_at_48k() {
    check_frames(0, 48000, 0);
    // less than one sample (20833.3 ns) cuts no frame
    check_frames(1, 48000, 0);
    check_frames(20833, 48000, 0);
    check_frames(20834, 48000, 1);
    check_frames(PKT - 1, 48000, 1023);
    check_frames(PKT, 48000, 1023);
    check_frames(PKT + 1, 48000, 1024);
    check_frames(NS, 48000, 48000);
    check_frames(NS - 1, 48000, 47999);
}

#[test]
fn steps_frames_other_rates() {
    check_frames(22675, 44100, 0);
    check_frames(22676, 44100, 1);
    check_frames(NS, 44100, 44100);
    check_frames(10416, 96000, 0);
    check_frames(10417, 96000, 1);
    check_frames(NS - 1, 1, 0);
    check_frames(NS, 1, 1);
    check_frames(1, U32_MAX, 4);
    check_frames(NS, U32_MAX, u64::from(u32::MAX));
    // a zero rate cuts nothing; nothing divides by it here
    check_frames(NS, 0, 0);
    check_frames(u64::MAX, 0, 0);
}

#[test]
fn steps_frames_edges() {
    // the longest cut a kept packet of u32::MAX frames has at 48 kHz, one ns
    // short of its length, and the length itself
    check_frames(89_478_485_312_499, 48000, u64::from(u32::MAX - 1));
    check_frames(89_478_485_312_500, 48000, u64::from(u32::MAX));
    // past what a kept packet has: the C's formula, exact here
    check_frames(u64::MAX, 48000, 885_443_715_538_058);
    check_frames(u64::MAX, 1, 18_446_744_073);
}

#[test]
fn steps_trim_grid() {
    const STARTS: [u64; 7] = [
        1,
        1000,
        NS,
        5 * NS,
        0x8000_0000_0000_0000,
        u64::MAX - 1,
        u64::MAX,
    ];
    const BEFORE: [u64; 9] = [
        1,
        2,
        20833,
        20834,
        PKT - 1,
        PKT,
        PKT + 1,
        NS,
        89_478_485_312_500,
    ];
    const FRAMES: [u32; 10] = [0, 1, 2, 480, 1023, 1024, 1025, 4096, u32::MAX - 1, u32::MAX];
    const RATES: [usize; 11] = [
        1,
        2,
        8000,
        22050,
        44100,
        48000,
        96000,
        192_000,
        384_000,
        1_000_000_000,
        U32_MAX,
    ];

    for v in STARTS {
        for b in BEFORE {
            if b > v {
                continue;
            }
            let ts = v - b;
            for f in FRAMES {
                for r in RATES {
                    let drop = master_drops(ts, f, v, r);
                    check_drops(ts, f, v, r, drop);
                    if drop {
                        continue;
                    }
                    let cut = cut_ns(v, ts);
                    let cut_frames = master_frames(cut, r);
                    check_frames(cut, r, cut_frames);
                    // a kept packet's cut is shorter than the packet
                    assert!(cut_frames < u64::from(f));
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// new

/// The packet's length alone, as `prepare_audio()` passes it to
/// `ends_before`: `util_mul_div64(frames, 10^9, sample_rate)`, rounded
/// down, and 0 at a zero rate (the intentional difference: the C divides
/// by zero; a packet that starts before video and lasts 0 ns is dropped).
#[test]
fn edge_duration() {
    for (f, r, want) in [
        (1024, 48000, PKT),
        (1, 48000, 20833),
        (1024, 44100, 23_219_954),
        (1024, 96000, 10_666_666),
        (1, 1, NS),
        (1, 2_000_000_000, 0),
        (1024, 2_000_000_000, 512),
        (u32::MAX - 1, U32_MAX, 999_999_999),
        (u32::MAX, U32_MAX, NS),
        (u32::MAX, 48000, 89_478_485_312_500),
        (0, 48000, 0),
        (1024, usize::MAX, 0),
        (0, 0, 0),
        (1024, 0, 0),
        (u32::MAX, 0, 0),
    ] {
        assert_eq!(duration(f, r), want, "duration({f}, {r})");
        if r != 0 {
            assert_eq!(
                duration(f, r),
                mul_div64(u64::from(f), NS, wide(r)),
                "duration({f}, {r}) against the C"
            );
        }
    }
    // a zero rate drops a packet that starts before video, through the
    // caller's own test
    assert!(ends_before(5 * NS - 1, duration(1024, 0), 5 * NS));
}

/// A zero rate drops a packet that starts before the video, wherever it
/// ends: the C divides by zero here (the intentional difference).
#[test]
fn edge_zero_rate_drops() {
    let v = 5 * NS;
    for (ts, f) in [
        (v - 1, 1024),
        (v - 1, u32::MAX),
        (0, 0),
        (0, u32::MAX),
        (v - PKT, 1024),
        (u64::MAX - 1, 1),
    ] {
        assert!(drops(ts, f, v.max(ts + 1), 0), "drops({ts}, {f}, 0)");
    }
}

/// Rates past `u32::MAX` (`sample_rate` is a `size_t`): the C's formula on
/// each platform.
#[test]
fn edge_rates_past_u32() {
    let rates = [U32_MAX + 1, 1 << 40, usize::MAX];
    let v = 5 * NS;
    for r in rates {
        for f in [1, 1024, u32::MAX] {
            for b in [1, 2, PKT, NS] {
                check_drops(v - b, f, v, r, master_drops(v - b, f, v, r));
            }
        }
        // a cut shorter than any packet at that rate: no frame
        check_frames(0, r, 0);
    }
}

/// On the cut path the C's `uint32_t` casts never truncate: the cut in
/// frames is below the packet's frames, so the frames left are their
/// difference and nothing wraps.
#[test]
fn edge_kept_cut_fits() {
    let v = u64::MAX;
    for r in [1, 44100, 48000, 96000, U32_MAX] {
        for f in [1, 1024, u32::MAX] {
            let duration = mul_div64(u64::from(f), NS, wide(r));
            for b in [1, duration / 2, duration.saturating_sub(1)] {
                if b == 0 || drops(v - b, f, v, r) {
                    continue;
                }
                let n = frames(cut_ns(v, v - b), r);
                assert!(n < u64::from(f), "{r} {f} {b}");
                assert_eq!(u64::from(cut_frames(f, n)), u64::from(f) - n, "{r} {f} {b}");
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
        cases: obs_c_oracle::parity::cases(4096),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    #[test]
    fn edge_drops_matches_the_c_expression(
        v in any::<u64>(),
        back in 0u64..=100_000_000_000,
        f in any::<u32>(),
        r in rate(),
    ) {
        let ts = v.saturating_sub(back);
        prop_assert_eq!(drops(ts, f, v, r), master_drops(ts, f, v, r));
    }

    #[test]
    fn edge_frames_matches_the_c_expression(
        cut in 0u64..=100_000_000_000_000,
        r in 0usize..=U32_MAX,
    ) {
        prop_assert_eq!(frames(cut, r), master_frames(cut, r));
    }

    #[test]
    fn edge_zero_rate_always_drops(ts in any::<u64>(), f in any::<u32>(), v in 1u64..) {
        // prepare_audio() asks only for a packet that starts before video
        let ts = ts % v;
        prop_assert_eq!(duration(f, 0), 0);
        prop_assert!(drops(ts, f, v, 0));
    }
}
