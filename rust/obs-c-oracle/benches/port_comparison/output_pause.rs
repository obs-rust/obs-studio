//! obs-output: the output pause plan (`libobs/obs-output-pause.c.h`). Every
//! pause and unpause of a recording asks the gates, the lock set over the
//! output's slots, the pause time and each record's rules. Each side runs
//! 256 pauses and unpauses of a recording with one video and two audio
//! encoders (as `obs_output_pause()` and `obs_encoded_output_pause()` ask
//! them, on stack records) and folds the answers into a checksum both must
//! match. Nothing is allocated in the loop.

use super::pair;

use criterion::Criterion;
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
use std::hint::black_box;

const CALLS: u64 = 256;
/// `OBS_OUTPUT_VIDEO | OBS_OUTPUT_AUDIO | OBS_OUTPUT_ENCODED |
/// OBS_OUTPUT_CAN_PAUSE`, a recording.
const FLAGS: u32 = 0x1 | 0x2 | 0x4 | (1 << 5);
/// 60 fps.
const FRAME: u64 = 16_666_666;

/// One side's functions.
struct Side {
    allowed: fn(u32) -> bool,
    already: fn(bool, bool) -> bool,
    lock_set: fn(&[usize; 10], &[usize; 6]) -> (u32, u32),
    closest: fn(u64, u64, u64) -> u64,
    started: fn(u64) -> bool,
    ended: fn(u64) -> bool,
    offset: fn(u64, u64, u64) -> u64,
}

#[derive(Clone, Copy, Default)]
struct Rec {
    last_video_ts: u64,
    ts_start: u64,
    ts_end: u64,
    ts_offset: u64,
}

fn fold(acc: u64, value: u64) -> u64 {
    acc.wrapping_mul(31).wrapping_add(value)
}

/// 256 calls alternating pause and unpause, the records cleared after
/// each unpause as the encoders clear them.
fn run(side: &Side) -> u64 {
    let mut video = [0_usize; 10];
    let mut audio = [0_usize; 6];
    video[0] = 1;
    audio[0] = 2;
    audio[1] = 3;
    let mut recs = [Rec::default(); 3];
    let mut paused = false;
    (0..CALLS).fold(0, |mut acc, n| {
        let n = black_box(n);
        let pause = n % 2 == 0;
        let now = 1_000_000_000 + n * 7 * FRAME + n;
        recs[0].last_video_ts = now - n % 5 * FRAME;
        if !(side.allowed)(FLAGS) || (side.already)(paused, pause) {
            return acc;
        }
        let (vlock, alock) = (side.lock_set)(&video, &audio);
        let order: [Option<usize>; 3] = [
            (vlock & 1 != 0).then_some(0),
            (alock & 1 != 0).then_some(1),
            (alock & 2 != 0).then_some(2),
        ];
        let ts = (side.closest)(recs[0].last_video_ts, FRAME, now);
        // pause_can_start() and pause_can_stop(): the end tested only once
        // the start test passed.
        let can = |r: &Rec| {
            let started = (side.started)(r.ts_start);
            (if pause { !started } else { started }) && !(side.ended)(r.ts_end)
        };
        if order.iter().flatten().all(|&r| can(&recs[r]))
        {
            for &r in order.iter().flatten() {
                let rec = &mut recs[r];
                if pause {
                    rec.ts_start = ts;
                } else if !(side.ended)(rec.ts_end) {
                    rec.ts_end = ts;
                    rec.ts_offset = (side.offset)(rec.ts_offset, rec.ts_start, rec.ts_end);
                }
                acc = fold(acc, rec.ts_start ^ rec.ts_end ^ rec.ts_offset);
            }
            paused = pause;
            if !pause {
                for rec in &mut recs {
                    rec.ts_start = 0;
                    rec.ts_end = 0;
                }
            }
        }
        fold(acc, u64::from(vlock) << 8 | u64::from(alock))
    })
}

fn rust_lock_set(video: &[usize; 10], audio: &[usize; 6]) -> (u32, u32) {
    let (mut v, mut a) = (0, 0);
    // SAFETY: two arrays of the C's lengths and two locals to write.
    unsafe { obs_output_pause_lock_set(video, audio, &raw mut v, &raw mut a) };
    (v, a)
}

fn c_lock_set(video: &[usize; 10], audio: &[usize; 6]) -> (u32, u32) {
    let (mut v, mut a) = (0, 0);
    // SAFETY: as above.
    unsafe { oracle_obs_output_pause_lock_set(video, audio, &raw mut v, &raw mut a) };
    (v, a)
}

const RUST: Side = Side {
    allowed: |f| obs_output_pause_allowed(f),
    already: |p, q| obs_output_pause_already(p, q),
    lock_set: rust_lock_set,
    closest: |l, i, n| obs_output_pause_closest_ts(l, i, n),
    started: |s| obs_output_pause_started(s),
    ended: |e| obs_output_pause_ended(e),
    offset: |o, s, e| obs_output_pause_offset(o, s, e),
};

// SAFETY (each wrapper): plain values; the interval is never 0.
const C: Side = Side {
    allowed: |f| unsafe { oracle_obs_output_pause_allowed(f) },
    already: |p, q| unsafe { oracle_obs_output_pause_already(p, q) },
    lock_set: c_lock_set,
    closest: |l, i, n| unsafe { oracle_obs_output_pause_closest_ts(l, i, n) },
    started: |s| unsafe { oracle_obs_output_pause_started(s) },
    ended: |e| unsafe { oracle_obs_output_pause_ended(e) },
    offset: |o, s, e| unsafe { oracle_obs_output_pause_offset(o, s, e) },
};

pub fn benchmarks(criterion: &mut Criterion) {
    pair(
        criterion,
        "output_pause/pause_unpause/256",
        || run(&RUST),
        || run(&C),
    );
}
