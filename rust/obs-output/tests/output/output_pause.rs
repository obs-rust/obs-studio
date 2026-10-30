//! Tier 1: `obs_output::output_pause`, the pause plan of `obs-output.c`.
//!
//! The `steps_*` cases port `test/cmocka/test_output_pause_steps.c` 1:1:
//! its 9 cases under the same names, in the same order, with the same
//! values and expectations, the "characterized, not endorsed" ones
//! included, and its stand-ins for `obs_encoded_output_pause()` and
//! `obs_raw_output_pause()` ([`encoded_pause`], [`raw_pause`]). The C calls
//! the hidden `obs_output_pause_*` functions; here the safe core is called
//! directly.
//!
//! The `edge_*` cases are new: the intentional differences (one encoder in
//! several slots locked and updated once, a frame interval of 0 giving the
//! clock), and proptests of the pause time against a model of the C's
//! expression where the C is defined, of the lock set's marks, of the
//! records' rules and of any input without a panic.

#![allow(clippy::as_conversions, clippy::cast_possible_truncation)]

use obs_output::output_pause::{
    LockSet, MAX_AUDIO_ENCODERS, MAX_VIDEO_ENCODERS, OBS_OUTPUT_CAN_PAUSE, allowed, already,
    can_start, can_stop, closest_ts, ended, lock_set, offset, started,
};
use proptest::prelude::*;

/// 60 fps: `video_frame_interval_ns` for 60/1.
const FRAME: u64 = 16_666_666;

const NUM_VIDEO: usize = MAX_VIDEO_ENCODERS;
const NUM_AUDIO: usize = MAX_AUDIO_ENCODERS;

/// An encoder's (or a raw output's) `struct pause_data` and `paused` flag,
/// and how often its pause mutex was taken and given back.
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

impl Rec {
    fn last(last_video_ts: u64) -> Self {
        Self {
            last_video_ts,
            ..Self::default()
        }
    }
}

/// Encoders by index into a record table: the slots hold `Some(index)`.
type Slots<const N: usize> = [Option<usize>; N];

fn end_pause(r: &mut Rec, ts: u64) {
    if !ended(r.ts_end) {
        r.ts_end = ts;
        r.ts_offset = offset(r.ts_offset, r.ts_start, r.ts_end);
    }
}

fn marked(mask: u32, i: usize) -> bool {
    mask & (1 << i) != 0
}

/// The opaque id of a slot: its record's index plus 1, 0 for an empty slot.
fn ids<const N: usize>(slots: &Slots<N>) -> [usize; N] {
    slots.map(|s| s.map_or(0, |i| i + 1))
}

/// `obs_encoded_output_pause()`, on the records in `recs`, at clock `now`
/// with frames of `interval` ns.
fn encoded_pause(
    recs: &mut [Rec],
    venc: &Slots<NUM_VIDEO>,
    aenc: &Slots<NUM_AUDIO>,
    interval: u64,
    now: u64,
    pause: bool,
) -> bool {
    let LockSet {
        video: vlock,
        audio: alock,
    } = lock_set(&ids(venc), &ids(aenc));
    // The marked slots in lock order, each with its record.
    let order: Vec<usize> = (0..NUM_VIDEO)
        .filter(|&i| marked(vlock, i))
        .map(|i| venc[i].unwrap())
        .chain(
            (0..NUM_AUDIO)
                .filter(|&i| marked(alock, i))
                .map(|i| aenc[i].unwrap()),
        )
        .collect();

    for &r in &order {
        recs[r].locks += 1;
    }

    let success = 'plan: {
        // get_first_video_encoder_index()
        let Some(first) = venc.iter().flatten().next() else {
            break 'plan false;
        };
        let ts = closest_ts(recs[*first].last_video_ts, interval, now);

        if !order.iter().all(|&r| {
            if pause {
                can_start(recs[r].ts_start, recs[r].ts_end)
            } else {
                can_stop(recs[r].ts_start, recs[r].ts_end)
            }
        }) {
            break 'plan false;
        }

        for &r in &order {
            recs[r].paused = pause;
            if pause {
                recs[r].ts_start = ts;
            } else {
                end_pause(&mut recs[r], ts);
            }
        }
        true
    };

    for &r in order.iter().rev() {
        recs[r].unlocks += 1;
    }
    success
}

/// `obs_raw_output_pause()`, on the output's own record.
fn raw_pause(r: &mut Rec, interval: u64, now: u64, pause: bool) -> bool {
    let ts = closest_ts(r.last_video_ts, interval, now);
    if pause {
        let success = can_start(r.ts_start, r.ts_end);
        if success {
            r.ts_start = ts;
        }
        success
    } else {
        let success = can_stop(r.ts_start, r.ts_end);
        if success {
            end_pause(r, ts);
        }
        success
    }
}

fn check_rec(what: &str, r: &Rec, start: u64, end: u64, offset: u64) {
    assert_eq!(
        (r.ts_start, r.ts_end, r.ts_offset),
        (start, end, offset),
        "{what}: start, end, offset"
    );
}

fn video_slots(slots: &[(usize, usize)]) -> Slots<NUM_VIDEO> {
    let mut v = [None; NUM_VIDEO];
    for &(slot, rec) in slots {
        v[slot] = Some(rec);
    }
    v
}

fn audio_slots(slots: &[(usize, usize)]) -> Slots<NUM_AUDIO> {
    let mut a = [None; NUM_AUDIO];
    for &(slot, rec) in slots {
        a[slot] = Some(rec);
    }
    a
}

// ---------------------------------------------------------------------------
// obs_output_pause() gates

#[test]
fn steps_only_an_output_that_can_pause() {
    assert!(allowed(OBS_OUTPUT_CAN_PAUSE));
    assert!(allowed(0xFFFF_FFFF));
    // video, audio, encoded and service, with and without it
    assert!(allowed(0x0F | OBS_OUTPUT_CAN_PAUSE));
    assert!(!allowed(0x0F));
    assert!(!allowed(0));
    assert!(!allowed(!OBS_OUTPUT_CAN_PAUSE));
}

#[test]
fn steps_already_in_that_state_succeeds() {
    // Pausing a paused output, or unpausing a running one, returns true
    // with nothing done.
    assert!(already(true, true));
    assert!(already(false, false));
    assert!(!already(false, true));
    assert!(!already(true, false));
}

// ---------------------------------------------------------------------------
// the lock set

#[test]
fn steps_every_slot_with_an_encoder_is_locked() {
    let mut video = [0_usize; NUM_VIDEO];
    let mut audio = [0_usize; NUM_AUDIO];

    assert_eq!(lock_set(&video, &audio), LockSet { video: 0, audio: 0 });

    // one video encoder, two audio tracks (the recording's usual set)
    video[0] = 0x1000;
    audio[0] = 0x2000;
    audio[1] = 0x3000;
    assert_eq!(
        lock_set(&video, &audio),
        LockSet {
            video: 0x1,
            audio: 0x3
        }
    );

    // gaps stay unmarked
    video[0] = 0;
    video[3] = 0x1000;
    video[9] = 0x1100;
    audio[1] = 0;
    audio[5] = 0x3000;
    assert_eq!(
        lock_set(&video, &audio),
        LockSet {
            video: (1 << 3) | (1 << 9),
            audio: 1 | (1 << 5)
        }
    );

    // every slot, each a different encoder
    for (i, v) in video.iter_mut().enumerate() {
        *v = 0x1000 + i;
    }
    for (i, a) in audio.iter_mut().enumerate() {
        *a = 0x2000 + i;
    }
    assert_eq!(
        lock_set(&video, &audio),
        LockSet {
            video: 0x3FF,
            audio: 0x3F
        }
    );
}

// ---------------------------------------------------------------------------
// the pause time

#[test]
fn steps_the_pause_time_snaps_to_the_frame_grid() {
    let last: u64 = 5_000_000_000;

    // no frame yet: two frames on from 0
    assert_eq!(closest_ts(0, FRAME, 0), 2 * FRAME);
    assert_eq!(closest_ts(0, FRAME, FRAME - 1), 2 * FRAME);
    assert_eq!(closest_ts(0, FRAME, FRAME), 3 * FRAME);

    // a clock on the grid: two frames after it
    assert_eq!(closest_ts(last, FRAME, last), last + 2 * FRAME);
    assert_eq!(closest_ts(last, FRAME, last + 5 * FRAME), last + 7 * FRAME);

    // one ns past a frame, or one before the next: truncated to the frame,
    // then two on
    assert_eq!(
        closest_ts(last, FRAME, last + 5 * FRAME + 1),
        last + 7 * FRAME
    );
    assert_eq!(
        closest_ts(last, FRAME, last + 6 * FRAME - 1),
        last + 7 * FRAME
    );

    // a clock a frame behind the last frame: the frame after it
    assert_eq!(closest_ts(last, FRAME, last - FRAME), last + FRAME);

    // other rates: 30 fps and 1 ns frames
    assert_eq!(
        closest_ts(last, 33_333_333, last + 100_000_000),
        last + 5 * 33_333_333
    );
    assert_eq!(closest_ts(7, 1, 10), 12);
}

// ---------------------------------------------------------------------------
// the records

#[test]
fn steps_a_record_starts_and_ends_a_pause() {
    // clear: can start, not stop
    assert!(can_start(0, 0));
    assert!(!can_stop(0, 0));
    // started: can stop, not start
    assert!(!can_start(10, 0));
    assert!(can_stop(10, 0));
    // ended (not cleared yet): neither
    assert!(!can_start(10, 20));
    assert!(!can_stop(10, 20));
    // an end with no start: neither (characterized, not endorsed: never set
    // that way)
    assert!(!can_start(0, 20));
    assert!(!can_stop(0, 20));

    // each field on its own; the end is set once
    assert!(!started(0));
    assert!(started(1));
    assert!(started(u64::MAX));
    assert!(!ended(0));
    assert!(ended(1));
    assert!(ended(u64::MAX));

    // the offset grows by the pause's length
    assert_eq!(offset(0, 100, 350), 250);
    assert_eq!(offset(250, 1000, 1500), 750);
    assert_eq!(offset(7, 5, 5), 7);
}

// ---------------------------------------------------------------------------
// scenarios

#[test]
fn steps_pause_and_unpause_one_video_two_audio() {
    let (v, a1, a2) = (0, 1, 2);
    let mut recs = [Rec::last(1000 * FRAME), Rec::last(77), Rec::default()];
    let venc = video_slots(&[(0, v)]);
    let aenc = audio_slots(&[(0, a1), (1, a2)]);

    // Every record gets the first video encoder's grid time.
    let start = 1000 * FRAME + 7 * FRAME;
    assert!(encoded_pause(
        &mut recs,
        &venc,
        &aenc,
        FRAME,
        1000 * FRAME + 5 * FRAME + 3,
        true
    ));
    check_rec("video, paused", &recs[v], start, 0, 0);
    check_rec("audio 1, paused", &recs[a1], start, 0, 0);
    check_rec("audio 2, paused", &recs[a2], start, 0, 0);
    assert!(recs[v].paused && recs[a1].paused && recs[a2].paused);

    // The video encoder moved on while paused.
    recs[v].last_video_ts = 1100 * FRAME;
    let end = 1100 * FRAME + 2 * FRAME;
    assert!(encoded_pause(
        &mut recs,
        &venc,
        &aenc,
        FRAME,
        1100 * FRAME,
        false
    ));
    check_rec("video, unpaused", &recs[v], start, end, end - start);
    check_rec("audio 1, unpaused", &recs[a1], start, end, end - start);
    check_rec("audio 2, unpaused", &recs[a2], start, end, end - start);
    assert!(!(recs[v].paused || recs[a1].paused || recs[a2].paused));

    // Each mutex taken and given back once per call.
    assert_eq!(recs[v].locks, 2);
    assert_eq!(recs[v].unlocks, 2);
    assert_eq!(recs[a1].locks, 2);
    assert_eq!(recs[a2].unlocks, 2);

    // Until the encoders clear their records, a second pause fails.
    assert!(!encoded_pause(
        &mut recs,
        &venc,
        &aenc,
        FRAME,
        1200 * FRAME,
        true
    ));
    check_rec("video, again", &recs[v], start, end, end - start);

    // Cleared with the offset kept (as the encoders do): the next pause adds
    // to it.
    for r in &mut recs {
        r.ts_start = 0;
        r.ts_end = 0;
    }
    recs[v].last_video_ts = 1200 * FRAME;
    assert!(encoded_pause(
        &mut recs,
        &venc,
        &aenc,
        FRAME,
        1200 * FRAME,
        true
    ));
    assert!(encoded_pause(
        &mut recs,
        &venc,
        &aenc,
        FRAME,
        1300 * FRAME,
        false
    ));
    check_rec(
        "video, second",
        &recs[v],
        1202 * FRAME,
        1302 * FRAME,
        end - start + 100 * FRAME,
    );
}

#[test]
fn steps_one_record_not_clear_fails_the_whole_pause() {
    let (v, a1, a2) = (0, 1, 2);
    let mut recs = [
        Rec::last(10 * FRAME),
        Rec::default(),
        Rec {
            ts_start: 3 * FRAME,
            ts_end: 4 * FRAME,
            ts_offset: FRAME,
            ..Rec::default()
        },
    ];
    let venc = video_slots(&[(0, v)]);
    let aenc = audio_slots(&[(0, a1), (1, a2)]);

    assert!(!encoded_pause(
        &mut recs,
        &venc,
        &aenc,
        FRAME,
        12 * FRAME,
        true
    ));
    check_rec("video", &recs[v], 0, 0, 0);
    check_rec("audio 1", &recs[a1], 0, 0, 0);
    check_rec("audio 2", &recs[a2], 3 * FRAME, 4 * FRAME, FRAME);
    assert!(!(recs[v].paused || recs[a1].paused || recs[a2].paused));
    // every lock still given back
    assert_eq!(recs[v].unlocks, 1);
    assert_eq!(recs[a2].unlocks, 1);

    // unpause with one record never started: fails too
    recs[a2] = Rec::default();
    recs[v].ts_start = 5 * FRAME;
    recs[a1].ts_start = 5 * FRAME;
    assert!(!encoded_pause(
        &mut recs,
        &venc,
        &aenc,
        FRAME,
        12 * FRAME,
        false
    ));
    check_rec("video, unpause", &recs[v], 5 * FRAME, 0, 0);
}

#[test]
fn steps_audio_encoders_only_never_pause() {
    // Characterized, not endorsed: with no video encoder there is no clock
    // to snap to, so an encoded audio-only output can't pause.
    let mut recs = [Rec::default()];
    let venc = [None; NUM_VIDEO];
    let aenc = audio_slots(&[(0, 0)]);

    assert!(!encoded_pause(
        &mut recs,
        &venc,
        &aenc,
        FRAME,
        12 * FRAME,
        true
    ));
    check_rec("audio", &recs[0], 0, 0, 0);
    assert_eq!(recs[0].locks, 1);
    assert_eq!(recs[0].unlocks, 1);
}

#[test]
fn steps_a_raw_output_pauses_on_its_own_record() {
    let mut r = Rec {
        last_video_ts: 50 * FRAME,
        ts_offset: 3,
        ..Rec::default()
    };

    assert!(!raw_pause(&mut r, FRAME, 51 * FRAME, false));
    assert!(raw_pause(&mut r, FRAME, 51 * FRAME, true));
    check_rec("paused", &r, 53 * FRAME, 0, 3);
    assert!(!raw_pause(&mut r, FRAME, 52 * FRAME, true));
    assert!(raw_pause(&mut r, FRAME, 60 * FRAME, false));
    check_rec("unpaused", &r, 53 * FRAME, 62 * FRAME, 3 + 9 * FRAME);
    assert!(!raw_pause(&mut r, FRAME, 70 * FRAME, false));
    check_rec("unpaused twice", &r, 53 * FRAME, 62 * FRAME, 3 + 9 * FRAME);
}

// ---------------------------------------------------------------------------
// intentional differences

#[test]
fn edge_one_encoder_in_two_slots_is_locked_once() {
    // Intentional difference: the C marks both slots, so the encoder's
    // pause mutex is locked twice and the caller hangs.
    let mut video = [0_usize; NUM_VIDEO];
    let mut audio = [0_usize; NUM_AUDIO];
    video[0] = 0x1000;
    audio[0] = 0x2000;
    audio[1] = 0x2000;
    assert_eq!(
        lock_set(&video, &audio),
        LockSet {
            video: 0x1,
            audio: 0x1
        }
    );

    // the first slot wins, gaps and all
    audio[0] = 0;
    audio[3] = 0x2000;
    audio[4] = 0x3000;
    audio[5] = 0x2000;
    assert_eq!(
        lock_set(&video, &audio),
        LockSet {
            video: 0x1,
            audio: (1 << 1) | (1 << 4)
        }
    );

    // two video slots
    video[2] = 0x1000;
    video[5] = 0x1100;
    assert_eq!(lock_set(&video, &audio).video, 1 | (1 << 5));

    // one id in a video and an audio slot (the setters refuse the wrong
    // type, but the ids are opaque): the video slot
    audio[4] = 0x1100;
    assert_eq!(
        lock_set(&video, &audio),
        LockSet {
            video: 1 | (1 << 5),
            audio: 1 << 1
        }
    );

    // every slot the same encoder
    assert_eq!(
        lock_set(&[7; NUM_VIDEO], &[7; NUM_AUDIO]),
        LockSet { video: 1, audio: 0 }
    );
}

#[test]
fn edge_one_audio_encoder_on_two_tracks_pauses_and_unpauses() {
    // A recording with one audio encoder set on tracks 1 and 2: each record
    // locked, checked and updated once per call.
    let (v, a) = (0, 1);
    let mut recs = [Rec::last(100 * FRAME), Rec::default()];
    let venc = video_slots(&[(0, v)]);
    let aenc = audio_slots(&[(0, a), (1, a)]);

    assert!(encoded_pause(
        &mut recs,
        &venc,
        &aenc,
        FRAME,
        100 * FRAME,
        true
    ));
    assert!(encoded_pause(
        &mut recs,
        &venc,
        &aenc,
        FRAME,
        110 * FRAME,
        false
    ));
    check_rec("audio", &recs[a], 102 * FRAME, 112 * FRAME, 10 * FRAME);
    assert_eq!((recs[a].locks, recs[a].unlocks), (2, 2));
    assert_eq!((recs[v].locks, recs[v].unlocks), (2, 2));
}

#[test]
fn edge_an_interval_of_zero_gives_the_clock() {
    // Intentional difference: the C divides by the interval, which stays 0
    // until video is first set up.
    assert_eq!(closest_ts(0, 0, 0), 0);
    assert_eq!(closest_ts(5, 0, 1_000_000), 1_000_000);
    assert_eq!(closest_ts(u64::MAX, 0, 3), 3);

    // a raw output in a host that never reset video
    let mut r = Rec::default();
    assert!(raw_pause(&mut r, 0, 40, true));
    assert!(raw_pause(&mut r, 0, 90, false));
    check_rec("raw", &r, 40, 90, 50);
}

// ---------------------------------------------------------------------------
// properties

/// `get_closest_v_ts()`'s expression, where the C is defined (an interval
/// that is not 0), in the C's wrapping `uint64_t` arithmetic.
fn c_closest(last: u64, interval: u64, now: u64) -> u64 {
    let i2 = interval.wrapping_mul(2);
    last.wrapping_add((now.wrapping_sub(last).wrapping_add(i2) / interval).wrapping_mul(interval))
}

/// A timestamp: 0, small, near `u64::MAX`, around a realistic clock, or
/// anything.
fn ts() -> impl Strategy<Value = u64> {
    prop_oneof![
        Just(0_u64),
        0..1000_u64,
        (u64::MAX - 1000)..=u64::MAX,
        1_000_000_000..1_000_000_000_000_u64,
        any::<u64>(),
    ]
}

/// A frame interval: 1 ns, the usual rates, or anything but 0.
fn interval() -> impl Strategy<Value = u64> {
    prop_oneof![
        Just(1_u64),
        Just(FRAME),
        Just(16_666_667_u64),
        Just(33_333_333_u64),
        Just(41_708_333_u64),
        1..100_u64,
        1..=u64::MAX,
    ]
}

/// A slot: empty, or one of four encoders.
fn slot() -> impl Strategy<Value = usize> {
    prop_oneof![Just(0_usize), 1..=4_usize]
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(4096),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    #[test]
    fn edge_closest_matches_the_c_expression(
        last in ts(),
        interval in interval(),
        now in ts(),
    ) {
        prop_assert_eq!(closest_ts(last, interval, now), c_closest(last, interval, now));
    }

    #[test]
    fn edge_closest_is_on_the_grid_two_frames_on(
        last in 0..1_000_000_000_000_u64,
        frames in 0..100_000_u64,
        into in 0..FRAME,
        interval in prop_oneof![Just(FRAME), Just(33_333_333_u64), 1..1_000_000_u64],
    ) {
        // A clock `frames` frames and `into` ns after the last frame (into a
        // frame of `interval` ns): two frames after the frame it is in.
        let into = into % interval;
        let now = last + frames * interval + into;
        prop_assert_eq!(closest_ts(last, interval, now), last + (frames + 2) * interval);
    }

    #[test]
    fn edge_closest_with_no_interval_is_now(last in ts(), now in ts()) {
        prop_assert_eq!(closest_ts(last, 0, now), now);
    }

    #[test]
    fn edge_records(start in ts(), end in ts(), off in ts()) {
        prop_assert_eq!(can_start(start, end), start == 0 && end == 0);
        prop_assert_eq!(can_stop(start, end), start != 0 && end == 0);
        prop_assert!(!(can_start(start, end) && can_stop(start, end)));
        prop_assert_eq!(started(start), start != 0);
        prop_assert_eq!(ended(end), end != 0);
        prop_assert_eq!(offset(off, start, end), off.wrapping_add(end.wrapping_sub(start)));
    }

    #[test]
    fn edge_gates(flags in any::<u32>(), paused in any::<bool>(), pause in any::<bool>()) {
        prop_assert_eq!(allowed(flags), flags & OBS_OUTPUT_CAN_PAUSE != 0);
        prop_assert_eq!(already(paused, pause), paused == pause);
    }

    #[test]
    fn edge_lock_set_marks_each_encoder_once(
        video in prop::array::uniform10(slot()),
        audio in prop::array::uniform6(slot()),
    ) {
        let LockSet { video: vlock, audio: alock } = lock_set(&video, &audio);
        prop_assert_eq!(vlock >> NUM_VIDEO, 0);
        prop_assert_eq!(alock >> NUM_AUDIO, 0);
        // In lock order: every marked slot holds an encoder, each encoder
        // is marked once, at its first slot, and none is left out.
        let slots: Vec<(bool, usize)> = video
            .iter()
            .enumerate()
            .map(|(i, &id)| (marked(vlock, i), id))
            .chain(audio.iter().enumerate().map(|(i, &id)| (marked(alock, i), id)))
            .collect();
        let mut seen = Vec::new();
        for &(mark, id) in &slots {
            let first = id != 0 && !seen.contains(&id);
            prop_assert_eq!(mark, first);
            if first {
                seen.push(id);
            }
        }
        // Distinct encoders: every slot with one (the C's marks).
        if seen.len() == slots.iter().filter(|&&(_, id)| id != 0).count() {
            for &(mark, id) in &slots {
                prop_assert_eq!(mark, id != 0);
            }
        }
    }

    #[test]
    fn edge_any_input_without_a_panic(
        last in any::<u64>(),
        interval in any::<u64>(),
        now in any::<u64>(),
        video in prop::array::uniform10(any::<usize>()),
        audio in prop::array::uniform6(any::<usize>()),
    ) {
        let _ = closest_ts(last, interval, now);
        let _ = offset(last, interval, now);
        let _ = lock_set(&video, &audio);
    }
}
