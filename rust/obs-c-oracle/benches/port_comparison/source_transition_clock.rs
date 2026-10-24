//! obs-source: the transition clock and fit
//! (`libobs/obs-source-transition-clock.c.h`). `calc_time()` runs for every
//! audio sample a transition mixes (`get_sample_time()`) and for every
//! frame it renders; the fit, the size fold and the colour space run on
//! every tick and render. Each measurement folds its results into a
//! checksum that both sides must match:
//! - `audio_clock`: 1024 samples of one 48 kHz audio buffer through the
//!   clock, as `mix_child()` asks it, a third of the way into a 300 ms
//!   transition (the `f64` path);
//! - `ratio_ties`: 1024 ratios whose `f64` quotient lies on an `f32`
//!   rounding tie, and ratios with a duration past 2^53 ns: every one takes
//!   the integer x87 path, the worst case;
//! - `tick`: 1024 ticks of the manual step, the size fold of two children,
//!   their fits with each scale type, the colour space and the timestamps.
//!
//! Measured when it was added (x86_64 Linux, GCC, not pinned to a core;
//! median per 1024 calls): audio_clock 2.32 us Rust, 2.43 us C (0.95x);
//! ratio_ties 13.38 us Rust, 2.61 us C (5.1x: the u128 division, which
//! only those ratios take, about 1 in 2^29 in use); tick 21.33 us Rust,
//! 19.34 us C (1.10x). The outputs are the same bits.

use std::hint::black_box;

use criterion::{Criterion, Throughput};
use obs_c_oracle::source_transition_clock as c;
use obs_source::ffi::source_transition_clock as rs;

const COUNT: usize = 1024;
const MSEC: u64 = 1_000_000;

struct Side {
    time: fn(i32, f32, u64, u64, u64) -> f32,
    manual: fn(i32, f32) -> u32,
    size: fn(u32, u32) -> u32,
    fold: fn(u32, u32) -> u32,
    fit: fn(f32, f32, f32, f32, i32, &mut f32, &mut f32) -> bool,
    mix: fn(i32, i32) -> i32,
    min_ts: fn(u64, u64) -> u64,
}

const RUST: Side = Side {
    time: |m, v, s, d, ts| rs::obs_source_transition_clock_time(m, v, s, d, ts),
    manual: |m, t| rs::obs_source_transition_clock_manual(m, t),
    size: |s, a| rs::obs_source_transition_clock_size(s, a),
    fold: |s, a| rs::obs_source_transition_clock_fold(s, a),
    // SAFETY: two valid floats.
    fit: |a, b, cx, cy, t, x, y| unsafe {
        rs::obs_source_transition_clock_fit(a, b, cx, cy, t, x, y)
    },
    mix: |a, b| rs::obs_source_transition_clock_mix_spaces(a, b),
    min_ts: |a, b| rs::obs_source_transition_clock_min_ts(a, b),
};

const C: Side = Side {
    // SAFETY: plain values.
    time: |m, v, s, d, ts| unsafe { c::oracle_obs_source_transition_clock_time(m, v, s, d, ts) },
    // SAFETY: plain values.
    manual: |m, t| unsafe { c::oracle_obs_source_transition_clock_manual(m, t) },
    // SAFETY: plain values.
    size: |s, a| unsafe { c::oracle_obs_source_transition_clock_size(s, a) },
    // SAFETY: plain values.
    fold: |s, a| unsafe { c::oracle_obs_source_transition_clock_fold(s, a) },
    // SAFETY: two valid floats.
    fit: |a, b, cx, cy, t, x, y| unsafe {
        c::oracle_obs_source_transition_clock_fit(a, b, cx, cy, t, x, y)
    },
    // SAFETY: plain values.
    mix: |a, b| unsafe { c::oracle_obs_source_transition_clock_mix_spaces(a, b) },
    // SAFETY: plain values.
    min_ts: |a, b| unsafe { c::oracle_obs_source_transition_clock_min_ts(a, b) },
};

fn fold_bits(sum: u64, bits: u32) -> u64 {
    sum.wrapping_mul(31).wrapping_add(u64::from(bits))
}

/// One 48 kHz buffer: sample i is `ts + i * 10^9 / 48000` ns, as
/// `get_sample_time()` computes it.
fn audio_clock(side: &Side, start: u64, ts: u64) -> u64 {
    let mut sum = 0u64;
    for i in 0..COUNT as u64 {
        let sample_ts = ts + i * 1_000_000_000 / 48_000;
        let t = (side.time)(0, 0.0, black_box(start), 300 * MSEC, black_box(sample_ts));
        sum = fold_bits(sum, t.to_bits());
    }
    sum
}

/// Ratios on `f64` ties, and past 2^53.
fn tie_ratios() -> Vec<(u64, u64)> {
    let mut ratios = Vec::with_capacity(COUNT);
    let mut i = 0u64;
    while ratios.len() < COUNT {
        // (2m + 1) * c / (2^25 * c) is exactly a tie of the top binade
        let c = 3 + 2 * (i % 97);
        let m = (1 << 23) + i * 4099 % (1 << 23);
        ratios.push(((2 * m + 1) * c, (1 << 25) * c));
        ratios.push(((1 << 62) + i * 0x9e37_79b9, (1 << 63) + i));
        i += 1;
    }
    ratios.truncate(COUNT);
    ratios
}

fn ratio_ties(side: &Side, ratios: &[(u64, u64)]) -> u64 {
    let mut sum = 0u64;
    for &(p, q) in ratios {
        let t = (side.time)(0, 0.0, 0, black_box(q), black_box(p));
        sum = fold_bits(sum, t.to_bits());
    }
    sum
}

/// Children's sizes (a 1080p and a 4K scene, a 720p camera, a vertical
/// video) and transition sizes (none set: the larger child).
const CHILDREN: [(u32, u32); 4] = [(1920, 1080), (3840, 2160), (1280, 720), (1080, 1920)];

fn tick(side: &Side) -> u64 {
    let mut sum = 0u64;
    let mut val = 0.0f32;
    for i in 0..COUNT {
        let a = CHILDREN[i % 4];
        let b = CHILDREN[(i / 4) % 4];
        // recalculate_transition_size()
        let cx = (side.fold)((side.fold)(0, a.0), b.0);
        let cy = (side.fold)((side.fold)(0, a.1), b.1);
        // get_cx() and get_cy(), every fourth tick with a size set
        let set = if i % 4 == 0 { (1920, 1080) } else { (0, 0) };
        let tr_cx = (side.size)(set.0, cx) as f32;
        let tr_cy = (side.size)(set.1, cy) as f32;
        // recalculate_transition_matrices(), each scale type in turn
        let scale_type = i32::try_from(i % 3).unwrap();
        for child in [a, b] {
            let (mut x, mut y) = (1.0f32, 1.0f32);
            let matrix = (side.fit)(
                black_box(tr_cx),
                black_box(tr_cy),
                child.0 as f32,
                child.1 as f32,
                scale_type,
                &mut x,
                &mut y,
            );
            sum = fold_bits(
                fold_bits(fold_bits(sum, u32::from(matrix)), x.to_bits()),
                y.to_bits(),
            );
        }
        // the manual step, a T-bar at 1/16 a tick
        let target = (i % 64) as f32 / 64.0;
        match (side.manual)(1, black_box(if i % 8 == 0 { 0.0 } else { 0.25 })) {
            rs::OBS_SOURCE_TRANSITION_CLOCK_SNAP => val = target,
            rs::OBS_SOURCE_TRANSITION_CLOCK_STEP => val = (val + target) / 2.0,
            _ => {}
        }
        sum = fold_bits(sum, val.to_bits());
        // the colour space and the timestamps
        let space = (side.mix)(
            i32::try_from(i % 4).unwrap(),
            i32::try_from(i / 4 % 4).unwrap(),
        );
        let ts = (side.min_ts)(black_box((i as u64 % 3) * 1000), black_box(2000));
        sum = fold_bits(
            fold_bits(sum, space.cast_unsigned()),
            u32::try_from(ts % 4096).unwrap(),
        );
    }
    sum
}

pub(crate) fn benchmarks(criterion: &mut Criterion) {
    let start = 3_600_000 * MSEC;
    let ts = start + 100 * MSEC;
    let ratios = tie_ratios();
    assert_eq!(
        audio_clock(&RUST, start, ts),
        audio_clock(&C, start, ts),
        "benchmark fixture differs: audio_clock"
    );
    assert_eq!(
        ratio_ties(&RUST, &ratios),
        ratio_ties(&C, &ratios),
        "benchmark fixture differs: ratio_ties"
    );
    assert_eq!(tick(&RUST), tick(&C), "benchmark fixture differs: tick");

    let mut group = criterion.benchmark_group("source_transition_clock/audio_clock");
    group.throughput(Throughput::Elements(COUNT as u64));
    group.bench_function("rust", |b| {
        b.iter(|| black_box(audio_clock(&RUST, start, ts)))
    });
    group.bench_function("c", |b| b.iter(|| black_box(audio_clock(&C, start, ts))));
    group.finish();

    let mut group = criterion.benchmark_group("source_transition_clock/ratio_ties");
    group.throughput(Throughput::Elements(COUNT as u64));
    group.bench_function("rust", |b| b.iter(|| black_box(ratio_ties(&RUST, &ratios))));
    group.bench_function("c", |b| b.iter(|| black_box(ratio_ties(&C, &ratios))));
    group.finish();

    let mut group = criterion.benchmark_group("source_transition_clock/tick");
    group.throughput(Throughput::Elements(COUNT as u64));
    group.bench_function("rust", |b| b.iter(|| black_box(tick(&RUST))));
    group.bench_function("c", |b| b.iter(|| black_box(tick(&C))));
    group.finish();
}
