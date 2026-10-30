//! Tier 3 parity: the shims of `obs_output::ffi::output_pause` (and the safe
//! core under them) against the original `libobs/obs-output-pause.c.h`,
//! compiled as `oracle_obs_output_pause_*` (obs-c-oracle,
//! oracle/output_pause.c).
//!
//! Every call runs the shim, the C and the safe core, and compares the
//! results exactly where the C is defined and the port keeps its behaviour.
//!
//! Runs: exhaustive over every low 16-bit flag word under 16 high words and
//! a stride through all of `u32` for the flag gate, and every pair for the
//! "already" gate; every slot layout of the first two video slots and the
//! six audio slots over an empty slot and three encoders (65,536 layouts,
//! repeats included); every record rule and offset over 14 edge timestamps
//! cubed; the pause time over those timestamps for 12 intervals; whole
//! pauses and unpauses (a copy of `obs_encoded_output_pause()` and
//! `obs_raw_output_pause()` on stand-in records, the C and the shim side by
//! side) for every layout of the first video slot and three audio slots
//! over three encoders, with every record state of each encoder; and
//! proptests over full-range values, slot layouts of all 16 slots and call
//! sequences.
//!
//! Intentional differences, checked on the shim and the core against the C
//! where the C is defined, and on the shim and the core alone where it is
//! not:
//! - one encoder in several slots is marked once, at its first slot, video
//!   slots first, where the C marks every slot with an encoder (and
//!   `obs_encoded_output_pause()` then locks that encoder's pause mutex
//!   again, which hangs). Every layout's marks equal the C's for the same
//!   layout with the repeats emptied, and a whole pause on it equals the
//!   C's on that layout;
//! - a frame interval of 0 gives the clock as the pause time (the C divides
//!   by it: undefined, never called).
//!
//! `test_output_pause_steps` runs the C test against the C; the real-libobs
//! output tests run against the hybrid libobs, where these shims replace
//! the C.

#![allow(clippy::as_conversions, clippy::cast_possible_truncation)]

use obs_c_oracle::output_pause::{
    oracle_obs_output_pause_allowed, oracle_obs_output_pause_already,
    oracle_obs_output_pause_closest_ts, oracle_obs_output_pause_ended,
    oracle_obs_output_pause_lock_set, oracle_obs_output_pause_offset,
    oracle_obs_output_pause_started,
};
use obs_output::ffi::output_pause::{
    obs_output_pause_allowed, obs_output_pause_already, obs_output_pause_closest_ts,
    obs_output_pause_ended, obs_output_pause_lock_set, obs_output_pause_offset,
    obs_output_pause_started,
};
use obs_output::output_pause::{self as core, LockSet, MAX_AUDIO_ENCODERS, MAX_VIDEO_ENCODERS};
use proptest::prelude::*;

const NUM_VIDEO: usize = MAX_VIDEO_ENCODERS;
const NUM_AUDIO: usize = MAX_AUDIO_ENCODERS;

/// 60 fps: `video_frame_interval_ns` for 60/1.
const FRAME: u64 = 16_666_666;

/// Timestamps at the edges: 0, small, around a realistic clock and the
/// frame, and near `u64::MAX`.
const EDGES: [u64; 14] = [
    0,
    1,
    2,
    3,
    FRAME - 1,
    FRAME,
    FRAME + 1,
    2 * FRAME,
    5_000_000_000,
    5_000_000_000 + 7 * FRAME,
    1 << 63,
    u64::MAX - 2,
    u64::MAX - 1,
    u64::MAX,
];

/// Frame intervals the C can divide by: 1 ns, the usual rates, and odd
/// and huge ones.
const INTERVALS: [u64; 12] = [
    1,
    2,
    3,
    FRAME,
    16_666_667,
    33_333_333,
    41_708_333,
    1_000_000_000,
    1 << 62,
    (1 << 63) + 1,
    u64::MAX - 1,
    u64::MAX,
];

fn c_lock_set(video: &[usize; NUM_VIDEO], audio: &[usize; NUM_AUDIO]) -> LockSet {
    let (mut v, mut a) = (0xdead, 0xbeef);
    // SAFETY: two arrays of the C's lengths and two locals to write.
    unsafe { oracle_obs_output_pause_lock_set(video, audio, &raw mut v, &raw mut a) };
    LockSet { video: v, audio: a }
}

fn shim_lock_set(video: &[usize; NUM_VIDEO], audio: &[usize; NUM_AUDIO]) -> LockSet {
    let (mut v, mut a) = (0xdead, 0xbeef);
    // SAFETY: as above.
    unsafe { obs_output_pause_lock_set(video, audio, &raw mut v, &raw mut a) };
    LockSet { video: v, audio: a }
}

/// The C's pause time, where the C is defined (an interval that is not 0).
fn c_closest(last: u64, interval: u64, now: u64) -> u64 {
    assert_ne!(interval, 0);
    // SAFETY: plain values; the interval is not 0.
    unsafe { oracle_obs_output_pause_closest_ts(last, interval, now) }
}

/// The layout with every repeat of an encoder emptied, video slots first:
/// what the C would mark the way the port does.
fn dedup(
    video: &[usize; NUM_VIDEO],
    audio: &[usize; NUM_AUDIO],
) -> ([usize; NUM_VIDEO], [usize; NUM_AUDIO]) {
    let mut seen = Vec::new();
    let mut keep = |id: usize| {
        if id == 0 || seen.contains(&id) {
            0
        } else {
            seen.push(id);
            id
        }
    };
    (video.map(&mut keep), audio.map(&mut keep))
}

fn check_lock_set(video: &[usize; NUM_VIDEO], audio: &[usize; NUM_AUDIO]) {
    let shim = shim_lock_set(video, audio);
    assert_eq!(
        shim,
        core::lock_set(video, audio),
        "core, {video:?} {audio:?}"
    );
    let (dv, da) = dedup(video, audio);
    // The port's marks are the C's on the layout without repeats.
    assert_eq!(shim, c_lock_set(&dv, &da), "{video:?} {audio:?}");
    if (dv, da) == (*video, *audio) {
        assert_eq!(shim, c_lock_set(video, audio), "{video:?} {audio:?}");
    }
}

fn check_closest(last: u64, interval: u64, now: u64) {
    let shim = obs_output_pause_closest_ts(last, interval, now);
    assert_eq!(shim, core::closest_ts(last, interval, now));
    if interval == 0 {
        // Intentional: the C divides by 0.
        assert_eq!(shim, now, "{last} {now}");
    } else {
        assert_eq!(
            shim,
            c_closest(last, interval, now),
            "{last} {interval} {now}"
        );
    }
}

fn check_record(start: u64, end: u64, offset: u64) {
    // SAFETY (each call below): plain values.
    unsafe {
        assert_eq!(
            obs_output_pause_started(start),
            oracle_obs_output_pause_started(start)
        );
        assert_eq!(
            obs_output_pause_ended(end),
            oracle_obs_output_pause_ended(end)
        );
        assert_eq!(
            obs_output_pause_offset(offset, start, end),
            oracle_obs_output_pause_offset(offset, start, end),
            "{offset} {start} {end}"
        );
    }
    assert_eq!(obs_output_pause_started(start), core::started(start));
    assert_eq!(obs_output_pause_ended(end), core::ended(end));
    // The core's joins are the C's: `pause_can_start()` and
    // `pause_can_stop()` over the C's field tests.
    assert_eq!(core::can_start(start, end), can_start(&C, start, end));
    assert_eq!(core::can_stop(start, end), can_stop(&C, start, end));
    assert_eq!(
        obs_output_pause_offset(offset, start, end),
        core::offset(offset, start, end)
    );
}

// ---------------------------------------------------------------------------
// whole pauses on stand-in records

/// An encoder's (or a raw output's) pause record and `paused` flag, and how
/// often its pause mutex was taken and given back.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Rec {
    last_video_ts: u64,
    ts_start: u64,
    ts_end: u64,
    ts_offset: u64,
    paused: bool,
    locks: u32,
    unlocks: u32,
}

/// One side's functions: the C's or the shims'.
struct Plan {
    lock_set: fn(&[usize; NUM_VIDEO], &[usize; NUM_AUDIO]) -> LockSet,
    closest: fn(u64, u64, u64) -> u64,
    started: fn(u64) -> bool,
    ended: fn(u64) -> bool,
    offset: fn(u64, u64, u64) -> u64,
}

fn c_started(s: u64) -> bool {
    // SAFETY: plain values.
    unsafe { oracle_obs_output_pause_started(s) }
}
fn c_ended(e: u64) -> bool {
    // SAFETY: plain values.
    unsafe { oracle_obs_output_pause_ended(e) }
}
fn c_offset(o: u64, s: u64, e: u64) -> u64 {
    // SAFETY: plain values.
    unsafe { oracle_obs_output_pause_offset(o, s, e) }
}

const C: Plan = Plan {
    lock_set: c_lock_set,
    closest: c_closest,
    started: c_started,
    ended: c_ended,
    offset: c_offset,
};

const SHIM: Plan = Plan {
    lock_set: shim_lock_set,
    closest: |l, i, n| obs_output_pause_closest_ts(l, i, n),
    started: |s| obs_output_pause_started(s),
    ended: |e| obs_output_pause_ended(e),
    offset: |o, s, e| obs_output_pause_offset(o, s, e),
};

/// `pause_can_start()`, as `obs-output.c` joins the field tests.
fn can_start(p: &Plan, start: u64, end: u64) -> bool {
    !(p.started)(start) && !(p.ended)(end)
}

/// `pause_can_stop()`, as `obs-output.c` joins the field tests.
fn can_stop(p: &Plan, start: u64, end: u64) -> bool {
    (p.started)(start) && !(p.ended)(end)
}

fn end_pause(p: &Plan, r: &mut Rec, ts: u64) {
    if !(p.ended)(r.ts_end) {
        r.ts_end = ts;
        r.ts_offset = (p.offset)(r.ts_offset, r.ts_start, r.ts_end);
    }
}

/// `obs_encoded_output_pause()` on the records in `recs`; the slots hold an
/// encoder's id, its index in `recs` plus 1, or 0.
fn encoded_pause(
    p: &Plan,
    recs: &mut [Rec],
    video: &[usize; NUM_VIDEO],
    audio: &[usize; NUM_AUDIO],
    interval: u64,
    now: u64,
    pause: bool,
) -> bool {
    let LockSet {
        video: vlock,
        audio: alock,
    } = (p.lock_set)(video, audio);
    let order: Vec<usize> = (0..NUM_VIDEO)
        .filter(|&i| vlock & (1 << i) != 0)
        .map(|i| video[i] - 1)
        .chain(
            (0..NUM_AUDIO)
                .filter(|&i| alock & (1 << i) != 0)
                .map(|i| audio[i] - 1),
        )
        .collect();
    for &r in &order {
        recs[r].locks += 1;
    }
    let success = 'plan: {
        let Some(&first) = video.iter().find(|&&id| id != 0) else {
            break 'plan false;
        };
        let ts = (p.closest)(recs[first - 1].last_video_ts, interval, now);
        let can = if pause { can_start } else { can_stop };
        if !order
            .iter()
            .all(|&r| can(p, recs[r].ts_start, recs[r].ts_end))
        {
            break 'plan false;
        }
        for &r in &order {
            recs[r].paused = pause;
            if pause {
                recs[r].ts_start = ts;
            } else {
                end_pause(p, &mut recs[r], ts);
            }
        }
        true
    };
    for &r in order.iter().rev() {
        recs[r].unlocks += 1;
    }
    success
}

/// `obs_raw_output_pause()` on the output's own record.
fn raw_pause(p: &Plan, r: &mut Rec, interval: u64, now: u64, pause: bool) -> bool {
    let ts = (p.closest)(r.last_video_ts, interval, now);
    let can = if pause { can_start } else { can_stop };
    let success = can(p, r.ts_start, r.ts_end);
    if success {
        if pause {
            r.ts_start = ts;
        } else {
            end_pause(p, r, ts);
        }
    }
    success
}

/// A record in one of its four states (clear, started, ended, an end with
/// no start), with an offset and a last video timestamp.
fn record(state: usize, last: u64) -> Rec {
    let (ts_start, ts_end) = [
        (0, 0),
        (3 * FRAME, 0),
        (3 * FRAME, 9 * FRAME),
        (0, 9 * FRAME),
    ][state];
    Rec {
        last_video_ts: last,
        ts_start,
        ts_end,
        ts_offset: 11,
        ..Rec::default()
    }
}

/// Runs a whole pause and unpause on both sides and compares every record
/// and result: the C on the layout without repeats, the shims on the layout
/// itself.
fn check_sequence(
    recs: &[Rec],
    video: &[usize; NUM_VIDEO],
    audio: &[usize; NUM_AUDIO],
    calls: &[(bool, u64)],
    interval: u64,
) {
    let (dv, da) = dedup(video, audio);
    let (mut c, mut s) = (recs.to_vec(), recs.to_vec());
    for &(pause, now) in calls {
        let rc = encoded_pause(&C, &mut c, &dv, &da, interval, now, pause);
        let rs = encoded_pause(&SHIM, &mut s, video, audio, interval, now, pause);
        assert_eq!(rs, rc, "{video:?} {audio:?} {calls:?}");
        assert_eq!(s, c, "{video:?} {audio:?} {calls:?}");
    }
}

// ---------------------------------------------------------------------------
// exhaustive

#[test]
fn gates_match() {
    for high in 0..16_u32 {
        for low in 0..=0xFFFF_u32 {
            let flags = (high << 28) | (high << 16) | low;
            // SAFETY: plain values.
            let c = unsafe { oracle_obs_output_pause_allowed(flags) };
            assert_eq!(obs_output_pause_allowed(flags), c, "{flags:#x}");
            assert_eq!(core::allowed(flags), c);
        }
    }
    // a stride through all of u32
    for flags in (0..=u32::MAX).step_by(65_521) {
        // SAFETY: plain values.
        let c = unsafe { oracle_obs_output_pause_allowed(flags) };
        assert_eq!(obs_output_pause_allowed(flags), c, "{flags:#x}");
    }
    for paused in [false, true] {
        for pause in [false, true] {
            // SAFETY: plain values.
            let c = unsafe { oracle_obs_output_pause_already(paused, pause) };
            assert_eq!(obs_output_pause_already(paused, pause), c);
            assert_eq!(core::already(paused, pause), c);
        }
    }
}

#[test]
fn lock_sets_match_over_every_small_layout() {
    // The first two video slots and the six audio slots, each empty or one
    // of three encoders: 4^8 layouts, repeats included.
    for n in 0..4_usize.pow(8) {
        let digit = |k: usize| (n / 4_usize.pow(k as u32)) % 4;
        let mut video = [0; NUM_VIDEO];
        let mut audio = [0; NUM_AUDIO];
        video[0] = digit(0);
        video[1] = digit(1);
        for (k, a) in audio.iter_mut().enumerate() {
            *a = digit(k + 2);
        }
        check_lock_set(&video, &audio);
        // the same layout further up the video slots
        video.rotate_right(8);
        check_lock_set(&video, &audio);
    }
    // every slot, distinct and all one encoder
    let video: [usize; NUM_VIDEO] = std::array::from_fn(|i| 0x1000 + i);
    let audio: [usize; NUM_AUDIO] = std::array::from_fn(|i| 0x2000 + i);
    check_lock_set(&video, &audio);
    check_lock_set(&[usize::MAX; NUM_VIDEO], &[usize::MAX; NUM_AUDIO]);
}

#[test]
fn records_match_over_the_edges() {
    for &start in &EDGES {
        for &end in &EDGES {
            for &offset in &EDGES {
                check_record(start, end, offset);
            }
        }
    }
}

#[test]
fn pause_times_match_over_the_edges() {
    for &last in &EDGES {
        for &now in &EDGES {
            for &interval in &INTERVALS {
                check_closest(last, interval, now);
            }
            check_closest(last, 0, now);
        }
    }
}

#[test]
fn whole_pauses_match_over_layouts_and_record_states() {
    // The first video slot and three audio slots over three encoders (each
    // slot empty or one of them), every record state of each encoder, and
    // a pause then an unpause.
    let calls = [
        (true, 100 * FRAME + 5),
        (false, 140 * FRAME),
        (true, 150 * FRAME),
    ];
    for n in 0..4_usize.pow(4) {
        let digit = |k: usize| (n / 4_usize.pow(k as u32)) % 4;
        let mut video = [0; NUM_VIDEO];
        let mut audio = [0; NUM_AUDIO];
        video[0] = digit(0);
        audio[0] = digit(1);
        audio[1] = digit(2);
        audio[3] = digit(3);
        for states in 0..4_usize.pow(3) {
            let recs: Vec<Rec> = (0..3)
                .map(|e| record((states / 4_usize.pow(e as u32)) % 4, 90 * FRAME + e as u64))
                .collect();
            check_sequence(&recs, &video, &audio, &calls, FRAME);
        }
    }
}

#[test]
fn raw_pauses_match() {
    for state in 0..4 {
        for &last in &EDGES {
            for &interval in &INTERVALS {
                let (mut c, mut s) = (record(state, last), record(state, last));
                for (pause, now) in [(true, 5_000_000_000), (false, 5_100_000_000), (false, 0)] {
                    let rc = raw_pause(&C, &mut c, interval, now, pause);
                    let rs = raw_pause(&SHIM, &mut s, interval, now, pause);
                    assert_eq!((rs, s), (rc, c), "{state} {last} {interval}");
                }
            }
        }
    }
}

#[test]
fn one_encoder_on_two_tracks_pauses_where_the_c_hangs() {
    // Intentional: one audio encoder (id 2) on tracks 1 and 2. The C marks
    // both slots, so it would lock that encoder's mutex twice.
    let mut video = [0; NUM_VIDEO];
    let mut audio = [0; NUM_AUDIO];
    video[0] = 1;
    audio[0] = 2;
    audio[1] = 2;
    assert_eq!(
        c_lock_set(&video, &audio),
        LockSet {
            video: 1,
            audio: 0b11
        }
    );
    assert_eq!(
        shim_lock_set(&video, &audio),
        LockSet {
            video: 1,
            audio: 0b01
        }
    );
    let mut recs = vec![record(0, 90 * FRAME), record(0, 0)];
    assert!(encoded_pause(
        &SHIM,
        &mut recs,
        &video,
        &audio,
        FRAME,
        90 * FRAME,
        true
    ));
    assert!(encoded_pause(
        &SHIM,
        &mut recs,
        &video,
        &audio,
        FRAME,
        100 * FRAME,
        false
    ));
    assert_eq!((recs[1].locks, recs[1].unlocks), (2, 2));
    assert_eq!(
        (recs[1].ts_start, recs[1].ts_end),
        (92 * FRAME, 102 * FRAME)
    );
}

#[test]
fn an_interval_of_zero_gives_the_clock() {
    // Intentional: the C divides by it.
    for &last in &EDGES {
        for &now in &EDGES {
            check_closest(last, 0, now);
        }
    }
    let mut r = record(0, 0);
    assert!(raw_pause(&SHIM, &mut r, 0, 40, true));
    assert!(raw_pause(&SHIM, &mut r, 0, 90, false));
    assert_eq!((r.ts_start, r.ts_end, r.ts_offset), (40, 90, 61));
}

// ---------------------------------------------------------------------------
// proptests

fn ts() -> impl Strategy<Value = u64> {
    prop_oneof![
        prop::sample::select(EDGES.to_vec()),
        0..1000_u64,
        1_000_000_000..1_000_000_000_000_u64,
        any::<u64>(),
    ]
}

fn slot() -> impl Strategy<Value = usize> {
    prop_oneof![3 => Just(0_usize), 2 => 1..=4_usize]
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(4096),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    #[test]
    fn prop_values_match(
        flags in any::<u32>(),
        last in ts(),
        interval in prop_oneof![prop::sample::select(INTERVALS.to_vec()), any::<u64>()],
        now in ts(),
        start in ts(),
        end in ts(),
        offset in ts(),
    ) {
        // SAFETY: plain values.
        prop_assert_eq!(obs_output_pause_allowed(flags), unsafe { oracle_obs_output_pause_allowed(flags) });
        check_closest(last, interval, now);
        check_record(start, end, offset);
    }

    #[test]
    fn prop_lock_sets_match(
        video in prop::array::uniform10(slot()),
        audio in prop::array::uniform6(slot()),
        ids in prop::array::uniform4(any::<usize>().prop_filter("an encoder", |&id| id != 0)),
    ) {
        // Slots 1 to 4 name four encoders with arbitrary ids (two may be
        // the same id: the same encoder).
        let name = |s: usize| if s == 0 { 0 } else { ids[s - 1] };
        check_lock_set(&video.map(name), &audio.map(name));
    }

    #[test]
    fn prop_whole_pauses_match(
        video in prop::array::uniform10(slot()),
        audio in prop::array::uniform6(slot()),
        states in prop::array::uniform4(0..4_usize),
        lasts in prop::array::uniform4(0..1_000_000_000_000_u64),
        calls in prop::collection::vec((any::<bool>(), 0..2_000_000_000_000_u64), 1..6),
        interval in prop_oneof![Just(FRAME), Just(33_333_333_u64), 1..1_000_000_u64],
    ) {
        let recs: Vec<Rec> = (0..4).map(|e| record(states[e], lasts[e])).collect();
        check_sequence(&recs, &video, &audio, &calls, interval);
    }
}
