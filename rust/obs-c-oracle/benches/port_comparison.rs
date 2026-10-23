//! #142: compare production C ABI shims with the original C oracles.
//! Unsafe code belongs in this test-only oracle crate, never the safe cores.

use criterion::{Criterion, criterion_group, criterion_main};
use std::{fmt::Debug, hint::black_box};

#[path = "port_comparison/audio.rs"]
mod audio;
#[path = "port_comparison/callback.rs"]
mod callback;
#[path = "port_comparison/codecs.rs"]
mod codecs;
#[path = "port_comparison/conversions.rs"]
mod conversions;
#[path = "port_comparison/core.rs"]
mod core_units;
#[path = "port_comparison/graphics.rs"]
mod graphics;
#[path = "port_comparison/util.rs"]
mod util;

fn pair<T: PartialEq + Debug>(
    criterion: &mut Criterion,
    name: &str,
    mut rust: impl FnMut() -> T,
    mut original: impl FnMut() -> T,
) {
    assert_eq!(rust(), original(), "benchmark fixture differs: {name}");
    measure_pair(criterion, name, rust, original);
}

fn measure_pair<T>(
    criterion: &mut Criterion,
    name: &str,
    mut rust: impl FnMut() -> T,
    mut original: impl FnMut() -> T,
) {
    let mut group = criterion.benchmark_group(name);
    group.bench_function("rust", |b| b.iter(|| black_box(rust())));
    group.bench_function("c", |b| b.iter(|| black_box(original())));
    group.finish();
}

fn comparisons(criterion: &mut Criterion) {
    util::benchmarks(criterion);
    graphics::benchmarks(criterion);
    codecs::benchmarks(criterion);
    callback::benchmarks(criterion);
    conversions::benchmarks(criterion);
    core_units::benchmarks(criterion);
    audio::benchmarks(criterion);
}

criterion_group!(benches, comparisons);
criterion_main!(benches);
