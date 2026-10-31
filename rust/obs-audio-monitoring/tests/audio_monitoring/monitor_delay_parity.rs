//! Tier 3 parity: the `monitor_delay` core and its shims against the original
//! `monitor-delay.c.h` (oracle/monitor_delay.c).
//!
//! The unchanged C test `test_audio_monitor_delay_steps` compiles the C into
//! itself; `test_audio_monitor_delay_steps_rust` runs it against the shims,
//! and this file compares all three sides (oracle, shims, core) exactly: the
//! constants, every step over grids of +-3 around every threshold of every
//! input, the front judgement put together as `process_audio_delay()` asks
//! it (the stall count only when the gate's first part leaves it open, and
//! whether it was asked compared too), the loop on the oracle and on the
//! shims over runs of packets, and proptests.
//!
//! No intentional difference. Excluded, because the C leaves them undefined:
//! a judgement whose `int64_t` difference overflows (the front timestamp and
//! the video timestamp, each cast, more than `i64` apart) or is `INT64_MIN`
//! (`llabs()`). Timestamps are nanoseconds of the monotonic clock, far below
//! 2^62, so neither happens in libobs; [`defined`] picks them out, and
//! `edge_judge_min_difference` in the Tier 1 tests pins what the core does
//! with `INT64_MIN`. Null pointers, which the C dereferences, are checked on
//! the Rust side only.

use std::collections::VecDeque;
use std::ptr;

use obs_audio_monitoring::ffi::monitor_delay as shim;
use obs_audio_monitoring::monitor_delay::{
    self as core, MAX_DIFF_NS, STALE_NS, SYNC_NS, VIDEO_STALL_NS, Verdict,
};
use obs_c_oracle::monitor_delay::{
    VALUE_NAMES, oracle_audio_monitor_delay_diff, oracle_audio_monitor_delay_judge_diff,
    oracle_audio_monitor_delay_stale, oracle_audio_monitor_delay_stalled,
    oracle_audio_monitor_delay_unsynced, oracle_monitor_delay_value,
};
use obs_c_oracle::util_uint64::oracle_util_mul_div64;
use proptest::prelude::*;

const MS: u64 = 1_000_000;
const S: u64 = 1_000_000_000;

/// The C's judgement is defined: the `int64_t` subtraction does not
/// overflow and its result is not `INT64_MIN`.
fn defined(cur_ts: u64, pad_ns: u64, last_frame_ts: u64) -> bool {
    cur_ts
        .wrapping_sub(pad_ns)
        .cast_signed()
        .checked_sub(last_frame_ts.cast_signed())
        .is_some_and(|d| d != i64::MIN)
}

fn verdict_from_c(v: u32) -> Verdict {
    match v {
        0 => Verdict::Play,
        1 => Verdict::Hold,
        2 => Verdict::Drag,
        _ => panic!("verdict {v} out of range"),
    }
}

fn oracle_stale(last: u64, cur: u64) -> (bool, u64) {
    let mut l = last;
    // SAFETY: a valid u64 to write.
    let s = unsafe { oracle_audio_monitor_delay_stale(&raw mut l, cur) };
    (s, l)
}

fn shim_stale(last: u64, cur: u64) -> (bool, u64) {
    let mut l = last;
    // SAFETY: a valid u64 to write.
    let s = unsafe { shim::audio_monitor_delay_stale(&raw mut l, cur) };
    (s, l)
}

fn core_stale(last: u64, cur: u64) -> (bool, u64) {
    let mut l = last;
    (core::stale(&mut l, cur), l)
}

/// The front packet's verdict, what `process_audio_delay()` keeps for its
/// `DEBUG_AUDIO` logs, and whether the stall count was asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Judgement {
    verdict: Verdict,
    front_ts: u64,
    diff: i64,
    stall_asked: bool,
}

/// The four steps of one side.
struct Steps {
    diff: fn(u64, u64, u64) -> (u64, i64),
    unsynced: fn(u64, i64) -> bool,
    stalled: fn(u64) -> bool,
    judge: fn(bool, i64) -> Verdict,
}

/// The steps put together as `process_audio_delay()` asks them: the
/// difference, the gate's first part, its second (the stall count) only
/// when the first is false, then the verdict.
fn compose(steps: &Steps, cur_ts: u64, pad_ns: u64, video: u64, stall: u64) -> Judgement {
    let (front_ts, diff) = (steps.diff)(cur_ts, pad_ns, video);
    let unsynced = (steps.unsynced)(video, diff);
    let bad_diff = unsynced || (steps.stalled)(stall);
    Judgement {
        verdict: (steps.judge)(bad_diff, diff),
        front_ts,
        diff,
        stall_asked: !unsynced,
    }
}

const ORACLE: Steps = Steps {
    diff: |cur_ts, pad_ns, video| {
        let mut front_ts = 0;
        // SAFETY: a valid output to write; the caller keeps to defined().
        let diff =
            unsafe { oracle_audio_monitor_delay_diff(cur_ts, pad_ns, video, &raw mut front_ts) };
        (front_ts, diff)
    },
    // SAFETY: a pure function; the caller keeps to defined().
    unsynced: |video, diff| unsafe { oracle_audio_monitor_delay_unsynced(video, diff) },
    // SAFETY: a pure function.
    stalled: |stall| unsafe { oracle_audio_monitor_delay_stalled(stall) },
    // SAFETY: a pure function.
    judge: |bad, diff| verdict_from_c(unsafe { oracle_audio_monitor_delay_judge_diff(bad, diff) }),
};

const SHIM: Steps = Steps {
    diff: |cur_ts, pad_ns, video| {
        let mut front_ts = 0;
        // SAFETY: a valid output to write.
        let diff =
            unsafe { shim::audio_monitor_delay_diff(cur_ts, pad_ns, video, &raw mut front_ts) };
        (front_ts, diff)
    },
    unsynced: |video, diff| shim::audio_monitor_delay_unsynced(video, diff),
    stalled: |stall| shim::audio_monitor_delay_stalled(stall),
    judge: |bad, diff| verdict_from_c(shim::audio_monitor_delay_judge_diff(bad, diff)),
};

const CORE: Steps = Steps {
    diff: |cur_ts, pad_ns, video| {
        let f = core::diff(cur_ts, pad_ns, video);
        (f.front_ts, f.diff)
    },
    unsynced: core::unsynced,
    stalled: core::stalled,
    judge: core::judge,
};

fn oracle_judge(cur_ts: u64, pad_ns: u64, video: u64, stall: u64) -> Judgement {
    compose(&ORACLE, cur_ts, pad_ns, video, stall)
}

fn shim_judge(cur_ts: u64, pad_ns: u64, video: u64, stall: u64) -> Judgement {
    compose(&SHIM, cur_ts, pad_ns, video, stall)
}

fn core_judge(cur_ts: u64, pad_ns: u64, video: u64, stall: u64) -> Judgement {
    compose(&CORE, cur_ts, pad_ns, video, stall)
}

fn check_stale(last: u64, cur: u64) {
    let want = oracle_stale(last, cur);
    assert_eq!(shim_stale(last, cur), want, "shim stale {last} {cur}");
    assert_eq!(core_stale(last, cur), want, "core stale {last} {cur}");
}

/// Compares the three sides; returns whether the input was compared.
fn check_judge(cur_ts: u64, pad_ns: u64, video: u64, stall: u64) -> bool {
    if !defined(cur_ts, pad_ns, video) {
        return false;
    }
    let want = oracle_judge(cur_ts, pad_ns, video, stall);
    assert_eq!(
        shim_judge(cur_ts, pad_ns, video, stall),
        want,
        "shim judge {cur_ts} {pad_ns} {video} {stall}"
    );
    assert_eq!(
        core_judge(cur_ts, pad_ns, video, stall),
        want,
        "core judge {cur_ts} {pad_ns} {video} {stall}"
    );
    true
}

/// `base` +-3, wrapping.
fn around(base: u64) -> impl Iterator<Item = u64> {
    (-3i64..=3).map(move |d| base.wrapping_add_signed(d))
}

#[test]
fn constants_match_c() {
    let rust: [i64; 8] = [
        STALE_NS.cast_signed(),
        MAX_DIFF_NS,
        SYNC_NS,
        VIDEO_STALL_NS.cast_signed(),
        i64::from(Verdict::Play.to_c()),
        i64::from(Verdict::Hold.to_c()),
        i64::from(Verdict::Drag.to_c()),
        4,
    ];
    for (i, (name, want)) in VALUE_NAMES.iter().zip(rust).enumerate() {
        // SAFETY: an index within the table.
        assert_eq!(unsafe { oracle_monitor_delay_value(i) }, want, "{name}");
    }
    // SAFETY: past the table, which the C checks.
    assert_eq!(unsafe { oracle_monitor_delay_value(VALUE_NAMES.len()) }, -1);
}

/// Every receive time near 0, the thresholds and the ends of u64 and i64,
/// with every clock step near 0, 1 s, 2 s, 2^63 and u64::MAX (so backwards
/// too), both ways.
#[test]
fn stale_grid() {
    let lasts = [0, 1, S, 10 * S, 1 << 62, i64::MAX as u64, u64::MAX];
    let steps = [0, STALE_NS, 2 * S, 1 << 63, u64::MAX];
    let mut n = 0;
    for &l in &lasts {
        for last in around(l) {
            for &st in &steps {
                for step in around(st) {
                    check_stale(last, last.wrapping_add(step));
                    n += 1;
                }
            }
        }
    }
    assert_eq!(n, 7 * 7 * 5 * 7);
}

/// The verdict over +-3 around every threshold of every input: the video
/// timestamp (0, 1, 10 s, 2^62, `INT64_MAX`, `UINT64_MAX`), the difference
/// (0, +-75 ms, +-1 s, +-5 s, +-10 s), the padding (0, 100 ms and the whole
/// front timestamp, so it wraps) and the stall count (100 ms, with 0 and
/// `UINT64_MAX`).
#[test]
fn judge_grid() {
    let videos = [0, 1, 10 * S, 1 << 62, i64::MAX as u64, u64::MAX];
    let diffs: Vec<i64> = [
        0,
        SYNC_NS,
        S.cast_signed(),
        MAX_DIFF_NS,
        10 * S.cast_signed(),
    ]
    .iter()
    .flat_map(|&t| [t, t.wrapping_neg()])
    .collect();
    let pads = [0, 100 * MS];
    let stalls: Vec<u64> = around(VIDEO_STALL_NS).chain([0, u64::MAX]).collect();
    let (mut compared, mut skipped) = (0u32, 0u32);
    for &v in &videos {
        for video in around(v) {
            for &d in &diffs {
                for diff in (-3..=3).map(|e| d.wrapping_add(e)) {
                    let front = video.wrapping_add_signed(diff);
                    for &p in &pads {
                        for pad in around(p).chain(around(front)) {
                            let ts = front.wrapping_add(pad);
                            for &stall in &stalls {
                                if check_judge(ts, pad, video, stall) {
                                    compared += 1;
                                } else {
                                    skipped += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    // Only the ends of u64 and i64 overflow the difference.
    assert!(
        compared > 10 * skipped,
        "{compared} compared, {skipped} skipped"
    );
}

/// Null pointers, which the C dereferences: the shims read 0 and write
/// nothing.
#[test]
fn shims_take_null() {
    // SAFETY: null is allowed by the shims.
    let stale = unsafe { shim::audio_monitor_delay_stale(ptr::null_mut(), S + 1) };
    assert!(stale);
    // SAFETY: as above.
    let stale = unsafe { shim::audio_monitor_delay_stale(ptr::null_mut(), S) };
    assert!(!stale);
    // SAFETY: as above.
    let diff = unsafe { shim::audio_monitor_delay_diff(10 * S + S, 0, 10 * S, ptr::null_mut()) };
    assert_eq!(diff, S.cast_signed());
}

/// Each step alone: the gate's two parts and the verdict over +-3 around
/// every threshold, the video timestamps of `judge_grid` and every
/// difference near 0, +-75 ms, +-5 s and the ends of `i64`.
#[test]
fn steps_grid() {
    let diffs: Vec<i64> = [0, SYNC_NS, MAX_DIFF_NS, i64::MAX]
        .iter()
        .flat_map(|&t| {
            (-3..=3).flat_map(move |e| [t.wrapping_add(e), t.wrapping_neg().wrapping_add(e)])
        })
        .collect();
    for &v in &[0, 1, 10 * S, 1 << 62, i64::MAX as u64, u64::MAX] {
        for video in around(v) {
            for &d in &diffs {
                // llabs(INT64_MIN) is undefined in the C.
                if d == i64::MIN {
                    continue;
                }
                let want = (ORACLE.unsynced)(video, d);
                assert_eq!((SHIM.unsynced)(video, d), want, "shim unsynced {video} {d}");
                assert_eq!((CORE.unsynced)(video, d), want, "core unsynced {video} {d}");
            }
        }
    }
    for &st in &[0, VIDEO_STALL_NS, S, u64::MAX] {
        for stall in around(st) {
            let want = (ORACLE.stalled)(stall);
            assert_eq!((SHIM.stalled)(stall), want, "shim stalled {stall}");
            assert_eq!((CORE.stalled)(stall), want, "core stalled {stall}");
        }
    }
    for &d in &diffs {
        for bad in [false, true] {
            let want = (ORACLE.judge)(bad, d);
            assert_eq!((SHIM.judge)(bad, d), want, "shim judge {bad} {d}");
            assert_eq!((CORE.judge)(bad, d), want, "core judge {bad} {d}");
        }
    }
}

/// `audio_monitor_track_video_ts()` (`monitor-video-sync.h`, unported): the
/// stall count the C passes in.
fn track_video_ts(
    prev_video_ts: &mut u64,
    time_since_prev: &mut u64,
    last_frame_ts: u64,
    packet_ns: u64,
) {
    if *prev_video_ts == 0 {
        *prev_video_ts = last_frame_ts;
    } else if *prev_video_ts == last_frame_ts {
        *time_since_prev = time_since_prev.wrapping_add(packet_ns);
    } else {
        *prev_video_ts = last_frame_ts;
        *time_since_prev = 0;
    }
}

#[derive(Clone, Debug)]
struct Packet {
    clock_step: u64,
    ts: u64,
    frames: u32,
    pad: u32,
    video: u64,
}

/// What `process_audio_delay()` does with one packet.
#[derive(Debug, PartialEq, Eq)]
struct Step {
    stale: bool,
    judgements: Vec<Judgement>,
    played: Option<u64>,
    queued: usize,
    last_recv_time: u64,
}

/// `process_audio_delay()` over a run of packets, its two decisions asked of
/// `stale` and `judge`. The queue holds each packet's timestamp; the clock,
/// the stall tracking and both `util_mul_div64()` calls are the C caller's.
fn run_loop(
    rate: u32,
    sync_offset: i64,
    packets: &[Packet],
    stale: &dyn Fn(u64, u64) -> (bool, u64),
    judge: &dyn Fn(u64, u64, u64, u64) -> Judgement,
) -> Vec<Step> {
    let mut queue: VecDeque<u64> = VecDeque::new();
    let (mut clock, mut last_recv_time, mut prev_video_ts, mut time_since_prev) =
        (S, 0u64, 0u64, 0u64);
    let mut steps = Vec::with_capacity(packets.len());
    for p in packets {
        clock = clock.wrapping_add(p.clock_step);
        let (cut, recv) = stale(last_recv_time, clock);
        last_recv_time = recv;
        if cut {
            queue.clear();
        }
        queue.push_back(p.ts.wrapping_add_signed(sync_offset));
        // SAFETY: a pure function; rate is never 0 here.
        let packet_ns = unsafe { oracle_util_mul_div64(u64::from(p.frames), S, u64::from(rate)) };
        track_video_ts(&mut prev_video_ts, &mut time_since_prev, p.video, packet_ns);

        let mut judgements = Vec::new();
        let mut played = None;
        while let Some(&front) = queue.front() {
            // SAFETY: as above.
            let pad_ns = unsafe { oracle_util_mul_div64(u64::from(p.pad), S, u64::from(rate)) };
            let j = judge(front, pad_ns, p.video, time_since_prev);
            judgements.push(j);
            if j.verdict == Verdict::Hold {
                break;
            }
            queue.pop_front();
            if j.verdict == Verdict::Drag && !queue.is_empty() {
                continue;
            }
            played = Some(front);
            break;
        }
        steps.push(Step {
            stale: cut,
            judgements,
            played,
            queued: queue.len(),
            last_recv_time,
        });
    }
    steps
}

fn oracle_loop_judge(cur_ts: u64, pad_ns: u64, video: u64, stall: u64) -> Judgement {
    assert!(
        defined(cur_ts, pad_ns, video),
        "a packet run left the defined range"
    );
    oracle_judge(cur_ts, pad_ns, video, stall)
}

/// A run of packets: a clock that steps by about a packet and sometimes
/// stalls past 1 s or steps back, video timestamps that advance, stand still
/// and jump, packet timestamps near the video (+-75 ms, +-5 s) with jitter,
/// and paddings of 0 to 48 000 frames.
fn packets(rate: u32) -> impl Strategy<Value = Vec<Packet>> {
    let step = prop_oneof![
        8 => 0u64..=50 * MS,
        1 => Just(STALE_NS),
        1 => Just(STALE_NS + 1),
        1 => 0u64..=3 * S,
        1 => Just(u64::MAX),
    ];
    let video_move = prop_oneof![4 => Just(0u64), 4 => 0u64..=40 * MS, 1 => 1u64..=1 << 45];
    let offset = prop_oneof![
        -10 * MS.cast_signed()..=10 * MS.cast_signed(),
        Just(SYNC_NS),
        Just(SYNC_NS + 1),
        Just(-SYNC_NS - 1),
        Just(MAX_DIFF_NS + 1),
        Just(-MAX_DIFF_NS - 1),
        -6 * S.cast_signed()..=6 * S.cast_signed(),
    ];
    let packet = (
        step,
        video_move,
        any::<bool>(),
        offset,
        0u32..=4096,
        0u32..=48_000,
    );
    proptest::collection::vec(packet, 1..=512).prop_map(move |raw| {
        let mut video = 1u64 << 40;
        raw.into_iter()
            .map(
                |(clock_step, video_move, video_zero, offset, frames, pad)| {
                    video = video.wrapping_add(video_move);
                    let shown = if video_zero && video_move == 0 {
                        0
                    } else {
                        video
                    };
                    // SAFETY: a pure function; rate is never 0.
                    let pad_ns =
                        unsafe { oracle_util_mul_div64(u64::from(pad), S, u64::from(rate)) };
                    Packet {
                        clock_step,
                        ts: video.wrapping_add_signed(offset).wrapping_add(pad_ns),
                        frames,
                        pad,
                        video: shown,
                    }
                },
            )
            .collect()
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(20_000)))]

    #[test]
    fn prop_stale(last in any::<u64>(), cur in any::<u64>()) {
        check_stale(last, cur);
    }

    #[test]
    fn prop_judge(
        ts in any::<u64>(),
        pad in prop_oneof![0u64..=2 * S, any::<u64>()],
        video in prop_oneof![Just(0u64), 1u64..=1 << 62, any::<u64>()],
        stall in prop_oneof![0u64..=2 * VIDEO_STALL_NS, any::<u64>()],
    ) {
        check_judge(ts, pad, video, stall);
    }

    #[test]
    fn prop_judge_near(
        video in prop_oneof![Just(0u64), 1u64..=1 << 50],
        diff in -6 * S.cast_signed()..=6 * S.cast_signed(),
        pad in 0u64..=S,
        stall in 0u64..=2 * VIDEO_STALL_NS,
    ) {
        let ts = video.wrapping_add_signed(diff).wrapping_add(pad);
        prop_assert!(check_judge(ts, pad, video, stall));
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(200)))]

    /// `process_audio_delay()` over runs of up to 512 packets, at rates of
    /// 8 000 to 192 000 and sync offsets of +-10 s: every stale cut, every
    /// judgement with what it writes back, what plays, the queue and the
    /// receive time, oracle against shims and core.
    #[test]
    fn prop_loop(
        (rate, run) in prop_oneof![
            Just(8000u32), Just(44_100), Just(48_000), Just(96_000), Just(192_000), 8000u32..=192_000
        ].prop_flat_map(|rate| (Just(rate), packets(rate))),
        sync_offset in prop_oneof![Just(0i64), -10 * S.cast_signed()..=10 * S.cast_signed()],
    ) {
        let want = run_loop(rate, sync_offset, &run, &oracle_stale, &oracle_loop_judge);
        prop_assert_eq!(&run_loop(rate, sync_offset, &run, &shim_stale, &shim_judge), &want);
        prop_assert_eq!(&run_loop(rate, sync_offset, &run, &core_stale, &core_judge), &want);
    }
}
