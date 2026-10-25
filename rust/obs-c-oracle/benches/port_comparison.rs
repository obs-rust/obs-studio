//! #142: compare production C ABI shims with the original C oracles.
//! Unsafe code belongs in this test-only oracle crate, never the safe cores.

use criterion::{Criterion, criterion_group, criterion_main};
use std::{fmt::Debug, hint::black_box};

#[path = "port_comparison/audio.rs"]
mod audio;
#[path = "port_comparison/callback.rs"]
mod callback;
#[path = "port_comparison/canvas_draw.rs"]
mod canvas_draw;
#[path = "port_comparison/codecs.rs"]
mod codecs;
#[path = "port_comparison/conversions.rs"]
mod conversions;
#[path = "port_comparison/core.rs"]
mod core_units;
#[path = "port_comparison/encoder_audio_start.rs"]
mod encoder_audio_start;
#[path = "port_comparison/encoder_roi.rs"]
mod encoder_roi;
#[path = "port_comparison/encoder_size.rs"]
mod encoder_size;
#[path = "port_comparison/graphics.rs"]
mod graphics;
#[path = "port_comparison/graphics_sprite.rs"]
mod graphics_sprite;
#[path = "port_comparison/media_remux.rs"]
mod media_remux;
#[path = "port_comparison/module_source_rules.rs"]
mod module_source_rules;
#[path = "port_comparison/module_type_rules.rs"]
mod module_type_rules;
#[path = "port_comparison/output_video_checks.rs"]
mod output_video_checks;
#[path = "port_comparison/scene.rs"]
mod scene;
#[path = "port_comparison/scene_audio_window.rs"]
mod scene_audio_window;
#[path = "port_comparison/scene_crop.rs"]
mod scene_crop;
#[path = "port_comparison/scene_item_names.rs"]
mod scene_item_names;
#[path = "port_comparison/scene_render_plan.rs"]
mod scene_render_plan;
#[path = "port_comparison/service.rs"]
mod service;
#[path = "port_comparison/source_async_pacing.rs"]
mod source_async_pacing;
#[path = "port_comparison/source_async_upload.rs"]
mod source_async_upload;
#[path = "port_comparison/source_audio_input.rs"]
mod source_audio_input;
#[path = "port_comparison/source_audio_render.rs"]
mod source_audio_render;
#[path = "port_comparison/source_audio_timing.rs"]
mod source_audio_timing;
#[path = "port_comparison/source_color_space.rs"]
mod source_color_space;
#[path = "port_comparison/source_conversion_plan.rs"]
mod source_conversion_plan;
#[path = "port_comparison/source_deinterlace_draw.rs"]
mod source_deinterlace_draw;
#[path = "port_comparison/source_deinterlace_pacing.rs"]
mod source_deinterlace_pacing;
#[path = "port_comparison/source_deinterlace_select.rs"]
mod source_deinterlace_select;
#[path = "port_comparison/source_draw_plan.rs"]
mod source_draw_plan;
#[path = "port_comparison/source_frame_copy.rs"]
mod source_frame_copy;
#[path = "port_comparison/source_load.rs"]
mod source_load;
#[path = "port_comparison/source_media_admission.rs"]
mod source_media_admission;
#[path = "port_comparison/source_profiler.rs"]
mod source_profiler;
#[path = "port_comparison/source_ptm_gate.rs"]
mod source_ptm_gate;
#[path = "port_comparison/source_render_route.rs"]
mod source_render_route;
#[path = "port_comparison/source_size.rs"]
mod source_size;
#[path = "port_comparison/source_transition_clock.rs"]
mod source_transition_clock;
#[path = "port_comparison/source_transition_render.rs"]
mod source_transition_render;
#[path = "port_comparison/source_volume.rs"]
mod source_volume;
#[path = "port_comparison/util.rs"]
mod util;
#[path = "port_comparison/video_copy_plan.rs"]
mod video_copy_plan;
#[path = "port_comparison/video_output_step.rs"]
mod video_output_step;
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

/// Both allocation counters: obs-util's bmalloc (which the Rust shims and
/// most oracles share) and the C bmem oracle's own.
fn allocation_counts() -> (std::ffi::c_long, std::ffi::c_long) {
    // SAFETY: each reads only its allocator's counter.
    unsafe {
        (
            obs_util::ffi::bmem::bnum_allocs(),
            obs_c_oracle::bmem::oracle_bnum_allocs(),
        )
    }
}

/// Runs one module's benches and checks they free what their fixtures
/// allocate, so a leak names its module instead of failing a later
/// `bmem/count` fixture (#2154).
fn module(criterion: &mut Criterion, name: &str, benchmarks: fn(&mut Criterion)) {
    let before = allocation_counts();
    benchmarks(criterion);
    assert_eq!(
        allocation_counts(),
        before,
        "{name} benches leave (bmalloc, oracle_bmalloc) allocations live"
    );
}

fn comparisons(criterion: &mut Criterion) {
    macro_rules! modules {
        ($($module:ident),* $(,)?) => {
            $(module(criterion, stringify!($module), $module::benchmarks);)*
        };
    }
    modules![
        audio,
        callback,
        canvas_draw,
        codecs,
        conversions,
        core_units,
        encoder_audio_start,
        encoder_roi,
        encoder_size,
        graphics,
        graphics_sprite,
        media_remux,
        module_source_rules,
        module_type_rules,
        output_video_checks,
        scene,
        scene_audio_window,
        scene_crop,
        scene_item_names,
        scene_render_plan,
        service,
        source_async_pacing,
        source_async_upload,
        source_audio_input,
        source_audio_render,
        source_audio_timing,
        source_color_space,
        source_conversion_plan,
        source_deinterlace_draw,
        source_deinterlace_pacing,
        source_deinterlace_select,
        source_draw_plan,
        source_frame_copy,
        source_load,
        source_media_admission,
        source_profiler,
        source_ptm_gate,
        source_render_route,
        source_size,
        source_transition_clock,
        source_transition_render,
        source_volume,
        util,
        video_copy_plan,
        video_output_step,
        view,
    ];
}

criterion_group!(benches, comparisons);
criterion_main!(benches);
