//! Tier 1: `obs_source::deinterlace_pacing`, the 2x deinterlace frame
//! pacing.
//!
//! The `steps_*` cases port `test/cmocka/test_deinterlace_pacing_steps.c`
//! 1:1, under the same case names, in the same order, with the same inputs
//! and expectations, the model of `deinterlace_get_closest_frames()` of the
//! `seq_*` cases included.
//!
//! The `edge_*` cases are new: every function over the special values of
//! the full `u64` range and the thresholds +-1 (no panic, and the result of
//! a model of the C's `uint64_t` arithmetic written in `i128`, with the
//! modulo explicit), and proptests of the same over random values.

use obs_source::deinterlace_pacing::{
    FRAME_TOLERANCE, TS_JUMP_THRESHOLD, TWOX_TOLERANCE, buffered_offset, half_duration, hold,
    reanchor, wait_for_next, waiting_frame_ts,
};
use proptest::prelude::*;

const MS: u64 = 1_000_000;
const TWOX: u64 = TWOX_TOLERANCE;
const FRAME_TOL: u64 = FRAME_TOLERANCE;
const JUMP: u64 = TS_JUMP_THRESHOLD;
/// The canvas clock (`obs->video.video_time`).
const VT: u64 = 1_000_000_000_000;
/// A frame timestamp on another clock, behind `VT`.
const TS: u64 = 5_000_000_000;
/// 25 interlaced frames per second (50i): 40 ms frames, 20 ms fields.
const FRAME_50I: u64 = 40 * MS;
const FIELD_50I: u32 = 20_000_000;
const FIELD_50I_NS: u64 = 20 * MS;
/// 60 and 30 fps canvas intervals, as `obs->video.video_frame_interval_ns`.
const TICK_60: u64 = 16_666_666;
const TICK_30: u64 = 33_333_333;
const INT64_MAX: u64 = 0x7fff_ffff_ffff_ffff;
const TWO_32: u64 = 1 << 32;

#[track_caller]
fn check_hold(frame_ts: u64, offset: u64, half: u32, sys_time: u64, expected: bool) {
    assert_eq!(
        hold(frame_ts, offset, half, sys_time),
        expected,
        "hold({frame_ts}, {offset}, {half}, {sys_time})"
    );
}

#[track_caller]
fn check_wait(ts: u64, next_ts: u64, offset: u64, sys_time: u64, expected: bool) {
    assert_eq!(
        wait_for_next(ts, next_ts, offset, sys_time),
        expected,
        "wait_for_next({ts}, {next_ts}, {offset}, {sys_time})"
    );
}

#[track_caller]
fn check_waiting_ts(ts: u64, next_ts: u64, expected: u64) {
    assert_eq!(
        waiting_frame_ts(ts, next_ts),
        expected,
        "waiting_frame_ts({ts}, {next_ts})"
    );
}

#[track_caller]
fn check_half(ts: u64, prev_ts: u64, expected: u32) {
    assert_eq!(
        half_duration(ts, prev_ts),
        expected,
        "half_duration({ts}, {prev_ts})"
    );
}

#[track_caller]
fn check_buffered(video_time: u64, last_frame_ts: u64, expected: u64) {
    assert_eq!(
        buffered_offset(video_time, last_frame_ts),
        expected,
        "buffered_offset({video_time}, {last_frame_ts})"
    );
}

#[track_caller]
fn check_reanchor(
    offset: u64,
    video_time: u64,
    frame_ts: u64,
    half: u32,
    interval: u64,
    expected: u64,
) {
    assert_eq!(
        reanchor(offset, video_time, frame_ts, half, interval),
        expected,
        "reanchor({offset}, {video_time}, {frame_ts}, {half}, {interval})"
    );
}

// --------------------------------------------------------------------------
// hold

#[test]
fn steps_hold_until_frame_end() {
    let offset = VT - TS;
    let end = TS + offset + 2 * FIELD_50I_NS - TWOX;

    check_hold(TS, offset, FIELD_50I, VT, true);
    check_hold(TS, offset, FIELD_50I, VT + 20 * MS, true);
    check_hold(TS, offset, FIELD_50I, end - 1, true);
    check_hold(TS, offset, FIELD_50I, end, false);
    check_hold(TS, offset, FIELD_50I, end + 1, false);
    check_hold(TS, offset, FIELD_50I, end + FRAME_50I, false);
    check_hold(TS, offset, FIELD_50I, VT - 10 * MS, true);
}

#[test]
fn steps_hold_at_jump_threshold() {
    let offset = VT - TS;
    let end = TS + offset + 2 * FIELD_50I_NS - TWOX;

    check_hold(TS, offset, FIELD_50I, end - (JUMP - 1), true);
    check_hold(TS, offset, FIELD_50I, end - JUMP, false);
    check_hold(TS, offset, FIELD_50I, end - (JUMP + 1), false);
    check_hold(TS, offset, FIELD_50I, 0, false);
}

#[test]
fn steps_hold_half_duration_widened() {
    let offset = VT - TS;
    let end = VT + 2 * u64::from(u32::MAX) - TWOX;

    check_hold(TS, offset, u32::MAX, end - 1, true);
    check_hold(TS, offset, u32::MAX, end, false);
    check_hold(TS, offset, u32::MAX, end - JUMP, false);
    check_hold(TS, offset, 0, VT - TWOX - 1, true);
    check_hold(TS, offset, 0, VT - TWOX, false);
}

/// Characterized, not endorsed: the end wraps.
#[test]
fn steps_hold_end_wraps() {
    check_hold(u64::MAX, 10 * MS + 1, FIELD_50I, 0, true);
    check_hold(u64::MAX, 10 * MS + 1, FIELD_50I, 48 * MS, true);
    check_hold(u64::MAX, 10 * MS + 1, FIELD_50I, 49 * MS, false);
    check_hold(u64::MAX, 10 * MS + 1, FIELD_50I, VT, false);
    check_hold(0, 0, 0, 0, false);
    check_hold(0, 0, 0, u64::MAX - TWOX, true);
    check_hold(0, 0, 0, u64::MAX - TWOX + 1, false);
}

// --------------------------------------------------------------------------
// wait_for_next / waiting_frame_ts

#[test]
fn steps_wait_until_first_frame_ends() {
    let offset = VT - TS;
    let end = VT + FRAME_50I;

    check_wait(TS, TS + FRAME_50I, offset, VT, true);
    check_wait(TS, TS + FRAME_50I, offset, end - 1, true);
    check_wait(TS, TS + FRAME_50I, offset, end, false);
    check_wait(TS, TS + FRAME_50I, offset, end + 1, false);
    check_wait(TS, TS + FRAME_50I, offset, 0, true);
    check_wait(TS, TS, offset, VT - 1, true);
    check_wait(TS, TS, offset, VT, false);

    check_waiting_ts(TS, TS + FRAME_50I, TS - FRAME_50I);
    check_waiting_ts(TS, TS, TS);
    check_half(TS, waiting_frame_ts(TS, TS + FRAME_50I), FIELD_50I);
}

/// Characterized, not endorsed: a timestamp going backwards wraps.
#[test]
fn steps_wait_timestamp_backwards_wraps() {
    let offset = VT - TS;

    check_wait(TS, TS - FRAME_50I, offset, VT - FRAME_50I - 1, true);
    check_wait(TS, TS - FRAME_50I, offset, VT - FRAME_50I, false);
    check_wait(TS, TS - FRAME_50I, offset, VT, false);
    check_waiting_ts(TS, TS - FRAME_50I, TS + FRAME_50I);
    check_wait(u64::MAX - 10 * MS, u64::MAX, 20 * MS, 9 * MS, true);
    check_wait(u64::MAX - 10 * MS, u64::MAX, 20 * MS, 20 * MS - 2, true);
    check_wait(u64::MAX - 10 * MS, u64::MAX, 20 * MS, 20 * MS - 1, false);
    check_waiting_ts(10 * MS, 50 * MS, u64::MAX - 30 * MS + 1);
    check_waiting_ts(0, u64::MAX, 1);
}

// --------------------------------------------------------------------------
// half_duration

#[test]
fn steps_half_duration_of_a_frame() {
    check_half(TS + FRAME_50I, TS, FIELD_50I);
    check_half(TS + 33_366_667, TS, 16_683_333);
    check_half(TS + 1, TS, 0);
    check_half(TS + 3, TS, 1);
    check_half(TS, TS, 0);
    check_half(TS + 2 * u64::from(u32::MAX) + 1, TS, u32::MAX);
}

/// Characterized, not endorsed: truncated to 32 bits.
#[test]
fn steps_half_duration_over_2_32_truncated() {
    check_half(TS + 2 * TWO_32, TS, 0);
    check_half(TS + 2 * TWO_32 + 6, TS, 3);
    check_half(TS + 10_000_000_000, TS, 705_032_704);
    check_half(u64::MAX, 0, u32::MAX);
}

/// Characterized, not endorsed: a timestamp going backwards wraps.
#[test]
fn steps_half_duration_timestamp_backwards_wraps() {
    check_half(TS - FRAME_50I, TS, 4_274_967_296);
    check_half(TS - 1, TS, u32::MAX);
    check_half(0, u64::MAX, 0);
}

/// Characterized, not endorsed: the first frame's half duration is taken
/// against a `deinterlace_frame_ts` of 0, or of the last frame shown.
#[test]
fn steps_half_duration_first_frame() {
    check_half(VT, 0, 1_783_793_664);
    check_half(TS, 0, 2_500_000_000);
    check_half(2 * (100 * TWO_32 + 30 * MS), 0, 30_000_000);
    check_half(0, 0, 0);
    check_half(TS + 10_000_000_000, TS, 705_032_704);
}

/// Characterized, not endorsed: what the first frame's half duration does
/// next.
#[test]
fn steps_first_frame_half_duration_hold() {
    let ts = VT - 5 * MS;
    let half = half_duration(ts, 0);
    assert_eq!(half, 1_781_293_664);
    let offset = reanchor(0, VT, ts, half, TICK_60);
    assert_eq!(offset, 5 * MS);
    check_hold(ts, offset, half, VT + TICK_60, false);

    let ts = 2 * (100 * TWO_32 + 30 * MS);
    let half = half_duration(ts, 0);
    let offset = reanchor(0, VT, ts, half, TICK_60);
    assert_eq!(offset, VT - ts);
    check_hold(ts, offset, half, VT + 3 * TICK_60, true);
    check_hold(ts, offset, half, VT + 59 * MS - 1, true);
    check_hold(ts, offset, half, VT + 59 * MS, false);
}

// --------------------------------------------------------------------------
// buffered_offset

#[test]
fn steps_buffered_offset() {
    check_buffered(VT, TS, VT - TS + FRAME_TOL + TWOX);
    check_buffered(VT, TS, VT - TS + 3 * MS);
    check_buffered(VT, VT, 3 * MS);
    check_buffered(0, 0, 3 * MS);
}

/// Characterized, not endorsed: the offset wraps.
#[test]
fn steps_buffered_offset_wraps() {
    check_buffered(TS, VT, TS.wrapping_sub(VT) + 3 * MS);
    check_buffered(TS, TS + 3 * MS, 0);
    check_buffered(TS, TS + 3 * MS + 1, u64::MAX);
    check_buffered(u64::MAX, 0, 3 * MS - 1);
    check_buffered(0, u64::MAX, 3 * MS + 1);
}

// --------------------------------------------------------------------------
// reanchor

#[test]
fn steps_reanchor_without_offset() {
    check_reanchor(0, VT, TS, FIELD_50I, TICK_60, VT - TS);
    check_reanchor(0, VT + 5 * MS, TS, FIELD_50I, TICK_60, VT + 5 * MS - TS);
    check_reanchor(0, VT, VT, FIELD_50I, TICK_60, 0);
}

#[test]
fn steps_reanchor_early_frame() {
    let offset = VT - TS;

    check_reanchor(offset, VT - 1, TS, FIELD_50I, TICK_60, offset - 1);
    check_reanchor(
        offset,
        VT - 30 * MS,
        TS,
        FIELD_50I,
        TICK_60,
        offset - 30 * MS,
    );
    check_reanchor(offset, VT, TS, FIELD_50I, TICK_60, offset);
    check_reanchor(offset, VT + 1, TS, FIELD_50I, TICK_60, offset);
    check_reanchor(offset, VT + 10 * MS, TS, FIELD_50I, TICK_60, offset);
}

#[test]
fn steps_reanchor_late_by_field_or_tick() {
    let offset = VT - TS;
    let f = FIELD_50I_NS;
    let tick_60_half = u32::try_from(TICK_60).unwrap();
    let twox_half = u32::try_from(TWOX).unwrap();

    check_reanchor(offset, VT + f - TWOX - 1, TS, FIELD_50I, TICK_60, offset);
    check_reanchor(
        offset,
        VT + f - TWOX,
        TS,
        FIELD_50I,
        TICK_60,
        offset + f - TWOX,
    );
    check_reanchor(
        offset,
        VT + f - TWOX + 1,
        TS,
        FIELD_50I,
        TICK_60,
        offset + f - TWOX + 1,
    );
    check_reanchor(offset, VT + f - TWOX, TS, FIELD_50I, TICK_30, offset);
    check_reanchor(
        offset,
        VT + TICK_30 - TWOX - 1,
        TS,
        FIELD_50I,
        TICK_30,
        offset,
    );
    check_reanchor(
        offset,
        VT + TICK_30 - TWOX,
        TS,
        FIELD_50I,
        TICK_30,
        offset + TICK_30 - TWOX,
    );
    check_reanchor(
        offset,
        VT + TICK_60 - TWOX - 1,
        TS,
        tick_60_half,
        TICK_60,
        offset,
    );
    check_reanchor(
        offset,
        VT + TICK_60 - TWOX,
        TS,
        tick_60_half,
        TICK_60,
        offset + TICK_60 - TWOX,
    );
    check_reanchor(offset, VT, TS, 0, 0, offset);
    check_reanchor(offset, VT + 1, TS, 0, 0, offset + 1);
    check_reanchor(offset, VT + 1, TS, twox_half, TWOX, offset + 1);
    let long = 5_000_000_000;
    check_reanchor(offset, VT + long - TWOX - 1, TS, FIELD_50I, long, offset);
    check_reanchor(
        offset,
        VT + long - TWOX,
        TS,
        FIELD_50I,
        long,
        offset + long - TWOX,
    );
}

/// Characterized, not endorsed: the offsets wrap and the lateness is an
/// `int64_t`.
#[test]
fn steps_reanchor_wraps() {
    let f = FIELD_50I_NS;
    check_reanchor(
        0,
        VT,
        VT + 10 * MS,
        FIELD_50I,
        TICK_60,
        u64::MAX - 10 * MS + 1,
    );
    check_reanchor(
        u64::MAX - 10 * MS + 1,
        VT + MS,
        VT + 10 * MS,
        FIELD_50I,
        TICK_60,
        u64::MAX - 10 * MS + 1,
    );
    check_reanchor(u64::MAX, VT, VT, FIELD_50I, TICK_60, u64::MAX);
    check_reanchor(
        u64::MAX,
        VT + f - TWOX - 1,
        VT,
        FIELD_50I,
        TICK_60,
        f - TWOX - 1,
    );
    check_reanchor(
        u64::MAX,
        VT + f - TWOX - 2,
        VT,
        FIELD_50I,
        TICK_60,
        u64::MAX,
    );
    check_reanchor(
        1,
        VT,
        VT.wrapping_sub(INT64_MAX + 1),
        FIELD_50I,
        TICK_60,
        INT64_MAX + 1,
    );
    check_reanchor(
        1,
        VT,
        VT.wrapping_sub(INT64_MAX + 2),
        FIELD_50I,
        TICK_60,
        INT64_MAX + 2,
    );
    check_reanchor(
        10 * MS,
        VT,
        VT - 10 * MS - (f - TWOX - 1),
        FIELD_50I,
        TICK_60,
        10 * MS,
    );
    check_reanchor(2, 0, 0, FIELD_50I, TICK_60, 0);
    check_reanchor(u64::MAX, 0, 1, FIELD_50I, TICK_60, u64::MAX);
    check_reanchor(1, 0, 0, FIELD_50I, TICK_60, 0);
}

// --------------------------------------------------------------------------
// Sequences: an unbuffered source through a model of
// deinterlace_get_closest_frames()

/// `n * 1e9 * den / num`: the n-th frame or tick of a rate `num/den`.
fn rate_time(n: u64, num: u64, den: u64) -> u64 {
    n * 1_000_000_000 * den / num
}

#[derive(Debug, Default, PartialEq, Eq)]
struct SeqResult {
    reanchors: usize,
    skipped: usize,
    early: usize,
}

/// The C test's model, step for step.
fn run_unbuffered(
    frame_num: u64,
    frame_den: u64,
    canvas_num: u64,
    canvas_den: u64,
    frames: i64,
) -> SeqResult {
    let mut r = SeqResult::default();
    let interval = 1_000_000_000 * canvas_den / canvas_num;
    let first_tick = VT + rate_time(1, canvas_num, canvas_den);
    let mut frame_ts = 0;
    let mut offset = 0;
    let mut half = 0;
    let mut last: i64 = -1;
    let at = |n: i64| rate_time(u64::try_from(n).unwrap(), frame_num, frame_den);

    let mut tick = 1;
    while last + 1 < frames {
        let video_time = VT + rate_time(tick, canvas_num, canvas_den);
        tick += 1;
        if offset != 0 && hold(frame_ts, offset, half, video_time) {
            continue;
        }

        let mut next = last;
        while next + 1 < frames && first_tick + at(next + 1) <= video_time {
            next += 1;
        }
        if next == last {
            continue;
        }

        let ts = TS + at(next);
        let mut prev_ts = ts - at(1);
        if last >= 0 {
            r.skipped += usize::try_from(next - last - 1).unwrap();
            if video_time < frame_ts + offset + 2 * u64::from(half) - TWOX {
                r.early += 1;
            }
            prev_ts = frame_ts;
        }
        half = half_duration(ts, prev_ts);
        frame_ts = ts;
        let before = offset;
        offset = reanchor(offset, video_time, frame_ts, half, interval);
        if last >= 0 && offset != before {
            r.reanchors += 1;
        }
        last = next;
    }
    r
}

#[track_caller]
fn check_seq(
    frame_num: u64,
    frame_den: u64,
    canvas_num: u64,
    canvas_den: u64,
    reanchors: usize,
    skipped: usize,
) {
    assert_eq!(
        run_unbuffered(frame_num, frame_den, canvas_num, canvas_den, 100),
        SeqResult {
            reanchors,
            skipped,
            early: 0
        },
        "{frame_num}/{frame_den} into {canvas_num}/{canvas_den}"
    );
}

#[test]
fn steps_seq_unbuffered_25_and_50() {
    check_seq(25, 1, 30, 1, 0, 0);
    check_seq(25, 1, 60, 1, 0, 0);
    check_seq(50, 1, 30, 1, 0, 76);
    check_seq(50, 1, 60, 1, 0, 0);
}

/// Characterized, not endorsed: the offset follows the canvas.
#[test]
fn steps_seq_unbuffered_ntsc() {
    check_seq(30000, 1001, 30, 1, 99, 0);
    check_seq(30000, 1001, 60, 1, 99, 0);
    check_seq(60000, 1001, 30, 1, 0, 49);
    check_seq(60000, 1001, 60, 1, 99, 0);
}

#[test]
fn steps_seq_unbuffered_late_frame() {
    let offset = VT - TS;
    let mut o = offset;

    o = reanchor(
        o,
        VT + FRAME_50I + 5 * MS,
        TS + FRAME_50I,
        FIELD_50I,
        TICK_60,
    );
    assert_eq!(o, offset);
    o = reanchor(
        o,
        VT + 2 * FRAME_50I + 25 * MS,
        TS + 2 * FRAME_50I,
        FIELD_50I,
        TICK_60,
    );
    assert_eq!(o, offset + 25 * MS);
    o = reanchor(
        o,
        VT + 3 * FRAME_50I + 35 * MS,
        TS + 3 * FRAME_50I,
        FIELD_50I,
        TICK_60,
    );
    assert_eq!(o, offset + 25 * MS);
    o = reanchor(
        o,
        VT + 4 * FRAME_50I,
        TS + 4 * FRAME_50I,
        FIELD_50I,
        TICK_60,
    );
    assert_eq!(o, offset);
}

// --------------------------------------------------------------------------
// New edge cases: a model of the C's uint64_t arithmetic in i128

const MOD: i128 = 1 << 64;

/// `x` modulo 2^64, as `uint64_t` keeps it.
fn wrap(x: i128) -> u64 {
    u64::try_from(x.rem_euclid(MOD)).unwrap()
}

fn model_hold(frame_ts: u64, offset: u64, half: u32, sys_time: u64) -> bool {
    let end =
        wrap(i128::from(frame_ts) + i128::from(offset) + 2 * i128::from(half) - i128::from(TWOX));
    sys_time < end && end - sys_time < JUMP
}

fn model_wait(_ts: u64, next_ts: u64, offset: u64, sys_time: u64) -> bool {
    // timestamp + offset + (next - timestamp) is next + offset, modulo 2^64
    sys_time < wrap(i128::from(next_ts) + i128::from(offset))
}

fn model_waiting_ts(ts: u64, next_ts: u64) -> u64 {
    wrap(2 * i128::from(ts) - i128::from(next_ts))
}

fn model_half(ts: u64, prev_ts: u64) -> u32 {
    let half = wrap(i128::from(ts) - i128::from(prev_ts)) / 2;
    u32::try_from(half % TWO_32).unwrap()
}

fn model_buffered(video_time: u64, last_frame_ts: u64) -> u64 {
    wrap(i128::from(video_time) - i128::from(last_frame_ts) + 3_000_000)
}

fn model_reanchor(offset: u64, video_time: u64, frame_ts: u64, half: u32, interval: u64) -> u64 {
    let new_offset = wrap(i128::from(video_time) - i128::from(frame_ts));
    let diff = wrap(i128::from(new_offset) - i128::from(offset));
    // (int64_t): negative from 2^63
    let early = diff >= 1 << 63;
    let field_or_tick = u64::from(half).max(interval);
    if offset == 0 || early || u128::from(diff) + u128::from(TWOX) >= u128::from(field_or_tick) {
        new_offset
    } else {
        offset
    }
}

const SPECIAL: [u64; 22] = [
    0,
    1,
    2,
    TWOX - 1,
    TWOX,
    TWOX + 1,
    FRAME_TOL,
    TICK_60,
    FIELD_50I_NS,
    FRAME_50I,
    JUMP - 1,
    JUMP,
    JUMP + 1,
    TWO_32 - 1,
    TWO_32,
    VT,
    INT64_MAX,
    INT64_MAX + 1,
    INT64_MAX + 2,
    u64::MAX - JUMP,
    u64::MAX - 1,
    u64::MAX,
];

const SPECIAL_HALF: [u32; 9] = [
    0,
    1,
    500_000,
    8_333_333,
    16_666_666,
    20_000_000,
    35_000_000,
    u32::MAX - 1,
    u32::MAX,
];

#[test]
fn edge_hold_special_values() {
    for &frame_ts in &SPECIAL {
        for &offset in &SPECIAL {
            for &half in &SPECIAL_HALF {
                for &sys_time in &SPECIAL {
                    assert_eq!(
                        hold(frame_ts, offset, half, sys_time),
                        model_hold(frame_ts, offset, half, sys_time),
                        "hold({frame_ts}, {offset}, {half}, {sys_time})"
                    );
                }
            }
        }
    }
}

#[test]
fn edge_hold_jump_threshold_at_every_end() {
    for &frame_ts in &SPECIAL {
        for &half in &SPECIAL_HALF {
            let end = wrap(i128::from(frame_ts) + 7 + 2 * i128::from(half) - i128::from(TWOX));
            for (back, expected) in [
                (0, false),
                (1, true),
                (JUMP - 1, true),
                (JUMP, false),
                (JUMP + 1, false),
            ] {
                let sys_time = end.wrapping_sub(back);
                let expected = expected && sys_time < end;
                assert_eq!(
                    hold(frame_ts, 7, half, sys_time),
                    expected,
                    "{frame_ts} {half} {back}"
                );
            }
        }
    }
}

#[test]
fn edge_wait_special_values() {
    for &ts in &SPECIAL {
        for &next_ts in &SPECIAL {
            assert_eq!(waiting_frame_ts(ts, next_ts), model_waiting_ts(ts, next_ts));
            for &offset in &SPECIAL {
                for &sys_time in &SPECIAL {
                    assert_eq!(
                        wait_for_next(ts, next_ts, offset, sys_time),
                        model_wait(ts, next_ts, offset, sys_time),
                        "wait_for_next({ts}, {next_ts}, {offset}, {sys_time})"
                    );
                }
            }
        }
    }
}

#[test]
fn edge_half_and_buffered_special_values() {
    for &a in &SPECIAL {
        for &b in &SPECIAL {
            assert_eq!(
                half_duration(a, b),
                model_half(a, b),
                "half_duration({a}, {b})"
            );
            assert_eq!(
                buffered_offset(a, b),
                model_buffered(a, b),
                "buffered_offset({a}, {b})"
            );
        }
    }
}

#[test]
fn edge_reanchor_special_values() {
    for &offset in &SPECIAL {
        for &video_time in &SPECIAL {
            for &frame_ts in &SPECIAL {
                for &half in &SPECIAL_HALF {
                    for &interval in &SPECIAL {
                        let got = reanchor(offset, video_time, frame_ts, half, interval);
                        assert_eq!(
                            got,
                            model_reanchor(offset, video_time, frame_ts, half, interval),
                            "reanchor({offset}, {video_time}, {frame_ts}, {half}, {interval})"
                        );
                        // only the old offset or the tick's
                        assert!(got == offset || got == video_time.wrapping_sub(frame_ts));
                    }
                }
            }
        }
    }
}

/// The lateness at the field-or-tick threshold and at the `int64_t` sign,
/// from every old offset.
#[test]
fn edge_reanchor_thresholds_at_every_offset() {
    for &offset in &SPECIAL {
        if offset == 0 {
            continue;
        }
        for &half in &SPECIAL_HALF {
            for interval in [0, TWOX, TICK_60, TICK_30, u64::from(u32::MAX) + 1] {
                let field_or_tick = u64::from(half).max(interval);
                let threshold = field_or_tick.saturating_sub(TWOX);
                for (late, keeps) in [
                    (0, threshold > 0),
                    (threshold.wrapping_sub(1), threshold > 0),
                    (threshold, false),
                    (threshold + 1, false),
                    (INT64_MAX, false),
                    (INT64_MAX + 1, false),
                    (u64::MAX, false),
                ] {
                    // late is new - offset: pick frame_ts 0 and video_time
                    // so that video_time - 0 = offset + late
                    let video_time = offset.wrapping_add(late);
                    let got = reanchor(offset, video_time, 0, half, interval);
                    let expected = if keeps { offset } else { video_time };
                    assert_eq!(got, expected, "{offset} {half} {interval} {late}");
                }
            }
        }
    }
}

fn near(points: &'static [u64]) -> impl Strategy<Value = u64> {
    (proptest::sample::select(points), -3i64..=3).prop_map(|(p, d)| p.wrapping_add_signed(d))
}

fn any_time() -> impl Strategy<Value = u64> {
    prop_oneof![any::<u64>(), near(&SPECIAL)]
}

fn any_half() -> impl Strategy<Value = u32> {
    prop_oneof![
        any::<u32>(),
        0u32..40_000_000,
        proptest::sample::select(&SPECIAL_HALF[..])
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4000))]

    #[test]
    fn edge_proptest_hold(frame_ts in any_time(), offset in any_time(), half in any_half(), back in any_time()) {
        // sys_time drawn relative to the end as often as anywhere
        let end = wrap(i128::from(frame_ts) + i128::from(offset) + 2 * i128::from(half) - i128::from(TWOX));
        for sys_time in [back, end.wrapping_sub(back % (2 * JUMP))] {
            prop_assert_eq!(hold(frame_ts, offset, half, sys_time), model_hold(frame_ts, offset, half, sys_time));
        }
    }

    #[test]
    fn edge_proptest_wait(ts in any_time(), next_ts in any_time(), offset in any_time(), sys_time in any_time()) {
        prop_assert_eq!(wait_for_next(ts, next_ts, offset, sys_time), model_wait(ts, next_ts, offset, sys_time));
        prop_assert_eq!(waiting_frame_ts(ts, next_ts), model_waiting_ts(ts, next_ts));
    }

    #[test]
    fn edge_proptest_half_and_buffered(a in any_time(), b in any_time()) {
        prop_assert_eq!(half_duration(a, b), model_half(a, b));
        prop_assert_eq!(buffered_offset(a, b), model_buffered(a, b));
    }

    #[test]
    fn edge_proptest_reanchor(
        offset in any_time(),
        frame_ts in any_time(),
        late in prop_oneof![any::<u64>(), 0u64..40_000_000, near(&SPECIAL)],
        half in any_half(),
        interval in prop_oneof![any::<u64>(), Just(TICK_60), Just(TICK_30), Just(20 * MS)],
    ) {
        let video_time = frame_ts.wrapping_add(offset).wrapping_add(late);
        prop_assert_eq!(
            reanchor(offset, video_time, frame_ts, half, interval),
            model_reanchor(offset, video_time, frame_ts, half, interval)
        );
    }
}
