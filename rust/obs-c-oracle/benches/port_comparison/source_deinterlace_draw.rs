//! obs-source: the deinterlace draw decisions
//! (`libobs/obs-source-deinterlace-draw.c.h`), which `deinterlace_render()`
//! runs once a frame for each deinterlaced async source, and
//! `obs_source_set_deinterlace_mode()` and `get_effect()` when the mode
//! changes. Each measurement runs 1024 calls that reach every branch, and
//! folds the results into a checksum that both sides must match:
//! - `render_frame`: the decisions of one `deinterlace_render()` in its
//!   order (the early return, the linear sRGB read and flag, the technique,
//!   the white level read, the multiplier and frame2), over every pair of
//!   colour spaces, the nine modes, white levels of 80 to 480 nits and
//!   field times either side of the threshold;
//! - `mode_change`: the mode change and the new mode's effect, over every
//!   pair of the nine modes and two outside the enum.
//!
//! Measured when it was added (x86_64 Linux, GCC, not pinned to a core, a
//! machine at a load average of 13 to 17; median per 1024 calls, two runs):
//! render_frame 25.2 and 25.8 us Rust, 13.9 and 13.3 us C (1.8x to 1.9x),
//! about 12 ns more per render, once a frame for each deinterlaced source;
//! mode_change 4.1 and 5.8 us Rust, 6.0 and 4.2 us C, within the noise.

use std::hint::black_box;
use std::os::raw::c_int;

use criterion::{Criterion, Throughput};
use obs_c_oracle::source_deinterlace_draw as c;
use obs_source::ffi::source_deinterlace_draw as rs;

const CALLS: usize = 1024;
const VT: u64 = 1_000_000_000_000;
const MS: u64 = 1_000_000;

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
struct Render {
    has_tex: bool,
    width: u32,
    source_space: c_int,
    current_space: c_int,
    gs_linear_srgb: bool,
    mode: c_int,
    white: f32,
    frame_ts: u64,
    offset: u64,
    half: u32,
}

fn renders() -> Vec<Render> {
    let mut rng = Lcg(2130);
    (0..CALLS)
        .map(|i| {
            let r = rng.next();
            let n = u64::try_from(i).unwrap();
            let frame_ts = 5_000_000_000 + n * 20 * MS;
            Render {
                // one in 64 calls returns early
                has_tex: !r.is_multiple_of(64),
                width: if r % 128 == 1 { 0 } else { 1920 },
                source_space: small(r >> 8, 4),
                current_space: small(r >> 12, 4),
                gs_linear_srgb: (r >> 16).is_multiple_of(2),
                mode: small(r >> 20, 9),
                white: [80.0, 203.0, 300.0, 480.0][usize::try_from((r >> 24) % 4).unwrap()],
                frame_ts,
                // the second field due, or 2 ms before it is
                offset: VT - frame_ts - 19 * MS + ((r >> 28) % 2) * 2 * MS,
                half: 20_000_000,
            }
        })
        .collect()
}

struct Side {
    skip: fn(bool, bool, u32, u32) -> u8,
    reads_linear_srgb: fn(c_int) -> u8,
    linear_srgb: fn(c_int, bool, c_int) -> u8,
    technique: fn(c_int, c_int) -> u32,
    reads_white_level: fn(c_int, c_int) -> u8,
    multiplier: fn(c_int, c_int, f32) -> f32,
    frame2: fn(u64, u64, u64, u32) -> u8,
    mode_change: fn(c_int, c_int) -> u32,
    effect: fn(c_int) -> c_int,
}

const RUST: Side = Side {
    skip: |a, b, w, h| u8::from(rs::obs_source_deinterlace_draw_skip(a, b, w, h)),
    reads_linear_srgb: |s| u8::from(rs::obs_source_deinterlace_draw_reads_linear_srgb(s)),
    linear_srgb: |s, g, m| u8::from(rs::obs_source_deinterlace_draw_linear_srgb(s, g, m)),
    technique: |s, c| rs::obs_source_deinterlace_draw_technique(s, c),
    reads_white_level: |s, c| u8::from(rs::obs_source_deinterlace_draw_reads_white_level(s, c)),
    multiplier: |s, c, w| rs::obs_source_deinterlace_draw_multiplier(s, c, w),
    frame2: |v, f, o, h| u8::from(rs::obs_source_deinterlace_draw_frame2(v, f, o, h)),
    mode_change: |a, b| rs::obs_source_deinterlace_draw_mode_change(a, b),
    effect: |m| rs::obs_source_deinterlace_draw_effect(m),
};

// SAFETY (each): the oracle functions take and return plain values.
const C: Side = Side {
    skip: |a, b, w, h| unsafe { c::oracle_obs_source_deinterlace_draw_skip(a, b, w, h) },
    reads_linear_srgb: |s| unsafe { c::oracle_obs_source_deinterlace_draw_reads_linear_srgb(s) },
    linear_srgb: |s, g, m| unsafe { c::oracle_obs_source_deinterlace_draw_linear_srgb(s, g, m) },
    technique: |s, cs| unsafe { c::oracle_obs_source_deinterlace_draw_technique(s, cs) },
    reads_white_level: |s, cs| unsafe {
        c::oracle_obs_source_deinterlace_draw_reads_white_level(s, cs)
    },
    multiplier: |s, cs, w| unsafe { c::oracle_obs_source_deinterlace_draw_multiplier(s, cs, w) },
    frame2: |v, f, o, h| unsafe { c::oracle_obs_source_deinterlace_draw_frame2(v, f, o, h) },
    mode_change: |a, b| unsafe { c::oracle_obs_source_deinterlace_draw_mode_change(a, b) },
    effect: |m| unsafe { c::oracle_obs_source_deinterlace_draw_effect(m) },
};

fn fold(sum: u64, value: u64) -> u64 {
    sum.wrapping_mul(31).wrapping_add(value)
}

fn render_frame(side: &Side, renders: &[Render]) -> u64 {
    let mut sum = 0u64;
    for r in renders {
        let r = black_box(*r);
        if (side.skip)(r.has_tex, true, r.width, 1080) != 0 {
            sum = fold(sum, 1);
            continue;
        }
        let gs = (side.reads_linear_srgb)(r.source_space) != 0 && r.gs_linear_srgb;
        let linear = (side.linear_srgb)(r.source_space, gs, r.mode);
        let technique = (side.technique)(r.source_space, r.current_space);
        let multiplier = if (side.reads_white_level)(r.source_space, r.current_space) != 0 {
            (side.multiplier)(r.source_space, r.current_space, r.white)
        } else {
            1.0
        };
        let frame2 = (side.frame2)(VT, r.frame_ts, r.offset, r.half);
        sum = fold(sum, u64::from(linear));
        sum = fold(sum, u64::from(technique));
        sum = fold(sum, u64::from(multiplier.to_bits()));
        sum = fold(sum, u64::from(frame2));
    }
    sum
}

fn mode_change(side: &Side, renders: &[Render]) -> u64 {
    let mut sum = 0u64;
    for (i, r) in renders.iter().enumerate() {
        let r = black_box(*r);
        let current = r.mode;
        // every new mode in turn, and two outside the enum
        let mode = match i % 11 {
            9 => -1,
            10 => 99,
            k => c_int::try_from(k).unwrap(),
        };
        let change = (side.mode_change)(current, mode);
        let effect = (side.effect)(mode);
        sum = fold(sum, u64::from(change));
        sum = fold(sum, u64::from(effect.unsigned_abs()));
    }
    sum
}

pub(crate) fn benchmarks(criterion: &mut Criterion) {
    let renders = renders();
    for (name, run) in [
        ("render_frame", render_frame as fn(&Side, &[Render]) -> u64),
        ("mode_change", mode_change),
    ] {
        assert_eq!(
            run(&RUST, &renders),
            run(&C, &renders),
            "benchmark fixture differs: {name}"
        );
        let mut group = criterion.benchmark_group(format!("source_deinterlace_draw/{name}"));
        group.throughput(Throughput::Elements(CALLS as u64));
        group.bench_function("rust", |b| b.iter(|| black_box(run(&RUST, &renders))));
        group.bench_function("c", |b| b.iter(|| black_box(run(&C, &renders))));
        group.finish();
    }
}
