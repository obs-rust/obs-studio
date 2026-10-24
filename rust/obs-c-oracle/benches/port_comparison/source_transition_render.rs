//! obs-source: the transition render and audio steps
//! (`libobs/obs-source-transition-render.c.h`). The video steps run for
//! every frame a transition renders, the offsets on every tick for each
//! child, and the audio steps for every audio tick a transition mixes. Each
//! measurement folds its results into a checksum that both sides must
//! match. The fixtures allocate nothing:
//! - `render`: 1024 frames of a transition, as render2 and the direct
//!   render ask: the due test over a clock running past 1, the video stop
//!   step where it is due, the draw choice, the direct index, and the two
//!   children's offsets (each scale type's scale in turn);
//! - `audio`: 1024 audio ticks: the audio stop step with and without a
//!   timestamp, the mix, and the validity check and start frame of each of
//!   the two children at 48 kHz around min_ts.
//!
//! Measured when it was added (x86_64 Linux, GCC, not pinned to a core,
//! load average 6; median per 1024 calls): render 12.46 us Rust, 12.83 us C
//! (0.97x); audio 10.81 us Rust, 11.11 us C (0.97x). The outputs are the
//! same bits.

use std::hint::black_box;

use criterion::{Criterion, Throughput};
use obs_c_oracle::source_transition_render as c;
use obs_source::ffi::source_transition_render as rs;

const COUNT: usize = 1024;

struct Side {
    due: fn(f32, bool) -> bool,
    stop: fn(bool, bool) -> u32,
    draw: fn(bool, bool, bool, bool) -> u32,
    index: fn(bool, i32) -> i32,
    audio_stop: fn(bool, f32, bool, bool) -> u32,
    mix: fn(bool, bool, bool) -> u32,
    valid: fn(bool, bool, bool) -> bool,
    place: fn(u64, u64, usize, &mut usize) -> u32,
    offset: fn(f32, f32, f32, f32, f32, f32) -> (f32, f32),
}

const RUST: Side = Side {
    due: |t, v| rs::obs_source_transition_render_video_due(t, v),
    stop: |a, act| rs::obs_source_transition_render_video_stop(a, act),
    draw: |v, l, cb, a| rs::obs_source_transition_render_draw(v, l, cb, a),
    index: |tr, target| rs::obs_source_transition_render_direct_index(tr, target),
    audio_stop: |h, t, v, a| rs::obs_source_transition_render_audio_stop(h, t, v, a),
    mix: |a, x, y| rs::obs_source_transition_render_audio_mix(a, x, y),
    valid: |p, pe, d| rs::obs_source_transition_render_audio_valid(p, pe, d),
    // SAFETY: pos is a valid size_t.
    place: |ts, min, rate, pos| unsafe {
        rs::obs_source_transition_render_audio_place(ts, min, rate, pos)
    },
    offset: |a, b, cx, cy, x, y| {
        let o = rs::obs_source_transition_render_offset(a, b, cx, cy, x, y);
        (o.x, o.y)
    },
};

const C: Side = Side {
    // SAFETY: plain values.
    due: |t, v| unsafe { c::oracle_obs_source_transition_render_video_due(t, v) },
    // SAFETY: plain values.
    stop: |a, act| unsafe { c::oracle_obs_source_transition_render_video_stop(a, act) },
    // SAFETY: plain values.
    draw: |v, l, cb, a| unsafe { c::oracle_obs_source_transition_render_draw(v, l, cb, a) },
    // SAFETY: plain values.
    index: |tr, target| unsafe { c::oracle_obs_source_transition_render_direct_index(tr, target) },
    // SAFETY: plain values.
    audio_stop: |h, t, v, a| unsafe {
        c::oracle_obs_source_transition_render_audio_stop(h, t, v, a)
    },
    // SAFETY: plain values.
    mix: |a, x, y| unsafe { c::oracle_obs_source_transition_render_audio_mix(a, x, y) },
    // SAFETY: plain values.
    valid: |p, pe, d| unsafe { c::oracle_obs_source_transition_render_audio_valid(p, pe, d) },
    // SAFETY: pos is a valid size_t.
    place: |ts, min, rate, pos| unsafe {
        c::oracle_obs_source_transition_render_audio_place(ts, min, rate, pos)
    },
    offset: |a, b, cx, cy, x, y| {
        // SAFETY: plain values.
        let o = unsafe { c::oracle_obs_source_transition_render_offset(a, b, cx, cy, x, y) };
        (o.x, o.y)
    },
};

fn fold(sum: u64, bits: u32) -> u64 {
    sum.wrapping_mul(31).wrapping_add(u64::from(bits))
}

/// Children's sizes (a 1080p and a 4K scene, a 720p camera, a vertical
/// video) and the scale each scale type gives them in 1920x1080.
/// A child: its width, its height and its scale for each scale type.
type Child = (f32, f32, [(f32, f32); 3]);

const CHILDREN: [Child; 4] = [
    (1920.0, 1080.0, [(1.0, 1.0), (1.0, 1.0), (1.0, 1.0)]),
    (3840.0, 2160.0, [(0.5, 0.5), (0.5, 0.5), (0.5, 0.5)]),
    (1280.0, 720.0, [(1.0, 1.0), (1.5, 1.5), (1.5, 1.5)]),
    (
        1080.0,
        1920.0,
        [(0.5625, 0.5625), (0.5625, 0.5625), (1.777_778, 0.5625)],
    ),
];

fn render(side: &Side) -> u64 {
    let mut sum = 0u64;
    let mut video = true;
    for i in 0..COUNT {
        // a clock from 0 past 1 over the frames
        let t = i as f32 / 1000.0;
        let audio = i % 3 != 0;
        if (side.due)(black_box(t), video) {
            sum = fold(sum, (side.stop)(audio, i % 5 != 0));
            video = false;
        }
        sum = fold(sum, (side.draw)(video, i % 7 != 0, true, audio));
        sum = fold(
            sum,
            (side.index)(video || audio, black_box(1)).cast_unsigned(),
        );
        // recalculate_transition_matrices(): both children
        for child in [CHILDREN[i % 4], CHILDREN[(i / 4) % 4]] {
            let scale = child.2[i % 3];
            let (x, y) = (side.offset)(
                black_box(1920.0),
                black_box(1080.0),
                child.0,
                child.1,
                scale.0,
                scale.1,
            );
            sum = fold(fold(sum, x.to_bits()), y.to_bits());
        }
        if i % 256 == 255 {
            video = true;
        }
    }
    sum
}

fn audio(side: &Side) -> u64 {
    let mut sum = 0u64;
    let min_ts = 3_600_000_000_000u64;
    for i in 0..COUNT {
        let t = i as f32 / 1000.0;
        let has_ts = i % 16 != 0;
        sum = fold(
            sum,
            (side.audio_stop)(has_ts, black_box(t), i % 2 == 0, true),
        );
        sum = fold(sum, (side.mix)(i % 4 != 0, true, i % 3 != 0));
        for (k, offset) in [0u64, 21_333_334 + (i as u64 % 64) * 331]
            .into_iter()
            .enumerate()
        {
            if (side.valid)(true, i % 9 == k, false) {
                let mut pos = 0usize;
                let place = (side.place)(black_box(min_ts + offset), min_ts, 48_000, &mut pos);
                sum = fold(fold(sum, place), u32::try_from(pos % 4096).unwrap());
            }
        }
    }
    sum
}

pub(crate) fn benchmarks(criterion: &mut Criterion) {
    assert_eq!(
        render(&RUST),
        render(&C),
        "benchmark fixture differs: render"
    );
    assert_eq!(audio(&RUST), audio(&C), "benchmark fixture differs: audio");

    let mut group = criterion.benchmark_group("source_transition_render/render");
    group.throughput(Throughput::Elements(COUNT as u64));
    group.bench_function("rust", |b| b.iter(|| black_box(render(&RUST))));
    group.bench_function("c", |b| b.iter(|| black_box(render(&C))));
    group.finish();

    let mut group = criterion.benchmark_group("source_transition_render/audio");
    group.throughput(Throughput::Elements(COUNT as u64));
    group.bench_function("rust", |b| b.iter(|| black_box(audio(&RUST))));
    group.bench_function("c", |b| b.iter(|| black_box(audio(&C))));
    group.finish();
}
