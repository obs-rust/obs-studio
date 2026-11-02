//! obs-source: the time of each audio sample a transition mixes
//! (`libobs/obs-source-transition-sample-time.c.h`, #3481). `mix_child()`
//! asks it once per sample, per channel, per mix, while a transition runs.
//! One call here is one channel of one block: 1024 samples at 48 kHz, a
//! third of the way into a 300 ms transition, and one at 44.1 kHz straddling
//! its end. Each folds its results into a checksum both sides must match.
//! The C side is the moved C over the C clock, as libobs-c-reference
//! composes them.

use std::hint::black_box;

use criterion::{Criterion, Throughput};
use obs_c_oracle::source_transition_sample_time::oracle_obs_source_transition_sample_time;
use obs_source::ffi::source_transition_sample_time::obs_source_transition_sample_time;

/// `AUDIO_OUTPUT_FRAMES`.
const FRAMES: usize = 1024;
const MSEC: u64 = 1_000_000;

type SampleTime = fn(i32, f32, u64, u64, u64, usize, usize) -> f32;

const RUST: SampleTime = |m, v, s, d, ts, rate, i| obs_source_transition_sample_time(m, v, s, d, ts, rate, i);
// SAFETY: plain values; the benchmarks never pass a rate of 0.
const C: SampleTime = |m, v, s, d, ts, rate, i| unsafe {
    oracle_obs_source_transition_sample_time(m, v, s, d, ts, rate, i)
};

fn fold_bits(sum: u64, bits: u32) -> u64 {
    sum.wrapping_mul(31).wrapping_add(u64::from(bits))
}

/// One channel of one block, as `mix_child()` asks it.
fn block(side: SampleTime, start: u64, ts: u64, rate: usize) -> u64 {
    let mut sum = 0u64;
    for i in 0..FRAMES {
        let t = side(0, 0.0, black_box(start), 300 * MSEC, black_box(ts), rate, i);
        sum = fold_bits(sum, t.to_bits());
    }
    sum
}

pub(crate) fn benchmarks(criterion: &mut Criterion) {
    let start = 3_600_000 * MSEC;
    let blocks = [
        ("48k", start + 100 * MSEC, 48_000),
        ("44k1_end", start + 290 * MSEC, 44_100),
    ];
    for (name, ts, rate) in blocks {
        assert_eq!(
            block(RUST, start, ts, rate),
            block(C, start, ts, rate),
            "benchmark fixture differs: {name}"
        );
        let mut group =
            criterion.benchmark_group(format!("source_transition_sample_time/{name}"));
        group.throughput(Throughput::Elements(FRAMES as u64));
        group.bench_function("rust", |b| {
            b.iter(|| black_box(block(RUST, start, ts, rate)))
        });
        group.bench_function("c", |b| b.iter(|| black_box(block(C, start, ts, rate))));
        group.finish();
    }
}
