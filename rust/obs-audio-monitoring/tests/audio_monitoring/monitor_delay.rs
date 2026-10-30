//! Tier 1: `obs_audio_monitoring::monitor_delay`, the WASAPI monitor's delay
//! decisions.
//!
//! The `steps_*` cases port `test/cmocka/test_audio_monitor_delay_steps.c`
//! 1:1: its 9 cases under the same names, in the same order, with the same
//! values and expectations, the "characterized, not endorsed" ones included.
//! The C calls the hidden `audio_monitor_delay_*` functions; here the safe
//! core is called directly. The C's stand-in of `process_audio_delay()`'s
//! loop is [`stand_in_loop`].
//!
//! The `edge_*` cases are new: both decisions against a model of the
//! origin/master C (its expressions inline) over every pair of edge values,
//! and proptests over full-range inputs and over runs of packets.

use std::cell::Cell;

use obs_audio_monitoring::monitor_delay::{
    MAX_DIFF_NS, STALE_NS, SYNC_NS, VIDEO_STALL_NS, Verdict, diff, judge, stale, stalled, unsynced,
};
use proptest::prelude::*;

const MS: u64 = 1_000_000;
const S: u64 = 1_000_000_000;
/// A video timestamp well away from 0 and from the limits.
const VIDEO: u64 = 10 * S;

/// The front packet's verdict with what `process_audio_delay()` keeps for
/// its `DEBUG_AUDIO` logs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Judgement {
    verdict: Verdict,
    front_ts: u64,
    diff: i64,
}

thread_local! {
    /// Times [`judge4`] asked for the stall count on this thread.
    static STALL_ASKS: Cell<u32> = const { Cell::new(0) };
}

/// The C test's `judge()`: the front packet's steps asked as
/// `process_audio_delay()` asks them: the difference, then the gate, whose
/// second part (the stall count) only when the first does not decide it,
/// then the verdict.
fn judge4(cur_ts: u64, pad_ns: u64, last_frame_ts: u64, time_since_prev: u64) -> Judgement {
    let front = diff(cur_ts, pad_ns, last_frame_ts);
    let bad_diff = unsynced(last_frame_ts, front.diff) || {
        STALL_ASKS.set(STALL_ASKS.get() + 1);
        stalled(time_since_prev)
    };
    Judgement {
        verdict: judge(bad_diff, front.diff),
        front_ts: front.front_ts,
        diff: front.diff,
    }
}

// ---- models of the origin/master C, expressions inline ----

/// `cur_time - monitor->last_recv_time > 1000000000`, then
/// `monitor->last_recv_time = cur_time`.
fn c_stale(last_recv_time: &mut u64, cur_time: u64) -> bool {
    let stale = cur_time.wrapping_sub(*last_recv_time) > 1_000_000_000;
    *last_recv_time = cur_time;
    stale
}

/// `!last_frame_ts || llabs(diff) > 5000000000`, for a difference whose
/// `llabs()` is defined.
fn c_unsynced(last_frame_ts: u64, diff: i64) -> bool {
    last_frame_ts == 0 || diff.unsigned_abs() > 5_000_000_000
}

/// `monitor->time_since_prev > AUDIO_MONITOR_VIDEO_STALL_NS`.
fn c_stalled(time_since_prev: u64) -> bool {
    time_since_prev > 100_000_000
}

/// The rush and drag tests.
fn c_judge_diff(bad_diff: bool, diff: i64) -> Verdict {
    if !bad_diff && diff > 75_000_000 {
        Verdict::Hold
    } else if !bad_diff && diff < -75_000_000 {
        Verdict::Drag
    } else {
        Verdict::Play
    }
}

/// The C's loop body up to the drag test, for a difference whose `llabs()`
/// is defined (not `i64::MIN`).
fn c_judge(cur_ts: u64, pad_ns: u64, last_frame_ts: u64, time_since_prev: u64) -> Judgement {
    // front_ts = cur_ts - util_mul_div64(pad, 1000000000ULL, rate);
    let front_ts = cur_ts.wrapping_sub(pad_ns);
    // diff = (int64_t)front_ts - (int64_t)last_frame_ts;
    let diff = (front_ts as i64).wrapping_sub(last_frame_ts as i64);
    // bad_diff = !last_frame_ts || llabs(diff) > 5000000000 ||
    //            monitor->time_since_prev > AUDIO_MONITOR_VIDEO_STALL_NS;
    let bad_diff =
        last_frame_ts == 0 || diff.unsigned_abs() > 5_000_000_000 || time_since_prev > 100_000_000;
    let verdict = if !bad_diff && diff > 75_000_000 {
        Verdict::Hold
    } else if !bad_diff && diff < -75_000_000 {
        Verdict::Drag
    } else {
        Verdict::Play
    };
    Judgement {
        verdict,
        front_ts,
        diff,
    }
}

// ---- 1:1 port of test_audio_monitor_delay_steps.c ----

/// The verdict on a front packet whose timestamp less the padding is `diff`
/// away from `VIDEO`, with the video moving; checks what is written back.
fn judge_at(diff: i64) -> Verdict {
    let ts = VIDEO.wrapping_add_signed(diff);
    let j = judge4(ts, 0, VIDEO, 0);
    assert_eq!(j.front_ts, ts);
    assert_eq!(j.diff, diff);
    j.verdict
}

#[test]
fn steps_constants() {
    assert_eq!(STALE_NS, 1_000_000_000);
    assert_eq!(MAX_DIFF_NS, 5_000_000_000);
    assert_eq!(SYNC_NS, 75_000_000);
    assert_eq!(VIDEO_STALL_NS, 100_000_000);
    assert_eq!(Verdict::Play.to_c(), 0);
    assert_eq!(Verdict::Hold.to_c(), 1);
    assert_eq!(Verdict::Drag.to_c(), 2);
}

#[test]
fn steps_stale_cut() {
    let mut last = 5 * S;

    assert!(!stale(&mut last, 5 * S));
    assert_eq!(last, 5 * S);
    assert!(!stale(&mut last, 5 * S + 1));
    assert_eq!(last, 5 * S + 1);
    assert!(!stale(&mut last, 6 * S + 1));
    assert_eq!(last, 6 * S + 1);
    assert!(stale(&mut last, 7 * S + 2));
    assert_eq!(last, 7 * S + 2);
    assert!(!stale(&mut last, 7 * S + 2 + 20 * MS));
    assert_eq!(last, 7 * S + 2 + 20 * MS);

    // the first packet: the receive time starts at 0
    last = 0;
    assert!(!stale(&mut last, S));
    assert_eq!(last, S);
    last = 0;
    assert!(stale(&mut last, S + 1));
    last = 0;
    assert!(!stale(&mut last, 0));

    // Characterized, not endorsed: a clock that steps back wraps the u64
    // difference, so the queue is stale.
    last = 5 * S;
    assert!(stale(&mut last, 5 * S - 1));
    assert_eq!(last, 5 * S - 1);

    // the far ends of the clock
    last = 0;
    assert!(stale(&mut last, u64::MAX));
    assert_eq!(last, u64::MAX);
    assert!(!stale(&mut last, u64::MAX));
    assert!(stale(&mut last, S));
}

/// `steps_alone`'s difference checks.
fn steps_alone_diff() {
    let ms = MS as i64;
    let s = S as i64;

    let f = diff(VIDEO + S, 0, VIDEO);
    assert_eq!(f.diff, s);
    assert_eq!(f.front_ts, VIDEO + S);
    let f = diff(VIDEO, 100 * MS, VIDEO);
    assert_eq!(f.diff, -100 * ms);
    assert_eq!(f.front_ts, VIDEO - 100 * MS);
    let f = diff(0, 1, 0);
    assert_eq!(f.diff, -1);
    assert_eq!(f.front_ts, u64::MAX);
}

/// `steps_alone`'s gate checks.
fn steps_alone_gate() {
    let ms = MS as i64;
    let s = S as i64;

    // no video yet, or more than 5 s apart
    assert!(unsynced(0, 0));
    assert!(unsynced(0, 75 * ms + 1));
    assert!(!unsynced(1, 0));
    assert!(!unsynced(VIDEO, 5 * s));
    assert!(!unsynced(VIDEO, -5 * s));
    assert!(unsynced(VIDEO, 5 * s + 1));
    assert!(unsynced(VIDEO, -5 * s - 1));
    assert!(unsynced(VIDEO, i64::MAX));
    assert!(unsynced(VIDEO, i64::MIN + 1));

    // the video stood still for more than 100 ms
    assert!(!stalled(0));
    assert!(!stalled(100 * MS - 1));
    assert!(!stalled(100 * MS));
    assert!(stalled(100 * MS + 1));
    assert!(stalled(u64::MAX));
}

/// `steps_alone`'s verdict checks.
fn steps_alone_verdict() {
    let ms = MS as i64;

    // the verdict: only a good difference holds or drags
    assert_eq!(judge(false, 75 * ms + 1), Verdict::Hold);
    assert_eq!(judge(false, 75 * ms), Verdict::Play);
    assert_eq!(judge(false, -75 * ms), Verdict::Play);
    assert_eq!(judge(false, -75 * ms - 1), Verdict::Drag);
    assert_eq!(judge(false, i64::MAX), Verdict::Hold);
    assert_eq!(judge(false, i64::MIN), Verdict::Drag);
    assert_eq!(judge(true, 75 * ms + 1), Verdict::Play);
    assert_eq!(judge(true, -75 * ms - 1), Verdict::Play);
    assert_eq!(judge(true, 0), Verdict::Play);
}

#[test]
fn steps_steps_alone() {
    steps_alone_diff();
    steps_alone_gate();
    steps_alone_verdict();
}

#[test]
fn steps_stall_asked_only_when_needed() {
    STALL_ASKS.set(0);
    let _ = judge4(VIDEO, 0, 0, 0);
    let _ = judge4(VIDEO + 5 * S + 1, 0, VIDEO, 0);
    let _ = judge4(VIDEO - 5 * S - 1, 0, VIDEO, 0);
    assert_eq!(STALL_ASKS.get(), 0);
    let _ = judge4(VIDEO + 5 * S, 0, VIDEO, 0);
    let _ = judge4(VIDEO, 0, VIDEO, 200 * MS);
    assert_eq!(STALL_ASKS.get(), 2);
}

#[test]
fn steps_judge_thresholds() {
    let ms = MS as i64;
    let s = S as i64;
    let cases = [
        (0, Verdict::Play),
        (1, Verdict::Play),
        (-1, Verdict::Play),
        (75 * ms - 1, Verdict::Play),
        (75 * ms, Verdict::Play),
        (75 * ms + 1, Verdict::Hold),
        (-75 * ms + 1, Verdict::Play),
        (-75 * ms, Verdict::Play),
        (-75 * ms - 1, Verdict::Drag),
        (s, Verdict::Hold),
        (-s, Verdict::Drag),
        (5 * s - 1, Verdict::Hold),
        (5 * s, Verdict::Hold),
        (5 * s + 1, Verdict::Play),
        (-5 * s + 1, Verdict::Drag),
        (-5 * s, Verdict::Drag),
        (-5 * s - 1, Verdict::Play),
        (-(VIDEO as i64) + 1, Verdict::Play),
        (60 * s, Verdict::Play),
    ];
    for (diff, want) in cases {
        assert_eq!(judge_at(diff), want, "difference {diff}");
    }
}

#[test]
fn steps_judge_without_video() {
    for ts in [0, 1, 75 * MS + 1, S, 5 * S, 5 * S + 1, 60 * S] {
        let j = judge4(ts, 0, 0, 0);
        assert_eq!(j.verdict, Verdict::Play, "no video {ts}");
        assert_eq!(j.front_ts, ts);
        assert_eq!(j.diff, ts as i64);
    }
}

#[test]
fn steps_judge_stalled_video() {
    for stall in [0, 1, 100 * MS - 1, 100 * MS] {
        assert_eq!(
            judge4(VIDEO + S, 0, VIDEO, stall).verdict,
            Verdict::Hold,
            "syncing, ahead {stall}"
        );
        assert_eq!(
            judge4(VIDEO - S, 0, VIDEO, stall).verdict,
            Verdict::Drag,
            "syncing, behind {stall}"
        );
    }
    for stall in [100 * MS + 1, 300 * MS, u64::MAX] {
        assert_eq!(
            judge4(VIDEO + S, 0, VIDEO, stall).verdict,
            Verdict::Play,
            "stalled, ahead {stall}"
        );
        let j = judge4(VIDEO - S, 0, VIDEO, stall);
        assert_eq!(j.verdict, Verdict::Play, "stalled, behind {stall}");
        assert_eq!(j.front_ts, VIDEO - S);
        assert_eq!(j.diff, -(S as i64));
    }
}

#[test]
fn steps_judge_padding() {
    let pad = 100 * MS;

    let j = judge4(VIDEO + 75 * MS + 1 + pad, pad, VIDEO, 0);
    assert_eq!(j.verdict, Verdict::Hold, "padding, ahead");
    assert_eq!(j.front_ts, VIDEO + 75 * MS + 1);
    assert_eq!(j.diff, 75 * MS as i64 + 1);
    assert_eq!(
        judge4(VIDEO + 75 * MS + pad, pad, VIDEO, 0).verdict,
        Verdict::Play,
        "padding, at the edge"
    );
    assert_eq!(
        judge4(VIDEO + pad, pad, VIDEO, 0).verdict,
        Verdict::Play,
        "padding, in step"
    );
    let j = judge4(VIDEO + 50 * MS, pad, VIDEO, 0);
    assert_eq!(j.verdict, Verdict::Play, "padding, behind within 75 ms");
    assert_eq!(j.diff, -50 * MS as i64);
    let j = judge4(VIDEO, pad, VIDEO, 0);
    assert_eq!(j.verdict, Verdict::Drag, "padding, behind");
    assert_eq!(j.front_ts, VIDEO - pad);
    assert_eq!(j.diff, -(pad as i64));
}

/// Characterized, not endorsed: a padding longer than the packet's timestamp
/// wraps the u64 subtraction, and the front timestamp is negative once cast.
#[test]
fn steps_judge_padding_wraps() {
    let j = judge4(1000, 2000, 500 * MS, 0);
    assert_eq!(j.verdict, Verdict::Drag, "wrapped, video near");
    assert_eq!(j.front_ts, u64::MAX - 999);
    assert_eq!(j.diff, -1000 - 500 * MS as i64);

    let j = judge4(1000, 2000, VIDEO, 0);
    assert_eq!(j.verdict, Verdict::Play, "wrapped, video far");
    assert_eq!(j.diff, -1000 - VIDEO as i64);

    let j = judge4(1000, 2000, 1, 0);
    assert_eq!(j.verdict, Verdict::Play, "wrapped, video at 1 ns");
    assert_eq!(j.diff, -1001);
}

#[test]
fn steps_judge_large_timestamps() {
    let j = judge4(u64::MAX, 0, 1, 0);
    assert_eq!(j.verdict, Verdict::Play, "timestamp UINT64_MAX");
    assert_eq!(j.diff, -2);
    assert_eq!(
        judge4(u64::MAX - 100 * MS + 1, 0, 1, 0).verdict,
        Verdict::Drag,
        "timestamp 100 ms before 0"
    );
    let j = judge4(i64::MAX as u64, 0, i64::MAX as u64 - S, 0);
    assert_eq!(j.verdict, Verdict::Hold, "timestamp INT64_MAX");
    assert_eq!(j.diff, S as i64);
}

/// The C test's `stand_in_loop()`: `process_audio_delay()`'s loop over the
/// core, as `wasapi-output.c` asks it. A hold returns with the queue kept
/// (`None`), a drag drops the packet only while more are queued after it,
/// and play returns the packet. `head` is the first packet still queued.
fn stand_in_loop(
    queue: &[u64],
    head: &mut usize,
    pad_ns: u64,
    last_frame_ts: u64,
    time_since_prev: u64,
) -> Option<usize> {
    while let Some(&ts) = queue.get(*head) {
        let verdict = judge4(ts, pad_ns, last_frame_ts, time_since_prev).verdict;
        if verdict == Verdict::Hold {
            return None;
        }
        let popped = *head;
        *head += 1;
        if verdict == Verdict::Drag && *head < queue.len() {
            continue;
        }
        return Some(popped);
    }
    unreachable!("the C loop starts with a packet queued");
}

#[test]
fn steps_stand_in_queue() {
    // 20 ms packets from 300 ms behind the video to 200 ms ahead
    let queue: Vec<u64> = (0..26).map(|i| VIDEO - 300 * MS + i * 20 * MS).collect();

    // behind: the packets more than 75 ms behind are dropped, and the first
    // one within it plays
    let mut head = 0;
    assert_eq!(stand_in_loop(&queue, &mut head, 0, VIDEO, 0), Some(12));
    assert_eq!(head, 13);
    assert_eq!(queue[12], VIDEO - 60 * MS);
    // then each packet in turn, up to 75 ms ahead
    for i in 13..=18 {
        assert_eq!(stand_in_loop(&queue, &mut head, 0, VIDEO, 0), Some(i));
    }
    // 80 ms ahead: held, the queue kept
    assert_eq!(stand_in_loop(&queue, &mut head, 0, VIDEO, 0), None);
    assert_eq!(head, 19);
    // the video moves on: it plays
    assert_eq!(
        stand_in_loop(&queue, &mut head, 0, VIDEO + 20 * MS, 0),
        Some(19)
    );

    // the last packet plays even when it drags
    head = 0;
    assert_eq!(stand_in_loop(&queue[..1], &mut head, 0, VIDEO, 0), Some(0));
    assert_eq!(head, 1);

    // the padding moves every packet back: with 100 ms of it, 80 ms ahead is
    // behind
    head = 19;
    assert_eq!(
        stand_in_loop(&queue, &mut head, 100 * MS, VIDEO, 0),
        Some(19)
    );

    // stalled video, or none: no packet is dropped or held
    head = 0;
    assert_eq!(
        stand_in_loop(&queue, &mut head, 0, VIDEO, 100 * MS + 1),
        Some(0)
    );
    head = 25;
    assert_eq!(
        stand_in_loop(&queue, &mut head, 0, VIDEO, 100 * MS + 1),
        Some(25)
    );
    head = 0;
    assert_eq!(stand_in_loop(&queue, &mut head, 0, 0, 0), Some(0));
}

// ---- edge cases against the model ----

/// Values around every threshold, 0 and the ends of u64 and i64.
fn edges() -> Vec<u64> {
    let mut v = Vec::new();
    for base in [
        0,
        STALE_NS,
        VIDEO_STALL_NS,
        SYNC_NS as u64,
        MAX_DIFF_NS as u64,
        VIDEO,
        i64::MAX as u64,
        u64::MAX,
    ] {
        for d in -3i64..=3 {
            v.push(base.wrapping_add_signed(d));
        }
    }
    v.sort_unstable();
    v.dedup();
    v
}

#[test]
fn edge_stale_against_c() {
    for &last in &edges() {
        for &step in &edges() {
            let cur = last.wrapping_add(step);
            let (mut a, mut b) = (last, last);
            assert_eq!(stale(&mut a, cur), c_stale(&mut b, cur), "{last} {cur}");
            assert_eq!(a, b);
            assert_eq!(a, cur);
        }
    }
}

#[test]
fn edge_judge_against_c() {
    let e = edges();
    let diffs: Vec<i64> = [0, SYNC_NS, MAX_DIFF_NS, S as i64]
        .iter()
        .flat_map(|&t| (-3..=3).flat_map(move |d| [t + d, -t + d]))
        .collect();
    for &video in &e {
        for &diff in &diffs {
            for &pad in &[0, 1, 100 * MS, VIDEO, u64::MAX] {
                for &stall in &[0, VIDEO_STALL_NS, VIDEO_STALL_NS + 1, u64::MAX] {
                    let ts = video.wrapping_add_signed(diff).wrapping_add(pad);
                    let want = c_judge(ts, pad, video, stall);
                    if want.diff == i64::MIN {
                        continue;
                    }
                    assert_eq!(
                        judge4(ts, pad, video, stall),
                        want,
                        "{ts} {pad} {video} {stall}"
                    );
                }
            }
        }
    }
}

#[test]
fn edge_steps_against_c() {
    let diffs: Vec<i64> = [0, SYNC_NS, MAX_DIFF_NS, S as i64, i64::MAX]
        .iter()
        .flat_map(|&t| {
            (-3..=3).flat_map(move |d| [t.wrapping_add(d), t.wrapping_neg().wrapping_add(d)])
        })
        .filter(|&d| d != i64::MIN)
        .collect();
    for &video in &edges() {
        for &d in &diffs {
            assert_eq!(unsynced(video, d), c_unsynced(video, d), "{video} {d}");
        }
    }
    for &stall in &edges() {
        assert_eq!(stalled(stall), c_stalled(stall), "{stall}");
    }
    for &d in diffs.iter().chain(&[i64::MIN]) {
        for bad in [false, true] {
            assert_eq!(judge(bad, d), c_judge_diff(bad, d), "{bad} {d}");
        }
    }
}

/// A difference of `i64::MIN` is undefined in the C (`llabs()`); the core
/// takes its magnitude as 2^63, more than 5 s, so the gate shuts and the
/// audio plays.
#[test]
fn edge_judge_min_difference() {
    assert!(unsynced(1, i64::MIN));
    let j = judge4(i64::MIN as u64, 0, 0, 0);
    assert_eq!(j.verdict, Verdict::Play);
    let j = judge4(0, 0, i64::MIN as u64, 0);
    assert_eq!(j.diff, i64::MIN);
    assert_eq!(j.verdict, Verdict::Play);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(4096)))]

    #[test]
    fn edge_prop_stale(last in any::<u64>(), cur in any::<u64>()) {
        let (mut a, mut b) = (last, last);
        prop_assert_eq!(stale(&mut a, cur), c_stale(&mut b, cur));
        prop_assert_eq!(a, b);
    }

    #[test]
    fn edge_prop_judge(
        ts in any::<u64>(),
        pad in prop_oneof![0u64..=2 * S, any::<u64>()],
        video in prop_oneof![Just(0u64), 1u64..=1 << 50, any::<u64>()],
        stall in prop_oneof![0u64..=2 * VIDEO_STALL_NS, any::<u64>()],
    ) {
        let want = c_judge(ts, pad, video, stall);
        prop_assume!(want.diff != i64::MIN);
        prop_assert_eq!(judge4(ts, pad, video, stall), want);
    }

    #[test]
    fn edge_prop_judge_near(
        video in 1u64..=1 << 50,
        diff in -6 * S as i64..=6 * S as i64,
        pad in 0u64..=S,
        stall in 0u64..=2 * VIDEO_STALL_NS,
    ) {
        let ts = video.wrapping_add_signed(diff).wrapping_add(pad);
        prop_assert_eq!(judge4(ts, pad, video, stall), c_judge(ts, pad, video, stall));
    }
}
