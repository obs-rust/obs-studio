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
#[path = "port_comparison/encoder_roi.rs"]
mod encoder_roi;
#[path = "port_comparison/graphics.rs"]
mod graphics;
#[path = "port_comparison/media_remux.rs"]
mod media_remux;
#[path = "port_comparison/module_source_rules.rs"]
mod module_source_rules;
#[path = "port_comparison/scene.rs"]
mod scene;
#[path = "port_comparison/scene_crop.rs"]
mod scene_crop;
#[path = "port_comparison/scene_render_plan.rs"]
mod scene_render_plan;
#[path = "port_comparison/service.rs"]
mod service;
#[path = "port_comparison/source_audio_input.rs"]
mod source_audio_input;
#[path = "port_comparison/source_audio_timing.rs"]
mod source_audio_timing;
#[path = "port_comparison/source_color_space.rs"]
mod source_color_space;
#[path = "port_comparison/source_deinterlace_pacing.rs"]
mod source_deinterlace_pacing;
#[path = "port_comparison/source_profiler.rs"]
mod source_profiler;
#[path = "port_comparison/util.rs"]
mod util;
#[path = "port_comparison/view.rs"]
mod view;

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
    audio::benchmarks(criterion);
    callback::benchmarks(criterion);
    codecs::benchmarks(criterion);
    conversions::benchmarks(criterion);
    core_units::benchmarks(criterion);
    encoder_roi::benchmarks(criterion);
    graphics::benchmarks(criterion);
    media_remux::benchmarks(criterion);
    module_source_rules::benchmarks(criterion);
    scene::benchmarks(criterion);
    scene_crop::benchmarks(criterion);
    scene_render_plan::benchmarks(criterion);
    service::benchmarks(criterion);
    source_audio_input::benchmarks(criterion);
    source_audio_timing::benchmarks(criterion);
    source_color_space::benchmarks(criterion);
    source_deinterlace_pacing::benchmarks(criterion);
    source_profiler::benchmarks(criterion);
    util::benchmarks(criterion);
    view::benchmarks(criterion);
}

criterion_group!(benches, comparisons);
criterion_main!(benches);
