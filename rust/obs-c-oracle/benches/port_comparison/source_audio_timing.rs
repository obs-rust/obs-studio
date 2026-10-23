//! obs-source: the audio timestamp placement
//! (`libobs/obs-source-audio-timing.c.h`), the two steps
//! `source_output_audio_data()` runs for every audio packet of every
//! source. Each measurement runs a step over 1024 packets that reach every
//! branch (direct, reset, jump, smooth with and without the re-anchor,
//! exceeded; append, place, the reset under the lock, a changed sync
//! offset), and folds the outputs into a checksum that both sides must
//! match.
//!
//! Measured when it was added (x86_64 Linux, GCC, not pinned to a core;
//! median per 1024 packets): pre_lock 5.12 us Rust, 4.78 us C (1.07x);
//! under_lock 4.50 us Rust, 3.95 us C (1.14x). That is under 0.6 ns more
//! per packet, for one packet of each source every 21 ms or so.

use std::ffi::c_void;
use std::hint::black_box;

use criterion::{Criterion, Throughput};
use obs_c_oracle::source_audio_timing as c;
use obs_source::ffi::source_audio_timing::{
    self as rs, obs_source_audio_timing_pre as Pre, obs_source_audio_timing_under as Under,
};

const PACKETS: usize = 1024;
const OS: u64 = 1_000_000_000_000_000;
const TS: u64 = 5_000_000_000;
const DUR: u64 = 21_333_333;
const MS: u64 = 1_000_000;

/// A small deterministic generator, so both sides see the same packets.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 11
    }
}

fn pre_inputs() -> Vec<Pre> {
    let mut rng = Lcg(1932);
    (0..PACKETS)
        .map(|i| {
            let r = rng.next();
            let ts = TS + u64::try_from(i).unwrap() * DUR;
            let (timestamp, os_time, next) = match r % 6 {
                // direct
                0 => (OS + r % (3 * MS), OS, OS + r % (100 * MS)),
                // jump
                1 => (ts, OS, ts + 3_000_000_000 + r % MS),
                // exceeded
                2 => (ts, OS, ts + 100 * MS + r % MS),
                // smooth, either side
                _ => (ts, OS, ts.wrapping_add(r % (60 * MS)).wrapping_sub(30 * MS)),
            };
            Pre {
                timestamp,
                os_time,
                duration: DUR,
                next_audio_ts_min: if r.is_multiple_of(17) { 0 } else { next },
                timing_adjust: 0,
                diff: 0,
                drift_error: 0,
                action: 0,
                timing_set: !r.is_multiple_of(13),
                async_unbuffered: r.is_multiple_of(5),
                async_decoupled: r.is_multiple_of(3),
                direct: false,
                reanchor: false,
                continuous: false,
            }
        })
        .collect()
}

fn under_inputs() -> Vec<Under> {
    let mut rng = Lcg(522);
    (0..PACKETS)
        .map(|i| {
            let r = rng.next();
            let ts = TS + u64::try_from(i).unwrap() * DUR;
            let adjust = OS - TS;
            let sys = ts + adjust;
            let next_sys = match r % 5 {
                0 => sys,
                1 => sys + r % (60 * MS),
                2 => sys + 3_000_000_000,
                3 => 0,
                _ => sys - 100 * MS,
            };
            let sync = if r.is_multiple_of(7) { 300 } else { 0 };
            Under {
                timestamp: ts,
                packet_timing_adjust: adjust,
                data_timestamp: ts,
                os_time: OS + r % MS,
                next_audio_ts_min: ts + DUR,
                resample_offset: r % 2,
                sync_offset: i64::try_from(sync * MS).unwrap(),
                timing_adjust: adjust,
                next_audio_sys_ts_min: next_sys,
                last_sync_offset: if r.is_multiple_of(11) {
                    0
                } else {
                    i64::try_from(sync * MS).unwrap()
                },
                continuous: r.is_multiple_of(4),
                push_back: false,
                reset_timing: false,
            }
        })
        .collect()
}

fn pre_sum(t: &Pre) -> u64 {
    t.timestamp
        ^ t.next_audio_ts_min.rotate_left(7)
        ^ t.timing_adjust.rotate_left(13)
        ^ t.diff.rotate_left(19)
        ^ t.drift_error.cast_unsigned().rotate_left(29)
        ^ u64::from(t.action) << 1
        ^ u64::from(t.direct) << 4
        ^ u64::from(t.reanchor) << 5
        ^ u64::from(t.continuous) << 6
}

fn under_sum(t: &Under) -> u64 {
    t.timestamp
        ^ t.timing_adjust.rotate_left(7)
        ^ t.next_audio_sys_ts_min.rotate_left(13)
        ^ t.last_sync_offset.cast_unsigned().rotate_left(19)
        ^ u64::from(t.push_back) << 1
        ^ u64::from(t.reset_timing) << 2
}

pub fn benchmarks(criterion: &mut Criterion) {
    let pre = pre_inputs();
    let under = under_inputs();
    let mut pre_work = pre.clone();
    let mut under_work = under.clone();

    let mut run_pre = |step: unsafe fn(*mut Pre)| {
        pre_work.copy_from_slice(&pre);
        let mut sum = 0u64;
        for t in &mut pre_work {
            // SAFETY: a valid struct with initialized inputs.
            unsafe { step(black_box(t)) };
            sum = sum.wrapping_mul(31).wrapping_add(pre_sum(t));
        }
        sum
    };
    unsafe fn rs_pre(t: *mut Pre) {
        // SAFETY: forwarded.
        unsafe { rs::obs_source_audio_timing_pre_lock(t) }
    }
    unsafe fn c_pre(t: *mut Pre) {
        // SAFETY: forwarded; the oracle has the same layout (the parity
        // test checks it).
        unsafe { c::oracle_obs_source_audio_timing_pre_lock(t.cast::<c_void>()) }
    }
    let rust_sum = run_pre(rs_pre);
    let c_sum = run_pre(c_pre);
    assert_eq!(rust_sum, c_sum, "benchmark fixture differs: pre_lock");
    {
        let mut group = criterion.benchmark_group("source_audio_timing/pre_lock");
        group.throughput(Throughput::Elements(PACKETS as u64));
        group.bench_function("rust", |b| b.iter(|| black_box(run_pre(rs_pre))));
        group.bench_function("c", |b| b.iter(|| black_box(run_pre(c_pre))));
        group.finish();
    }

    let mut run_under = |step: unsafe fn(*mut Under)| {
        under_work.copy_from_slice(&under);
        let mut sum = 0u64;
        for t in &mut under_work {
            // SAFETY: a valid struct with initialized inputs.
            unsafe { step(black_box(t)) };
            sum = sum.wrapping_mul(31).wrapping_add(under_sum(t));
        }
        sum
    };
    unsafe fn rs_under(t: *mut Under) {
        // SAFETY: forwarded.
        unsafe { rs::obs_source_audio_timing_under_lock(t) }
    }
    unsafe fn c_under(t: *mut Under) {
        // SAFETY: as for c_pre.
        unsafe { c::oracle_obs_source_audio_timing_under_lock(t.cast::<c_void>()) }
    }
    let rust_sum = run_under(rs_under);
    let c_sum = run_under(c_under);
    assert_eq!(rust_sum, c_sum, "benchmark fixture differs: under_lock");
    {
        let mut group = criterion.benchmark_group("source_audio_timing/under_lock");
        group.throughput(Throughput::Elements(PACKETS as u64));
        group.bench_function("rust", |b| b.iter(|| black_box(run_under(rs_under))));
        group.bench_function("c", |b| b.iter(|| black_box(run_under(c_under))));
        group.finish();
    }
}
