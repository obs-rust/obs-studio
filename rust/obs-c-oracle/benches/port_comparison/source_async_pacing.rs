//! obs-source: the async video frame pacing
//! (`libobs/obs-source-async-pacing.c.h`), which `get_closest_frame()` runs
//! on every canvas tick of every async source without 2x deinterlacing
//! (media, cameras, capture cards). Each measurement runs 1024 ticks'
//! queues, recorded from a 29.97 frames a second source into a 60 fps
//! canvas with timestamp and arrival jitter, late deliveries and clock
//! jumps, and folds the results into a checksum that both sides must
//! match:
//! - `buffered_tick`: a buffered source: the jump check and the catch-up
//!   walk with its guard and re-anchor, over queues of 1 to 8 frames;
//! - `unbuffered_tick`: the same queues, unbuffered (newest only);
//! - `walk_30`: 30 frames all in the past (`MAX_ASYNC_FRAMES`), the
//!   longest walk.
//!
//! Measured when it was added (x86_64 Linux, GCC, not pinned to a core, a
//! loaded host; median per 1024 ticks): buffered_tick 4.34 us Rust, 3.43 us
//! C (1.26x); unbuffered_tick 3.83 us Rust, 3.05 us C (1.25x); walk_30
//! 25.30 us Rust, 22.98 us C (1.10x). That is under 1 ns more per tick for
//! the queues a source sees, and about 2 ns for a 30-frame walk, once per
//! async source per canvas frame.

use std::hint::black_box;

use criterion::{Criterion, Throughput};
use obs_c_oracle::source_async_pacing as c;
use obs_source::async_pacing::{State, pace};
use obs_source::ffi::source_async_pacing as rs;

const TICKS: usize = 1024;
const MS: u64 = 1_000_000;
const SEC: u64 = 1_000_000_000;
const VT: u64 = 5000 * SEC;
const TS: u64 = 100 * SEC;
const TICK_60: u64 = 16_666_666;
const FRAME_2997: u64 = 33_366_666;

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

/// One call: the queue's timestamps, the canvas time and the source state.
#[derive(Clone)]
struct Tick {
    timestamps: Vec<u64>,
    sys_time: u64,
    state: State,
}

/// The queues a source sees tick by tick, recorded by running the Rust
/// core (both sides then get the same inputs).
fn ticks(unbuffered: bool) -> Vec<Tick> {
    let mut rng = Lcg(2024);
    let mut queue: Vec<u64> = Vec::new();
    let mut state = State {
        last_frame_ts: 0,
        last_sys_timestamp: VT,
        async_unbuffered: unbuffered,
    };
    let mut offset = TS;
    let mut next_frame = 0u64;
    let mut out = Vec::with_capacity(TICKS);
    let mut k = 0u64;
    while out.len() < TICKS {
        let r = rng.next();
        if r.is_multiple_of(300) {
            // a clock jump
            offset = offset.wrapping_add(3 * SEC);
        }
        let late = if r.is_multiple_of(17) {
            40 * MS
        } else {
            r % (8 * MS)
        };
        let rel = k * TICK_60;
        while next_frame * FRAME_2997 + late <= rel {
            if queue.len() < 30 {
                queue.push(offset + next_frame * FRAME_2997 + r % (2 * MS));
            }
            next_frame += 1;
        }
        let sys_time = VT + rel;
        if !queue.is_empty() {
            out.push(Tick {
                timestamps: queue.clone(),
                sys_time,
                state,
            });
            let p = pace(&queue, sys_time, state);
            queue.drain(..p.drop_front);
            if p.take_first {
                queue.remove(0);
            }
            state.last_frame_ts = p.last_frame_ts;
        }
        state.last_sys_timestamp = sys_time;
        k += 1;
    }
    out
}

/// 30 frames, 33 ms apart, all behind the source clock.
fn walk_30() -> Vec<Tick> {
    (0..TICKS)
        .map(|i| {
            let n = u64::try_from(i).unwrap();
            Tick {
                timestamps: (0..30).map(|f| TS - SEC + f * 33 * MS + n).collect(),
                sys_time: VT + TICK_60,
                state: State {
                    last_frame_ts: TS + n,
                    last_sys_timestamp: VT,
                    async_unbuffered: false,
                },
            }
        })
        .collect()
}

type Side = fn(&[u64], u64, State) -> (usize, u64, u8);

const RUST: Side = |ts, sys, s| {
    // SAFETY: ts.len() readable timestamps.
    let r = unsafe {
        rs::obs_source_async_pacing(
            ts.as_ptr(),
            ts.len(),
            sys,
            s.last_frame_ts,
            s.last_sys_timestamp,
            s.async_unbuffered,
        )
    };
    (r.drop_front, r.last_frame_ts, u8::from(r.take_first))
};

const C: Side = |ts, sys, s| {
    // SAFETY: as for the Rust side.
    let r = unsafe {
        c::oracle_obs_source_async_pacing(
            ts.as_ptr(),
            ts.len(),
            sys,
            s.last_frame_ts,
            s.last_sys_timestamp,
            s.async_unbuffered,
        )
    };
    (r.drop_front, r.last_frame_ts, r.take_first)
};

fn run(side: Side, ticks: &[Tick]) -> u64 {
    let mut sum = 0u64;
    for t in ticks {
        let (drop, ts, take) = side(black_box(&t.timestamps), black_box(t.sys_time), t.state);
        sum = sum
            .wrapping_mul(31)
            .wrapping_add(u64::try_from(drop).unwrap())
            .wrapping_mul(31)
            .wrapping_add(ts)
            .wrapping_mul(31)
            .wrapping_add(u64::from(take));
    }
    sum
}

pub(crate) fn benchmarks(criterion: &mut Criterion) {
    for (name, ticks) in [
        ("buffered_tick", ticks(false)),
        ("unbuffered_tick", ticks(true)),
        ("walk_30", walk_30()),
    ] {
        assert_eq!(ticks.len(), TICKS, "{name}");
        assert_eq!(
            run(RUST, &ticks),
            run(C, &ticks),
            "benchmark fixture differs: {name}"
        );
        let mut group = criterion.benchmark_group(format!("source_async_pacing/{name}"));
        group.throughput(Throughput::Elements(TICKS as u64));
        group.bench_function("rust", |b| b.iter(|| black_box(run(RUST, &ticks))));
        group.bench_function("c", |b| b.iter(|| black_box(run(C, &ticks))));
        group.finish();
    }
}
