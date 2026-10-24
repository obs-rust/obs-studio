//! obs-core: the canvas texture draw and display clear decisions
//! (`libobs/obs-canvas-draw.c.h`), which `obs_render_canvas_texture_internal()`
//! runs once a frame for each canvas drawn into a display, and
//! `render_display_begin()` once a frame for each display. Each measurement
//! runs 1024 calls that reach every branch, and folds the results into a
//! checksum that both sides must match:
//! - `canvas_texture`: the decisions of one canvas draw in its order (the
//!   skip, the technique, the white level read and the multiplier only when
//!   it is read), over both skip flags, every pair of colour spaces and
//!   white levels of 80 to 480 nits;
//! - `display_begin`: the clear colour (the host's answer, then the space
//!   only when it is read), the clear flags and the workaround, over both
//!   host answers, every colour space and both workaround values.
//!
//! Neither side allocates, so the module leaves no allocation live.
//!
//! Measured when it was added (x86_64 Linux, GCC, not pinned to a core, a
//! machine at a load average of 29 to 35; median per 1024 calls, two runs):
//! canvas_texture 10.4 and 8.7 us Rust, 5.6 and 13.4 us C; display_begin
//! 7.8 and 8.8 us Rust, 5.8 and 13.4 us C. About 5 to 13 ns a call either
//! way, within the noise of that host.

use std::hint::black_box;
use std::os::raw::c_int;

use criterion::{Criterion, Throughput};
use obs_c_oracle::canvas_draw as c;
use obs_core::ffi::canvas_draw as rs;

const CALLS: usize = 1024;

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

fn small(r: u64, n: u64) -> c_int {
    c_int::try_from(r % n).unwrap()
}

#[derive(Clone, Copy)]
struct Draw {
    has_mix: bool,
    rendered: bool,
    source_space: c_int,
    current_space: c_int,
    white: f32,
    always_srgb: bool,
    workaround: bool,
}

fn draws() -> Vec<Draw> {
    let mut rng = Lcg(2240);
    (0..CALLS)
        .map(|_| {
            let r = rng.next();
            Draw {
                // mostly drawn, as a live canvas is
                has_mix: !(r & 15).is_multiple_of(16),
                rendered: !((r >> 20) & 7).is_multiple_of(8),
                source_space: small(r >> 4, 4),
                current_space: small(r >> 8, 4),
                white: [80.0, 203.0, 300.0, 480.0][usize::try_from((r >> 12) % 4).unwrap()],
                always_srgb: (r >> 14).is_multiple_of(2),
                workaround: (r >> 15).is_multiple_of(2),
            }
        })
        .collect()
}

struct Side {
    skip: fn(bool, bool) -> u8,
    technique: fn(c_int, c_int) -> u32,
    reads_white_level: fn(c_int) -> u8,
    multiplier: fn(f32) -> f32,
    reads_space: fn(bool) -> u8,
    linear: fn(c_int) -> u8,
    flags: fn(bool) -> u32,
    draws_solid: fn(bool) -> u8,
}

const RUST: Side = Side {
    skip: |m, t| u8::from(rs::obs_canvas_draw_skip(m, t)),
    technique: |s, c| rs::obs_canvas_draw_technique(s, c),
    reads_white_level: |c| u8::from(rs::obs_canvas_draw_reads_white_level(c)),
    multiplier: |w| rs::obs_canvas_draw_multiplier(w),
    reads_space: |a| u8::from(rs::obs_display_clear_reads_space(a)),
    linear: |c| u8::from(rs::obs_display_clear_linear(c)),
    flags: |w| rs::obs_display_clear_flags(w),
    draws_solid: |w| u8::from(rs::obs_display_clear_draws_solid(w)),
};

// SAFETY (each): the oracle functions take and return plain values.
const C: Side = Side {
    skip: |m, t| unsafe { c::oracle_obs_canvas_draw_skip(m, t) },
    technique: |s, cs| unsafe { c::oracle_obs_canvas_draw_technique(s, cs) },
    reads_white_level: |cs| unsafe { c::oracle_obs_canvas_draw_reads_white_level(cs) },
    multiplier: |w| unsafe { c::oracle_obs_canvas_draw_multiplier(w) },
    reads_space: |a| unsafe { c::oracle_obs_display_clear_reads_space(a) },
    linear: |cs| unsafe { c::oracle_obs_display_clear_linear(cs) },
    flags: |w| unsafe { c::oracle_obs_display_clear_flags(w) },
    draws_solid: |w| unsafe { c::oracle_obs_display_clear_draws_solid(w) },
};

fn fold(sum: u64, value: u64) -> u64 {
    sum.wrapping_mul(31).wrapping_add(value)
}

fn canvas_texture(side: &Side, draws: &[Draw]) -> u64 {
    let mut sum = 0u64;
    for d in draws {
        let d = black_box(*d);
        let skip = (side.skip)(d.has_mix, d.has_mix && d.rendered);
        sum = fold(sum, u64::from(skip));
        if skip != 0 {
            continue;
        }
        let technique = (side.technique)(d.source_space, d.current_space);
        let multiplier = if (side.reads_white_level)(d.current_space) != 0 {
            (side.multiplier)(d.white)
        } else {
            1.0
        };
        sum = fold(sum, u64::from(technique));
        sum = fold(sum, u64::from(multiplier.to_bits()));
    }
    sum
}

fn display_begin(side: &Side, draws: &[Draw]) -> u64 {
    let mut sum = 0u64;
    for d in draws {
        let d = black_box(*d);
        let linear = (side.reads_space)(d.always_srgb) != 0 && (side.linear)(d.current_space) != 0;
        sum = fold(sum, u64::from(linear));
        sum = fold(sum, u64::from((side.flags)(d.workaround)));
        sum = fold(sum, u64::from((side.draws_solid)(d.workaround)));
    }
    sum
}

pub(crate) fn benchmarks(criterion: &mut Criterion) {
    let draws = draws();
    for (name, run) in [
        (
            "canvas_texture",
            canvas_texture as fn(&Side, &[Draw]) -> u64,
        ),
        ("display_begin", display_begin),
    ] {
        assert_eq!(
            run(&RUST, &draws),
            run(&C, &draws),
            "benchmark fixture differs: {name}"
        );
        let mut group = criterion.benchmark_group(format!("canvas_draw/{name}"));
        group.throughput(Throughput::Elements(CALLS as u64));
        group.bench_function("rust", |b| b.iter(|| black_box(run(&RUST, &draws))));
        group.bench_function("c", |b| b.iter(|| black_box(run(&C, &draws))));
        group.finish();
    }
}
