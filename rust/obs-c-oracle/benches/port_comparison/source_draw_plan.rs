//! obs-source: the source draw decisions (`libobs/obs-source-draw-plan.c.h`),
//! which `obs_source_render_async_video()` runs once a frame for each drawn
//! async source (with `rotate_async_video()` when it is rotated), and
//! `source_render()` once a frame for each drawn source with a video render
//! callback. Each measurement runs 1024 calls that reach every branch, and
//! folds the results into a checksum that both sides must match:
//! - `async_frame`: the decisions of one async draw in its order (the
//!   linear sRGB flag, the non-linear alpha blend, the technique, the white
//!   level read and the multiplier, then the translate for a rotation other
//!   than 0), over every pair of colour spaces, both linear and alpha flags,
//!   white levels of 80 to 480 nits and rotations of 0, 90, 180 and 270;
//! - `source_render`: the conversion, its technique and multiplier, over
//!   every pair of colour spaces and the same white levels.
//!
//! Neither side allocates, so the module leaves no allocation live.
//!
//! Measured when it was added (x86_64 Linux, GCC, not pinned to a core, a
//! machine at a load average of 6 to 8; median per 1024 calls, two runs):
//! async_frame 16.3 and 14.6 us Rust, 13.9 and 13.3 us C (1.1x to 1.2x),
//! about 2 ns more per async frame; source_render 8.9 and 10.0 us Rust, 7.2
//! and 7.7 us C (1.2x to 1.3x), about 2 ns more per source drawn.

use std::hint::black_box;
use std::os::raw::{c_int, c_long};

use criterion::{Criterion, Throughput};
use obs_c_oracle::source_draw_plan as c;
use obs_source::ffi::source_draw_plan as rs;

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
    source_space: c_int,
    current_space: c_int,
    gs_linear_srgb: bool,
    linear_alpha: bool,
    white: f32,
    rotation: c_long,
}

fn draws() -> Vec<Draw> {
    let mut rng = Lcg(2187);
    (0..CALLS)
        .map(|_| {
            let r = rng.next();
            Draw {
                source_space: small(r, 4),
                current_space: small(r >> 4, 4),
                gs_linear_srgb: (r >> 8).is_multiple_of(2),
                linear_alpha: (r >> 9).is_multiple_of(2),
                white: [80.0, 203.0, 300.0, 480.0][usize::try_from((r >> 12) % 4).unwrap()],
                rotation: [0, 90, 180, 270][usize::try_from((r >> 16) % 4).unwrap()],
            }
        })
        .collect()
}

struct Side {
    rotation_x: fn(c_long, u32, u32) -> f32,
    rotation_y: fn(c_long, u32, u32) -> f32,
    linear_srgb: fn(c_int, c_int, bool) -> u8,
    nonlinear_alpha: fn(c_int, bool, bool) -> u8,
    technique: fn(c_int, c_int, bool) -> u32,
    reads_white_level: fn(c_int, c_int) -> u8,
    multiplier: fn(c_int, c_int, f32) -> f32,
    convert: fn(c_int, c_int) -> u8,
}

const RUST: Side = Side {
    rotation_x: |r, w, h| rs::obs_source_draw_plan_rotation_x(r, w, h),
    rotation_y: |r, w, h| rs::obs_source_draw_plan_rotation_y(r, w, h),
    linear_srgb: |s, c, g| u8::from(rs::obs_source_draw_plan_async_linear_srgb(s, c, g)),
    nonlinear_alpha: |s, l, a| u8::from(rs::obs_source_draw_plan_async_nonlinear_alpha(s, l, a)),
    technique: |s, c, n| rs::obs_source_draw_plan_technique(s, c, n),
    reads_white_level: |s, c| u8::from(rs::obs_source_draw_plan_reads_white_level(s, c)),
    multiplier: |s, c, w| rs::obs_source_draw_plan_multiplier(s, c, w),
    convert: |s, c| u8::from(rs::obs_source_draw_plan_convert(s, c)),
};

// SAFETY (each): the oracle functions take and return plain values.
const C: Side = Side {
    rotation_x: |r, w, h| unsafe { c::oracle_obs_source_draw_plan_rotation_x(r, w, h) },
    rotation_y: |r, w, h| unsafe { c::oracle_obs_source_draw_plan_rotation_y(r, w, h) },
    linear_srgb: |s, cs, g| unsafe { c::oracle_obs_source_draw_plan_async_linear_srgb(s, cs, g) },
    nonlinear_alpha: |s, l, a| unsafe {
        c::oracle_obs_source_draw_plan_async_nonlinear_alpha(s, l, a)
    },
    technique: |s, cs, n| unsafe { c::oracle_obs_source_draw_plan_technique(s, cs, n) },
    reads_white_level: |s, cs| unsafe { c::oracle_obs_source_draw_plan_reads_white_level(s, cs) },
    multiplier: |s, cs, w| unsafe { c::oracle_obs_source_draw_plan_multiplier(s, cs, w) },
    convert: |s, cs| unsafe { c::oracle_obs_source_draw_plan_convert(s, cs) },
};

fn fold(sum: u64, value: u64) -> u64 {
    sum.wrapping_mul(31).wrapping_add(value)
}

fn multiplier(side: &Side, d: Draw) -> f32 {
    if (side.reads_white_level)(d.source_space, d.current_space) != 0 {
        (side.multiplier)(d.source_space, d.current_space, d.white)
    } else {
        1.0
    }
}

fn async_frame(side: &Side, draws: &[Draw]) -> u64 {
    let mut sum = 0u64;
    for d in draws {
        let d = black_box(*d);
        let linear = (side.linear_srgb)(d.source_space, d.current_space, d.gs_linear_srgb);
        let nonlinear = (side.nonlinear_alpha)(d.source_space, linear != 0, d.linear_alpha);
        let technique = (side.technique)(d.source_space, d.current_space, nonlinear != 0);
        let multiplier = multiplier(side, d);
        sum = fold(sum, u64::from(linear));
        sum = fold(sum, u64::from(nonlinear));
        sum = fold(sum, u64::from(technique));
        sum = fold(sum, u64::from(multiplier.to_bits()));
        if d.rotation != 0 {
            sum = fold(
                sum,
                u64::from((side.rotation_x)(d.rotation, 1920, 1080).to_bits()),
            );
            sum = fold(
                sum,
                u64::from((side.rotation_y)(d.rotation, 1920, 1080).to_bits()),
            );
        }
    }
    sum
}

fn source_render(side: &Side, draws: &[Draw]) -> u64 {
    let mut sum = 0u64;
    for d in draws {
        let d = black_box(*d);
        let convert = (side.convert)(d.source_space, d.current_space);
        let technique = (side.technique)(d.source_space, d.current_space, false);
        let multiplier = multiplier(side, d);
        sum = fold(sum, u64::from(convert));
        if convert != 0 {
            sum = fold(sum, u64::from(technique));
            sum = fold(sum, u64::from(multiplier.to_bits()));
        }
    }
    sum
}

pub(crate) fn benchmarks(criterion: &mut Criterion) {
    let draws = draws();
    for (name, run) in [
        ("async_frame", async_frame as fn(&Side, &[Draw]) -> u64),
        ("source_render", source_render),
    ] {
        assert_eq!(
            run(&RUST, &draws),
            run(&C, &draws),
            "benchmark fixture differs: {name}"
        );
        let mut group = criterion.benchmark_group(format!("source_draw_plan/{name}"));
        group.throughput(Throughput::Elements(CALLS as u64));
        group.bench_function("rust", |b| b.iter(|| black_box(run(&RUST, &draws))));
        group.bench_function("c", |b| b.iter(|| black_box(run(&C, &draws))));
        group.finish();
    }
}
