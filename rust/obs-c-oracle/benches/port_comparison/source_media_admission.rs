//! obs-source: the media action admission decisions
//! (`libobs/obs-source-media-admission.c.h`), which every
//! `obs_source_media_*()` call runs (obs-websocket's
//! `TriggerMediaInputAction` and `SetMediaInputCursor`, the frontend's media
//! controls, hotkeys) and `process_media_actions()` runs for every queued
//! action on the video thread. Each measurement folds its results into a
//! checksum that both sides must match:
//! - `admission`: 1024 calls through the steps in the callers' order (the
//!   validity step, the flags only for a valid source, the callback only
//!   for a controllable one that has one to check) over every call, both
//!   validities, flag sets with and without OBS_SOURCE_CONTROLLABLE_MEDIA
//!   and both callbacks;
//! - `process_actions`: 1024 queued actions through the dispatch and the
//!   signal (read the pause only for a play/pause), every action type and a
//!   few outside the enum, both pauses; the signal name's first byte goes
//!   into the checksum.
//!
//! Neither side allocates while measuring.
//!
//! Measured when it was added (x86_64 Linux, GCC, not pinned to a core,
//! load average 28 to 36; median per 1024, two runs): admission 7.0 and
//! 8.7 us Rust, 3.4 and 6.0 us C; process_actions 20.7 and 19.6 us Rust,
//! 12.0 and 19.5 us C. Under 8 ns more a call either way, for calls a user
//! or a client makes by hand.

use std::hint::black_box;
use std::os::raw::{c_char, c_int};

use criterion::{Criterion, Throughput};
use obs_c_oracle::source_media_admission as c;
use obs_source::ffi::source_media_admission as rs;

const COUNT: usize = 1024;

/// `OBS_SOURCE_MEDIA_ADMIT_READ_FLAGS`, `_READ_CALLBACK` and `_ACCEPT`.
const READ_FLAGS: c_int = 2;
const READ_CALLBACK: c_int = 3;
const ACCEPT: c_int = 1;
/// `OBS_SOURCE_MEDIA_DISPATCH_RETURN`.
const RETURN: c_int = 0;

struct Side {
    admit: fn(bool) -> c_int,
    admit_flags: fn(c_int, u32) -> c_int,
    admit_callback: fn(bool) -> c_int,
    dispatch: fn(c_int) -> c_int,
    reads_pause: fn(c_int) -> bool,
    signal: fn(c_int, bool) -> *const c_char,
}

const RUST: Side = Side {
    admit: |v| rs::obs_source_media_admit(v),
    admit_flags: |call, f| rs::obs_source_media_admit_flags(call, f),
    admit_callback: |cb| rs::obs_source_media_admit_callback(cb),
    dispatch: |t| rs::obs_source_media_dispatch(t),
    reads_pause: |d| rs::obs_source_media_signal_reads_pause(d),
    signal: |d, p| rs::obs_source_media_signal(d, p),
};

const C: Side = Side {
    // SAFETY: plain values; the C touches no memory.
    admit: |v| unsafe { c::oracle_obs_source_media_admit(v) },
    // SAFETY: as above; the C takes any int for the enum.
    admit_flags: |call, f| unsafe { c::oracle_obs_source_media_admit_flags(call, f) },
    // SAFETY: as above.
    admit_callback: |cb| unsafe { c::oracle_obs_source_media_admit_callback(cb) },
    // SAFETY: as above.
    dispatch: |t| unsafe { c::oracle_obs_source_media_dispatch(t) },
    // SAFETY: as above.
    reads_pause: |d| unsafe { c::oracle_obs_source_media_signal_reads_pause(d) != 0 },
    // SAFETY: as above; the result is NULL or a string literal.
    signal: |d, p| unsafe { c::oracle_obs_source_media_signal(d, p) },
};

const FLAGS: [u32; 6] = [1 << 13, (1 << 13) | 0b111, 0b111, 0, u32::MAX, !(1 << 13)];

/// Per call: the call, whether the source is valid, its flags and whether
/// it has the callback.
fn calls() -> Vec<(c_int, bool, u32, bool)> {
    (0..COUNT)
        .map(|i| {
            let call = c_int::try_from(i % 11).unwrap();
            (call, i % 13 != 0, FLAGS[(i / 11) % FLAGS.len()], i % 5 != 0)
        })
        .collect()
}

fn run_admission(side: &Side, calls: &[(c_int, bool, u32, bool)]) -> u64 {
    let mut sum = 0_u64;
    for &(call, valid, flags, cb) in calls {
        let mut step = (side.admit)(black_box(valid));
        if step == READ_FLAGS {
            step = (side.admit_flags)(call, black_box(flags));
        }
        if step == READ_CALLBACK {
            step = (side.admit_callback)(black_box(cb));
        }
        sum = sum
            .wrapping_mul(31)
            .wrapping_add(u64::from(step == ACCEPT) + 1);
    }
    sum
}

const TYPES: [c_int; 10] = [1, 2, 3, 4, 5, 6, 1, 7, -1, 100];

/// Per queued action: its type and pause.
fn actions() -> Vec<(c_int, bool)> {
    (0..COUNT)
        .map(|i| (TYPES[i % TYPES.len()], i % 3 == 0))
        .collect()
}

fn run_actions(side: &Side, actions: &[(c_int, bool)]) -> u64 {
    let mut sum = 0_u64;
    for &(t, pause) in actions {
        let d = (side.dispatch)(black_box(t));
        if d == RETURN {
            continue;
        }
        let signal = (side.signal)(d, (side.reads_pause)(d) && black_box(pause));
        let first = if signal.is_null() {
            0
        } else {
            // SAFETY: a static NUL-terminated name, at least one byte.
            u64::from(unsafe { signal.cast::<u8>().read() })
        };
        sum = sum
            .wrapping_mul(31)
            .wrapping_add(u64::try_from(d).unwrap())
            .wrapping_add(first);
    }
    sum
}

pub(crate) fn benchmarks(criterion: &mut Criterion) {
    let calls = calls();
    let actions = actions();
    assert_eq!(
        run_admission(&RUST, &calls),
        run_admission(&C, &calls),
        "benchmark fixture differs: admission"
    );
    assert_eq!(
        run_actions(&RUST, &actions),
        run_actions(&C, &actions),
        "benchmark fixture differs: process_actions"
    );

    let mut group = criterion.benchmark_group("source_media_admission/admission");
    group.throughput(Throughput::Elements(COUNT as u64));
    group.bench_function("rust", |b| {
        b.iter(|| black_box(run_admission(&RUST, &calls)))
    });
    group.bench_function("c", |b| b.iter(|| black_box(run_admission(&C, &calls))));
    group.finish();

    let mut group = criterion.benchmark_group("source_media_admission/process_actions");
    group.throughput(Throughput::Elements(COUNT as u64));
    group.bench_function("rust", |b| {
        b.iter(|| black_box(run_actions(&RUST, &actions)))
    });
    group.bench_function("c", |b| b.iter(|| black_box(run_actions(&C, &actions))));
    group.finish();
}
