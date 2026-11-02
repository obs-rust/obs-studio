//! Tier 1: `obs_output::encoder_audio_start_frames`, the two sample-rate
//! conversions of the encoder's audio start.
//!
//! The `steps_*` cases port
//! `test/cmocka/test_encoder_audio_start_frames_steps.c` 1:1: its 7 cases
//! under the same names, in the same order, with the same values and
//! expectations, the "characterized, not endorsed" ones included. Like the
//! C, each case is also checked against the expressions `obs-encoder.c` had
//! before the move ([`master_offset`], [`master_duration`]), here with
//! `obs_util`'s `mul_div64`, the C's `util_mul_div64()` on each platform.
//! The packet cases run the port's length and `encoder_audio_start`'s
//! `packet_start`, as `buffer_audio()` does ([`packet`]); the size cases run
//! `calc_offset_size()`'s three steps ([`offset_size`]).
//!
//! The `edge_*` cases are new: the zero sample rate (the intentional
//! difference: the length is 0 where the C divides by zero, so a packet
//! waits when it starts at or before the video and starts from the buffer
//! otherwise, never truncated; the offset is 0 as in the C), and proptests
//! of both functions over full-range inputs against the expressions.

use obs_output::encoder_audio_start::{self as rules, PacketStart};
use obs_output::encoder_audio_start_frames::{duration, offset};
use obs_util::util_uint64::mul_div64;
use proptest::prelude::*;

const NS: u64 = 1_000_000_000;
/// One 1024-frame packet at 48 kHz: `util_mul_div64(1024, NS, 48000)`.
const PKT: u64 = 21_333_333;

/// The offset in frames as `calc_offset_size()` wrote it before the move.
fn master_offset(offset_ns: u64, samplerate: u32) -> u64 {
    mul_div64(offset_ns, u64::from(samplerate), NS)
}

/// The length as `buffer_audio()` wrote it before the move. `samplerate`
/// must not be 0.
fn master_duration(frames: u32, samplerate: u32) -> u64 {
    mul_div64(u64::from(frames), NS, u64::from(samplerate))
}

#[track_caller]
fn check_offset(offset_ns: u64, samplerate: u32, expect: u64) {
    let got = offset(offset_ns, samplerate);
    assert_eq!(got, expect, "offset({offset_ns}, {samplerate})");
    assert_eq!(
        got,
        master_offset(offset_ns, samplerate),
        "offset({offset_ns}, {samplerate}) against the C"
    );
}

#[track_caller]
fn check_duration(frames: u32, samplerate: u32, expect: u64) {
    let got = duration(frames, samplerate);
    assert_eq!(got, expect, "duration({frames}, {samplerate})");
    assert_eq!(
        got,
        master_duration(frames, samplerate),
        "duration({frames}, {samplerate}) against the C"
    );
}

/// `buffer_audio()` once the paired video has started: the length, then the
/// plan, as it runs them.
fn packet(timestamp: u64, frames: u32, samplerate: u32, v_start_ts: u64) -> PacketStart {
    rules::packet_start(timestamp, duration(frames, samplerate), v_start_ts)
}

/// `calc_offset_size()` as it runs: the offset, in frames, in bytes.
fn offset_size(v_start_ts: u64, a_start_ts: u64, samplerate: u32, blocksize: usize) -> usize {
    let n = rules::offset_ns(v_start_ts, a_start_ts);
    rules::offset_size(offset(n, samplerate), blocksize)
}

/// `q` is `a * b / c` rounded down.
fn is_floor(q: u64, a: u64, b: u64, c: u64) -> bool {
    let ab = u128::from(a) * u128::from(b);
    let qc = u128::from(q) * u128::from(c);
    qc <= ab && qc + u128::from(c) > ab
}

fn waits(p: PacketStart) -> bool {
    p == PacketStart::Wait
}

// ---------------------------------------------------------------------------
// duration()

#[test]
fn steps_duration_common_rates() {
    // 1024 frames
    check_duration(1024, 48000, PKT);
    check_duration(1024, 44100, 23_219_954);
    check_duration(1024, 96000, 10_666_666);
    // one frame rounds down
    check_duration(1, 48000, 20833);
    check_duration(1, 44100, 22675);
    check_duration(1, 96000, 10416);
    // exact: 10 ms
    check_duration(480, 48000, 10_000_000);
    check_duration(441, 44100, 10_000_000);
    // no frames
    check_duration(0, 48000, 0);
}

#[test]
fn steps_duration_edges() {
    // the most frames
    check_duration(u32::MAX, 48000, 89_478_485_312_500);
    check_duration(u32::MAX, 44100, 97_391_548_639_455);
    check_duration(u32::MAX, 1_000_000_000, u64::from(u32::MAX));
    // 1 Hz: a frame is a second; u32::MAX seconds still fit
    check_duration(1, 1, NS);
    check_duration(u32::MAX, 1, 4_294_967_295_000_000_000);
    // over 1 GHz a frame lasts 0 ns (characterized, not endorsed)
    check_duration(1, 1_000_000_000, 1);
    check_duration(1, 1_000_000_001, 0);
    check_duration(2, 2_000_000_000, 1);
    check_duration(1024, 2_000_000_000, 512);
    // the highest rate
    check_duration(1, u32::MAX, 0);
    check_duration(u32::MAX, u32::MAX, NS);
    check_duration(u32::MAX - 1, u32::MAX, NS - 1);
}

#[test]
fn steps_duration_drives_the_plan() {
    let v = 5 * NS;

    // a packet that ends exactly at the video start waits
    assert!(waits(packet(v - PKT, 1024, 48000, v)));
    // one ns later it starts, truncated
    assert_eq!(
        packet(v - PKT + 1, 1024, 48000, v),
        PacketStart::Start {
            truncate: true,
            clear: true,
            from_buffer: false
        }
    );
    // the length rounds down: 1 frame at 48 kHz is 20833 ns
    assert!(waits(packet(v - 20833, 1, 48000, v)));
    assert!(!waits(packet(v - 20832, 1, 48000, v)));
    // 44.1 kHz
    assert!(waits(packet(v - 23_219_954, 1024, 44100, v)));
    assert!(!waits(packet(v - 23_219_953, 1024, 44100, v)));
    // a frame of 0 ns just before the video waits (characterized, not
    // endorsed)
    assert!(waits(packet(v - 1, 1, 2_000_000_000, v)));
    // a packet after the video start starts from the buffer
    assert_eq!(
        packet(v + 1, 1024, 48000, v),
        PacketStart::Start {
            truncate: false,
            clear: false,
            from_buffer: true
        }
    );
}

// ---------------------------------------------------------------------------
// offset()

#[test]
fn steps_offset_common_rates() {
    // a second
    check_offset(NS, 48000, 48000);
    check_offset(NS, 1, 1);
    check_offset(NS, u32::MAX, u64::from(u32::MAX));
    // uneven offsets round down
    check_offset(PKT, 48000, 1023);
    check_offset(23_219_954, 44100, 1023);
    check_offset(999_999, 48000, 47);
    // less than a frame is none; a frame plus a ns is one
    check_offset(20833, 48000, 0);
    check_offset(20834, 48000, 1);
    check_offset(1, 48000, 0);
    check_offset(NS - 1, 1, 0);
    check_offset(1, u32::MAX, 4);
    // none
    check_offset(0, 48000, 0);
    check_offset(0, u32::MAX, 0);
}

#[test]
fn steps_offset_wide_products() {
    // offset times rate is past 2^64; the quotient still fits
    check_offset(u64::MAX, 48000, 885_443_715_538_058);
    check_offset(u64::MAX, 44100, 813_501_413_650_591);
    check_offset(u64::MAX, 96000, 1_770_887_431_076_116);
    check_offset(384_307_168_202_283, 48000, 18_446_744_073);
    check_offset(
        4_000_000_000_000_000_000,
        u32::MAX,
        17_179_869_180_000_000_000,
    );
    check_offset(u64::MAX, 1_000_000_000, u64::MAX);
    check_offset(u64::MAX, 1, 18_446_744_073);
}

#[test]
fn steps_offset_drives_the_size() {
    let v = 5 * NS;

    // a packet a whole packet before the video drops 1023 frames of 4 or
    // 8 bytes, not 1024: the offset rounds down
    assert_eq!(offset_size(v, v - PKT, 48000, 4), 1023 * 4);
    assert_eq!(offset_size(v, v - PKT, 48000, 8), 1023 * 8);
    // one ns before the video drops nothing
    assert_eq!(offset_size(v, v - 1, 48000, 8), 0);
    // a second before drops a second
    assert_eq!(offset_size(v, v - NS, 44100, 8), 44100 * 8);
}

// ---------------------------------------------------------------------------
// both, over a grid, against the expressions before the move

#[test]
fn steps_grid() {
    const RATES: [u32; 19] = [
        1,
        2,
        8000,
        11025,
        16000,
        22050,
        32000,
        44100,
        48000,
        88200,
        96000,
        192_000,
        384_000,
        999_999_999,
        1_000_000_000,
        1_000_000_001,
        2_000_000_000,
        u32::MAX - 1,
        u32::MAX,
    ];
    const FRAMES: [u32; 14] = [
        0,
        1,
        2,
        441,
        480,
        960,
        1023,
        1024,
        1025,
        2048,
        65535,
        65536,
        u32::MAX - 1,
        u32::MAX,
    ];
    const OFFSETS: [u64; 14] = [
        0,
        1,
        999,
        20833,
        20834,
        PKT,
        NS - 1,
        NS,
        NS + 1,
        60 * NS,
        3600 * NS,
        4_294_967_295,
        4_294_967_296,
        1 << 40,
    ];

    for r in RATES {
        for f in FRAMES {
            let d = duration(f, r);
            check_duration(f, r, d);
            assert!(is_floor(d, u64::from(f), NS, u64::from(r)), "{f} {r}");
        }
        for o in OFFSETS {
            let n = offset(o, r);
            check_offset(o, r, n);
            assert!(is_floor(n, o, u64::from(r), NS), "{o} {r}");
        }
    }
}

// ---------------------------------------------------------------------------
// new cases

/// A zero rate: the length is 0, where the C divides by zero (the
/// intentional difference). The offset is 0, as in the C.
#[test]
fn edge_zero_rate() {
    for f in [0, 1, 1024, u32::MAX] {
        assert_eq!(duration(f, 0), 0, "duration({f}, 0)");
    }
    for o in [0, 1, PKT, NS, u64::MAX] {
        check_offset(o, 0, 0);
    }
}

/// With a zero rate the packet ends at its own timestamp: it waits when it
/// starts at or before the video, and starts from the buffer otherwise. It
/// is never truncated, and a truncation would drop nothing.
#[test]
fn edge_zero_rate_plan() {
    let v = 5 * NS;
    for ts in [0, 1, v - PKT, v - 1, v] {
        for f in [0, 1, 1024, u32::MAX] {
            assert!(waits(packet(ts, f, 0, v)), "packet({ts}, {f}, 0)");
        }
    }
    for ts in [v + 1, v + PKT, u64::MAX] {
        assert_eq!(
            packet(ts, 1024, 0, v),
            PacketStart::Start {
                truncate: false,
                clear: false,
                from_buffer: true
            },
            "packet({ts}, 1024, 0)"
        );
    }
    assert_eq!(offset_size(v, 0, 0, 8), 0);
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
    fn edge_duration_matches_the_c_expression(f in any::<u32>(), r in rate()) {
        prop_assert_eq!(duration(f, r), master_duration(f, r));
        prop_assert!(is_floor(duration(f, r), u64::from(f), NS, u64::from(r)));
    }

    #[test]
    fn edge_offset_matches_the_c_expression(o in any::<u64>(), r in 0u32..=1_000_000_000) {
        // at or below 10^9 Hz the quotient fits on every platform
        prop_assert_eq!(offset(o, r), master_offset(o, r));
        prop_assert!(is_floor(offset(o, r), o, u64::from(r), NS));
    }

    #[test]
    fn edge_plan_matches_the_c(
        v in 1u64..,
        back in 0u64..=10_000_000_000,
        f in any::<u32>(),
        r in rate(),
    ) {
        let ts = v.saturating_sub(back);
        prop_assert_eq!(
            packet(ts, f, r, v),
            rules::packet_start(ts, master_duration(f, r), v)
        );
    }
}
