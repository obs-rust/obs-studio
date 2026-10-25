//! obs-source: the deinterlace frame selection
//! (`libobs/obs-source-deinterlace-select.c.h`), which `async_tick()` runs
//! once a tick for each deinterlaced async source, under `async_mutex`.
//! Each measurement runs 1024 ticks' decisions and folds the results into a
//! checksum that both sides must match:
//! - `select_tick`: `first()`, the walk over the queued timestamps (1 to 8
//!   frames at 50 fields a second, buffered and unbuffered, the clock from
//!   before the front frame to well past the last), the pair guard for an
//!   unbuffered source left with two, and the take decisions;
//! - `long_queue`: the walk over 33 queued frames (past
//!   `MAX_ASYNC_FRAMES`), the clock past most of them.
//!
//! The fixtures are plain vectors; nothing goes through bmem.

use std::hint::black_box;

use criterion::{Criterion, Throughput};
use obs_c_oracle::source_deinterlace_select as c;
use obs_source::ffi::source_deinterlace_select as rs;

const CALLS: usize = 1024;
const MS: u64 = 1_000_000;
const T: u64 = 1_000_000_000_000;
const S: u64 = 20 * MS;
const SYS: u64 = 7_000_000_000_000;

/// A small deterministic generator, so both sides see the same calls.
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

struct Tick {
    timestamps: Vec<u64>,
    last_frame_ts: u64,
    sys_time: u64,
    unbuffered: bool,
    offset: u64,
}

fn ticks(frames: Option<usize>) -> Vec<Tick> {
    let mut rng = Lcg(2315);
    (0..CALLS)
        .map(|_| {
            let r = rng.next();
            let n = frames.unwrap_or(usize::try_from(r % 8).unwrap() + 1);
            let timestamps: Vec<u64> = (0..n as u64).map(|i| T + i * S).collect();
            // the clock from 10 ms before the front frame to 4 frames past
            // the last
            let past = (r >> 8) % ((n as u64 + 4) * S);
            Tick {
                timestamps,
                // one tick in 32 is a new source (no clock yet)
                last_frame_ts: if (r >> 40).is_multiple_of(32) { 0 } else { T },
                sys_time: SYS - 10 * MS + past,
                unbuffered: (r >> 48).is_multiple_of(2),
                offset: if (r >> 50).is_multiple_of(2) {
                    0
                } else {
                    SYS - T
                },
            }
        })
        .collect()
}

/// A walk result as the C lays it out: drop_front, last_frame_ts and the
/// bytes of offset_reset, pairs, flag_front and ready.
type WalkOut = (usize, u64, u8, u8, u8, u8);
/// The take decisions: reads_flag, pair and init_last as bytes, the offset
/// kind as the C's u32.
type TakeOut = (u8, u8, u8, u32);

struct Side {
    first: fn(u64, usize) -> (u8, u8),
    walk: fn(&[u64], u64, u64, u64, bool) -> WalkOut,
    pair: fn(u64, u64, u64, u64) -> (u8, u8, u64),
    take: fn(usize, bool, u64, bool) -> TakeOut,
}

const RUST: Side = Side {
    first: |l, n| {
        let r = rs::obs_source_deinterlace_select_first(l, n);
        (u8::from(r.first), u8::from(r.flag_front))
    },
    walk: |ts, sys, last, last_sys, unbuffered| {
        // SAFETY: ts.len() readable timestamps, not written during the call.
        let r = unsafe {
            rs::obs_source_deinterlace_select_walk(
                ts.as_ptr(),
                ts.len(),
                sys,
                last,
                last_sys,
                unbuffered,
            )
        };
        (
            r.drop_front,
            r.last_frame_ts,
            u8::from(r.offset_reset),
            u8::from(r.pairs),
            u8::from(r.flag_front),
            u8::from(r.ready),
        )
    },
    pair: |a, b, o, t| {
        let r = rs::obs_source_deinterlace_select_unbuffered_pair(a, b, o, t);
        (u8::from(r.prev_frame), u8::from(r.set_frame_ts), r.frame_ts)
    },
    take: |remaining, flag, last, unbuffered| {
        (
            u8::from(rs::obs_source_deinterlace_select_take_reads_flag(remaining)),
            u8::from(rs::obs_source_deinterlace_select_take_pair(remaining, flag)),
            u8::from(rs::obs_source_deinterlace_select_take_init_last(last)),
            rs::obs_source_deinterlace_select_take_offset(unbuffered),
        )
    },
};

// SAFETY (each): the oracle functions take and return plain values, and the
// walk reads ts.len() timestamps that are not written during the call.
const C: Side = Side {
    first: |l, n| {
        let r = unsafe { c::oracle_obs_source_deinterlace_select_first(l, n) };
        (r.first, r.flag_front)
    },
    walk: |ts, sys, last, last_sys, unbuffered| {
        let r = unsafe {
            c::oracle_obs_source_deinterlace_select_walk(
                ts.as_ptr(),
                ts.len(),
                sys,
                last,
                last_sys,
                unbuffered,
            )
        };
        (
            r.drop_front,
            r.last_frame_ts,
            r.offset_reset,
            r.pairs,
            r.flag_front,
            r.ready,
        )
    },
    pair: |a, b, o, t| {
        let r = unsafe { c::oracle_obs_source_deinterlace_select_unbuffered_pair(a, b, o, t) };
        (r.prev_frame, r.set_frame_ts, r.frame_ts)
    },
    take: |remaining, flag, last, unbuffered| unsafe {
        (
            c::oracle_obs_source_deinterlace_select_take_reads_flag(remaining),
            c::oracle_obs_source_deinterlace_select_take_pair(remaining, flag),
            c::oracle_obs_source_deinterlace_select_take_init_last(last),
            c::oracle_obs_source_deinterlace_select_take_offset(unbuffered),
        )
    },
};

fn fold(sum: u64, value: u64) -> u64 {
    sum.wrapping_mul(31).wrapping_add(value)
}

/// One tick's decisions in `deinterlace_get_closest_frames()`'s order.
fn select_tick(side: &Side, ticks: &[Tick]) -> u64 {
    let mut sum = 0u64;
    for t in ticks {
        let ts = black_box(t.timestamps.as_slice());
        let (first, flag_front) = (side.first)(t.last_frame_ts, ts.len());
        sum = fold(sum, u64::from(flag_front));
        let mut start = 0usize;
        let mut flag = flag_front != 0;
        if first == 0 {
            let last = if t.unbuffered { 0 } else { t.last_frame_ts };
            let (drop, lft, reset, pairs, flag_walk, ready) =
                (side.walk)(ts, t.sys_time, last, SYS, t.unbuffered);
            sum = fold(sum, drop as u64);
            sum = fold(sum, lft);
            sum = fold(sum, u64::from(reset));
            sum = fold(sum, u64::from(ready));
            start = drop;
            flag = flag_walk != 0;
            if pairs != 0 {
                let (prev, set, frame_ts) =
                    (side.pair)(ts[drop], ts[drop + 1], t.offset, t.sys_time);
                sum = fold(sum, u64::from(set));
                sum = fold(sum, frame_ts);
                flag = prev != 0;
            }
            if ready == 0 {
                continue;
            }
        }
        let remaining = ts.len() - start - 1;
        let (reads, pair, init, kind) = (side.take)(remaining, flag, t.last_frame_ts, t.unbuffered);
        sum = fold(sum, u64::from(reads));
        sum = fold(sum, u64::from(pair));
        sum = fold(sum, u64::from(init));
        sum = fold(sum, u64::from(kind));
    }
    sum
}

pub(crate) fn benchmarks(criterion: &mut Criterion) {
    for (name, fixture) in [
        ("select_tick", ticks(None)),
        ("long_queue", ticks(Some(33))),
    ] {
        assert_eq!(
            select_tick(&RUST, &fixture),
            select_tick(&C, &fixture),
            "benchmark fixture differs: {name}"
        );
        let mut group = criterion.benchmark_group(format!("source_deinterlace_select/{name}"));
        group.throughput(Throughput::Elements(CALLS as u64));
        group.bench_function("rust", |b| {
            b.iter(|| black_box(select_tick(&RUST, &fixture)))
        });
        group.bench_function("c", |b| b.iter(|| black_box(select_tick(&C, &fixture))));
        group.finish();
    }
}
