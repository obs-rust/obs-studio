//! Tier 1: `obs_output::encoder_packet_ts`, the packet timestamp decisions
//! of `obs-encoder.c`.
//!
//! The `steps_*` cases port `test/cmocka/test_encoder_packet_ts_steps.c`
//! 1:1: its 10 cases under the same names, in the same order, with the same
//! values and expectations, the "characterized, not endorsed" ones
//! included. The C calls the hidden `obs_encoder_packet_ts_*` functions;
//! here the safe core is called directly, and `find` gets the C ABI's
//! `encoder_packet_time` (it implements `PacketTime`).
//!
//! The `edge_*` cases are new: proptests over full-range values against
//! models written from the C's expressions in `i128` (the unsigned sums
//! reduced modulo 2^64 and stored as `int64_t`), the newest entry over
//! random lists, and the intentional differences where C is undefined.

#![allow(
    clippy::as_conversions,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]

use obs_output::encoder_packet_ts::{
    MICROSECOND_DEN, NOT_FOUND, Timebase, dts_usec, find, paused, system, timebase,
};
use obs_output::ffi::output_delay::encoder_packet_time;
use proptest::prelude::*;

const NTSC_NUM: u32 = 1001;
const NTSC_DEN: u32 = 30000;
/// An encoder start time, in ns.
const T0_NS: u64 = 5_000_000_123;

// ---------------------------------------------------------------------------
// helpers

fn check_timebase(what: &str, num: u32, den: u32, divisor: u32, want_num: i32, want_den: i32) {
    assert_eq!(
        timebase(num, den, divisor),
        Timebase {
            num: want_num,
            den: want_den
        },
        "{what}"
    );
}

fn times(pts: &[i64]) -> Vec<encoder_packet_time> {
    pts.iter()
        .map(|&pts| encoder_packet_time {
            pts,
            ..encoder_packet_time::default()
        })
        .collect()
}

/// `find`'s result as the C ABI returns it.
fn idx(found: Option<usize>) -> usize {
    found.unwrap_or(NOT_FOUND)
}

/// A value reduced modulo 2^64 and stored as `int64_t`.
fn store64(v: i128) -> i64 {
    v.rem_euclid(1_i128 << 64) as u64 as i64
}

// ---------------------------------------------------------------------------
// the packet timebase

#[test]
fn steps_timebase_from_the_encoder() {
    check_timebase("30 fps", 1, 30, 1, 1, 30);
    check_timebase("29.97 fps", NTSC_NUM, NTSC_DEN, 1, 1001, 30000);
    check_timebase("60 fps, divisor 2", 1, 60, 2, 2, 60);
    check_timebase("29.97 fps, divisor 3", NTSC_NUM, NTSC_DEN, 3, 3003, 30000);
    check_timebase("48 kHz audio", 1, 48000, 1, 1, 48000);
}

#[test]
fn steps_timebase_wraps_at_the_limits() {
    check_timebase(
        "product beyond INT32_MAX",
        1001,
        30000,
        3_000_000,
        -1_291_967_296,
        30000,
    );
    check_timebase("product wraps to 0", 65536, 30, 65536, 0, 30);
    check_timebase("den beyond INT32_MAX", 1, 0x8000_0000, 1, 1, i32::MIN);
    check_timebase("divisor 0", 1, 30, 0, 0, 30);
}

// ---------------------------------------------------------------------------
// packet_dts_usec()

#[test]
fn steps_dts_usec_of_common_rates() {
    let ntsc_den = NTSC_DEN as i32;
    let ntsc_num = i64::from(NTSC_NUM);
    assert_eq!(dts_usec(30, 30), 1_000_000, "30 fps, 1 s");
    assert_eq!(dts_usec(ntsc_num, ntsc_den), 33366, "29.97 fps, one frame");
    assert_eq!(
        dts_usec(30 * ntsc_num, ntsc_den),
        1_001_000,
        "29.97 fps, 30 frames"
    );
    assert_eq!(dts_usec(3, 25), 120_000, "25 fps, frame 3");
    assert_eq!(dts_usec(1024, 48000), 21333, "48 kHz, one AAC frame");
    assert_eq!(dts_usec(0, 30), 0, "dts 0");
}

#[test]
fn steps_dts_usec_negative_truncates_toward_zero() {
    let ntsc_den = NTSC_DEN as i32;
    let ntsc_num = i64::from(NTSC_NUM);
    assert_eq!(dts_usec(-ntsc_num, ntsc_den), -33366, "29.97 fps, -1 frame");
    assert_eq!(
        dts_usec(-2 * ntsc_num, ntsc_den),
        -66733,
        "29.97 fps, -2 frames"
    );
    assert_eq!(dts_usec(-1, 48000), -20, "48 kHz, -1 sample");
}

#[test]
fn steps_dts_usec_at_the_limits() {
    let max = i64::MAX / MICROSECOND_DEN;
    let min = i64::MIN / MICROSECOND_DEN;
    assert_eq!(dts_usec(5, -1), -5_000_000, "den -1");
    assert_eq!(dts_usec(max, i32::MIN), -4_294_967_295, "INT32_MIN den");
    assert_eq!(
        dts_usec(max, 1),
        max * MICROSECOND_DEN,
        "INT64_MAX / 1000000"
    );
    assert_eq!(
        dts_usec(min, 1),
        min * MICROSECOND_DEN,
        "INT64_MIN / 1000000"
    );
}

// ---------------------------------------------------------------------------
// dts_usec and sys_dts_usec

#[test]
fn steps_system_time_from_the_first_packet() {
    let ntsc_den = NTSC_DEN as i32;
    let ntsc_num = i64::from(NTSC_NUM);
    let offset = dts_usec(-2 * ntsc_num, ntsc_den);

    assert_eq!(system(T0_NS, offset, offset), 5_000_000, "first packet");
    assert_eq!(
        system(T0_NS, dts_usec(-ntsc_num, ntsc_den), offset),
        5_000_000 + 33367,
        "second packet"
    );
    assert_eq!(system(T0_NS, 0, offset), 5_000_000 + 66733, "dts 0");
    assert_eq!(system(0, 21333, 0), 21333, "start 0, first dts 0");
    assert_eq!(system(999, 0, 0), 0, "start below 1 us");
}

#[test]
fn steps_system_time_before_the_start() {
    assert_eq!(system(T0_NS, -10, 0), 4_999_990, "10 us before");
    assert_eq!(system(0, 0, 5), -5, "before 0");
}

#[test]
fn steps_system_time_wraps_at_the_limits() {
    assert_eq!(
        system(u64::MAX, 0, 0),
        18_446_744_073_709_551,
        "start beyond INT64_MAX ns"
    );
    assert_eq!(system(1000, i64::MAX, 0), i64::MIN, "sum above INT64_MAX");
    assert_eq!(system(0, 0, i64::MIN), i64::MIN, "offset INT64_MIN");
}

#[test]
fn steps_paused_adds_the_pause_offset() {
    assert_eq!(paused(5_000_000, 0), 5_000_000, "no pause");
    assert_eq!(paused(5_000_000, 2_500_000_000), 7_500_000, "2.5 s paused");
    assert_eq!(paused(5_000_000, 999), 5_000_000, "below 1 us");
    assert_eq!(paused(-5, 7000), 2, "negative time");
    assert_eq!(paused(i64::MAX, 1000), i64::MIN, "above INT64_MAX");
    assert_eq!(
        paused(0, u64::MAX),
        18_446_744_073_709_551,
        "largest offset"
    );
}

// ---------------------------------------------------------------------------
// the timing entry

#[test]
fn steps_find_the_newest_entry() {
    let t = times(&[0, 1, 2, 1, 3]);
    let empty: [encoder_packet_time; 0] = [];

    assert_eq!(idx(find(&empty, 0)), NOT_FOUND, "empty");
    assert_eq!(idx(find(&t[..1], 0)), 0, "one entry");
    assert_eq!(idx(find(&t, 0)), 0, "first entry");
    assert_eq!(idx(find(&t, 3)), 4, "last entry");
    assert_eq!(idx(find(&t, 1)), 3, "two entries, the newest");
    assert_eq!(idx(find(&t[..3], 1)), 1, "only the first num");
    assert_eq!(idx(find(&t, 7)), NOT_FOUND, "no entry");
    assert_eq!(idx(find(&t, -1)), NOT_FOUND, "negative pts");
}

// ---------------------------------------------------------------------------
// edges

/// Intentional differences, unreachable in libobs (undefined in C).
#[test]
fn edge_dts_usec_where_c_is_undefined() {
    assert_eq!(dts_usec(123, 0), 0, "timebase_den 0");
    assert_eq!(dts_usec(i64::MIN, 0), 0, "timebase_den 0, INT64_MIN");
    assert_eq!(
        dts_usec(i64::MAX, 1),
        i64::MAX.wrapping_mul(MICROSECOND_DEN),
        "dts * 1000000 wraps"
    );
    assert_eq!(
        dts_usec(i64::MIN / MICROSECOND_DEN - 1, 7),
        (i64::MIN / MICROSECOND_DEN - 1).wrapping_mul(MICROSECOND_DEN) / 7,
        "below INT64_MIN wraps"
    );
    // A DTS whose product wraps to exactly INT64_MIN, divided by -1.
    let dts = 144_115_188_075_855_872_i64;
    assert_eq!(dts.wrapping_mul(MICROSECOND_DEN), i64::MIN);
    assert_eq!(dts_usec(dts, -1), i64::MIN, "INT64_MIN / -1");
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4096))]

    /// Any timebase and divisor.
    #[test]
    fn edge_timebase_matches_the_model(num in any::<u32>(), den in any::<u32>(), divisor in any::<u32>()) {
        let product = (u64::from(num) * u64::from(divisor)) % (1 << 32);
        prop_assert_eq!(
            timebase(num, den, divisor),
            Timebase { num: product as u32 as i32, den: den as i32 }
        );
    }

    /// Any DTS and timebase_den where C is defined.
    #[test]
    fn edge_dts_usec_matches_the_model(
        dts in prop_oneof![
            i64::MIN / MICROSECOND_DEN..=i64::MAX / MICROSECOND_DEN,
            -10_000_000_i64..10_000_000,
        ],
        den in prop_oneof![any::<i32>(), 1_i32..200_000, -200_000_i32..0],
    ) {
        prop_assume!(den != 0);
        let want = i128::from(dts) * i128::from(MICROSECOND_DEN) / i128::from(den);
        prop_assert_eq!(i128::from(dts_usec(dts, den)), want);
    }

    /// Any start time, DTS and offset.
    #[test]
    fn edge_system_matches_the_model(start in any::<u64>(), dts in any::<i64>(), offset in any::<i64>()) {
        let want = store64(i128::from(start / 1000) + i128::from(dts) - i128::from(offset));
        prop_assert_eq!(system(start, dts, offset), want);
    }

    /// Any time and pause offset.
    #[test]
    fn edge_paused_matches_the_model(sys in any::<i64>(), offset in any::<u64>()) {
        let want = store64(i128::from(sys) + i128::from(offset / 1000));
        prop_assert_eq!(paused(sys, offset), want);
    }

    /// Any list of PTS values: the newest equal entry, or none.
    #[test]
    fn edge_find_the_newest(list in prop::collection::vec(-3_i64..4, 0..12), pts in -4_i64..5) {
        let t = times(&list);
        let want = list.iter().rposition(|&p| p == pts);
        prop_assert_eq!(find(&t, pts), want);
    }

    /// A realistic stream: packets at a fixed frame duration, the first
    /// one negative (B-frames), placed at the start time plus their
    /// distance from the first; a pause shifts sys_dts_usec only.
    #[test]
    fn edge_realistic_stream(
        fps_num in prop_oneof![Just(30_u32), Just(30000), Just(60), Just(25), Just(48000)],
        fps_den in prop_oneof![Just(1_u32), Just(1001)],
        start in 0_u64..1_000_000_000_000_000,
        delay in 0_i64..3,
        pause in 0_u64..10_000_000_000,
    ) {
        let tb = timebase(fps_den, fps_num, 1);
        let first = -delay * i64::from(tb.num);
        let offset = dts_usec(first, tb.den);
        let mut last = i64::MIN;
        for frame in 0..120_i64 {
            let dts = first + frame * i64::from(tb.num);
            let at = system(start, dts_usec(dts, tb.den), offset);
            prop_assert!(at >= last);
            prop_assert!(at >= (start / 1000) as i64);
            let exact = i128::from(frame) * i128::from(tb.num) * 1_000_000 / i128::from(tb.den);
            prop_assert!((i128::from(at - (start / 1000) as i64) - exact).abs() <= 1);
            prop_assert_eq!(paused(at, pause), at + (pause / 1000) as i64);
            last = at;
        }
    }
}
