//! obs-source: the 2x deinterlace frame pacing
//! (`libobs/obs-source-deinterlace-pacing.c.h`), which
//! `deinterlace_get_closest_frames()` runs on every canvas tick of an async
//! source with a double-rate deinterlace mode. Each measurement runs 1024
//! ticks of 50i and 59.94i sources into a 60 fps canvas, with jitter, late
//! frames and jumps, that reach every branch (held, moved on, the clock
//! jumped; waited or not; the re-anchor with no offset, early, late and
//! kept), and folds the results into a checksum that both sides must match:
//! - `unbuffered_tick`: the hold, the guard and its timestamp, the half
//!   duration and the re-anchor, as an unbuffered source runs them;
//! - `buffered_pick`: the half duration and the buffered offset.
//!
//! Measured when it was added (x86_64 Linux, GCC, not pinned to a core;
//! median per 1024 ticks): unbuffered_tick 8.67 us Rust, 7.59 us C (1.14x);
//! buffered_pick 3.20 us Rust, 3.13 us C (1.02x). That is about 1 ns more
//! per tick, for one tick of each deinterlaced source every canvas frame.

use std::hint::black_box;

use criterion::{Criterion, Throughput};
use obs_c_oracle::source_deinterlace_pacing as c;
use obs_source::ffi::source_deinterlace_pacing as rs;

const TICKS: usize = 1024;
const VT: u64 = 1_000_000_000_000;
const TS: u64 = 5_000_000_000;
const MS: u64 = 1_000_000;
const TICK_60: u64 = 16_666_666;

/// A small deterministic generator, so both sides see the same ticks.
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

#[derive(Clone, Copy)]
struct Tick {
    video_time: u64,
    frame_ts: u64,
    next_ts: u64,
    offset: u64,
    half: u32,
}

fn ticks() -> Vec<Tick> {
    let mut rng = Lcg(1950);
    (0..TICKS)
        .map(|i| {
            let r = rng.next();
            let n = u64::try_from(i).unwrap();
            let frame = if r.is_multiple_of(2) {
                40 * MS
            } else {
                33_366_667
            };
            let frame_ts = TS + n * frame + r % (2 * MS);
            let video_time = VT + n * TICK_60;
            let offset = match r % 8 {
                // no offset yet
                0 => 0,
                // a frame early
                1 => (video_time - frame_ts) + 1 + r % MS,
                // a frame late by more than a field
                2 => (video_time - frame_ts).wrapping_sub(25 * MS + r % MS),
                // a clock jump
                3 => (video_time - frame_ts).wrapping_sub(3_000_000_000),
                // on time, kept
                _ => (video_time - frame_ts).wrapping_sub(r % (10 * MS)),
            };
            Tick {
                video_time,
                frame_ts,
                next_ts: frame_ts + frame,
                offset,
                half: u32::try_from(frame / 2).unwrap(),
            }
        })
        .collect()
}

struct Side {
    hold: fn(u64, u64, u32, u64) -> u8,
    wait: fn(u64, u64, u64, u64) -> u8,
    waiting_ts: fn(u64, u64) -> u64,
    half: fn(u64, u64, u32, u64) -> u32,
    buffered: fn(u64, u64) -> u64,
    reanchor: fn(u64, u64, u64, u32, u64) -> u64,
}

const RUST: Side = Side {
    hold: |a, b, h, s| u8::from(rs::obs_source_deinterlace_pacing_hold(a, b, h, s)),
    wait: |a, b, o, s| u8::from(rs::obs_source_deinterlace_pacing_wait_for_next(a, b, o, s)),
    waiting_ts: |a, b| rs::obs_source_deinterlace_pacing_waiting_frame_ts(a, b),
    half: |a, b, h, i| rs::obs_source_deinterlace_pacing_half_duration(a, b, h, i),
    buffered: |a, b| rs::obs_source_deinterlace_pacing_buffered_offset(a, b),
    reanchor: |o, v, f, h, i| rs::obs_source_deinterlace_pacing_reanchor(o, v, f, h, i),
};

// SAFETY (each): the oracle functions take and return plain values.
const C: Side = Side {
    hold: |a, b, h, s| unsafe { c::oracle_obs_source_deinterlace_pacing_hold(a, b, h, s) },
    wait: |a, b, o, s| unsafe { c::oracle_obs_source_deinterlace_pacing_wait_for_next(a, b, o, s) },
    waiting_ts: |a, b| unsafe { c::oracle_obs_source_deinterlace_pacing_waiting_frame_ts(a, b) },
    half: |a, b, h, i| unsafe { c::oracle_obs_source_deinterlace_pacing_half_duration(a, b, h, i) },
    buffered: |a, b| unsafe { c::oracle_obs_source_deinterlace_pacing_buffered_offset(a, b) },
    reanchor: |o, v, f, h, i| unsafe {
        c::oracle_obs_source_deinterlace_pacing_reanchor(o, v, f, h, i)
    },
};

fn unbuffered(side: &Side, ticks: &[Tick]) -> u64 {
    let mut sum = 0u64;
    for t in ticks {
        let t = black_box(*t);
        let held = (side.hold)(t.frame_ts, t.offset, t.half, t.video_time);
        let wait = (side.wait)(t.frame_ts, t.next_ts, t.offset, t.video_time);
        let waiting = (side.waiting_ts)(t.frame_ts, t.next_ts);
        let half = (side.half)(t.next_ts, t.frame_ts, t.half, TICK_60);
        let offset = (side.reanchor)(t.offset, t.video_time, t.next_ts, half, TICK_60);
        sum = sum
            .wrapping_mul(31)
            .wrapping_add(u64::from(held))
            .wrapping_mul(31)
            .wrapping_add(u64::from(wait))
            .wrapping_mul(31)
            .wrapping_add(waiting)
            .wrapping_mul(31)
            .wrapping_add(offset);
    }
    sum
}

fn buffered(side: &Side, ticks: &[Tick]) -> u64 {
    let mut sum = 0u64;
    for t in ticks {
        let t = black_box(*t);
        let half = (side.half)(t.next_ts, t.frame_ts, t.half, TICK_60);
        let offset = (side.buffered)(t.video_time, t.frame_ts);
        sum = sum
            .wrapping_mul(31)
            .wrapping_add(u64::from(half))
            .wrapping_mul(31)
            .wrapping_add(offset);
    }
    sum
}

pub(crate) fn benchmarks(criterion: &mut Criterion) {
    let ticks = ticks();
    for (name, run) in [
        ("unbuffered_tick", unbuffered as fn(&Side, &[Tick]) -> u64),
        ("buffered_pick", buffered),
    ] {
        assert_eq!(
            run(&RUST, &ticks),
            run(&C, &ticks),
            "benchmark fixture differs: {name}"
        );
        let mut group = criterion.benchmark_group(format!("source_deinterlace_pacing/{name}"));
        group.throughput(Throughput::Elements(TICKS as u64));
        group.bench_function("rust", |b| b.iter(|| black_box(run(&RUST, &ticks))));
        group.bench_function("c", |b| b.iter(|| black_box(run(&C, &ticks))));
        group.finish();
    }
}
