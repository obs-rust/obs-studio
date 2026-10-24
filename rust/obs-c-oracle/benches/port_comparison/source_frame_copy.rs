//! obs-source: the async frame copy plan and cache decisions
//! (`libobs/obs-source-frame-copy.c.h`), which every frame an async source
//! outputs goes through: `obs_source_output_video()` (the range rule, the
//! cache step, the clean step of each cache entry and the copy plan),
//! `obs_source_preload_video()` and `obs_source_set_video_frame()` (the
//! preload rule). Each measurement folds its results into a checksum that
//! both sides must match:
//! - `plan`: 1024 copy plans, every video format and two values outside the
//!   enum, at 1920 x 1080, 1281 x 721 and 3840 x 2160, with the line sizes
//!   equal and with the source's padded;
//! - `cache`: 1024 frames' decisions: the range rule (`*2()` too), the cache
//!   step at queue lengths around the limit, the clean step of 4 entries and
//!   the preload rule.
//!
//! Neither side allocates.
//!
//! Measured when it was added (x86_64 Linux, GCC, not pinned to a core, a
//! heavily loaded machine with load average 45; median per 1024 calls):
//! plan 62.24 us Rust, 39.48 us C (1.58x); cache 39.00 us Rust, 48.34 us C
//! (0.81x). About 22 ns more a plan, as the core builds the plan and the
//! shim copies it out; a frame asks for one plan and one set of decisions,
//! so well under 0.1 us a frame.

use std::ffi::{c_int, c_long};
use std::hint::black_box;

use criterion::{Criterion, Throughput};
use obs_c_oracle::source_frame_copy::{
    self as c, OracleCopyPlan, OracleKey, OracleLines, OracleShape,
};
use obs_source::ffi::source_frame_copy::{
    self as rs, obs_source_frame_copy_key, obs_source_frame_copy_lines, obs_source_frame_copy_plan,
    obs_source_frame_copy_shape,
};

const COUNT: usize = 1024;

#[derive(Clone, Copy)]
struct PlanInput {
    format: c_int,
    full_range: bool,
    height: u32,
    src: [u32; 4],
    dst: [u32; 4],
}

#[derive(Clone, Copy)]
struct CacheInput {
    format: c_int,
    full_range: bool,
    range: c_int,
    queued: usize,
    cache: (u32, u32, c_int),
    frame: (u32, u32, c_int),
    used: [bool; 4],
    unused: [c_long; 4],
    has_preload: bool,
    preload: (c_int, u32, u32),
}

fn index(i: usize, n: usize) -> u32 {
    u32::try_from(i % n).unwrap()
}

fn plans() -> Vec<PlanInput> {
    let sizes = [(1920u32, 1080u32), (1281, 721), (3840, 2160)];
    (0..COUNT)
        .map(|i| {
            // formats 0 to 25 and two outside the enum
            let format = i32::try_from(i % 28).unwrap() - 1;
            let (width, height) = sizes[(i / 28) % 3];
            let dst = [width * 2, width, width.div_ceil(2), width];
            let pad = if (i / 84) % 2 == 1 { 64 } else { 0 };
            PlanInput {
                format,
                full_range: i % 2 == 0,
                height,
                src: dst.map(|l| l + pad),
                dst,
            }
        })
        .collect()
}

fn caches() -> Vec<CacheInput> {
    (0..COUNT)
        .map(|i| {
            let format = i32::try_from(i % 26).unwrap();
            let size = 1920 + index(i, 3);
            CacheInput {
                format,
                full_range: i % 3 == 0,
                range: i32::try_from(i % 4).unwrap(),
                queued: 27 + (i % 6),
                cache: (1920, 1080, 1),
                frame: (size, 1080, i32::try_from(i % 2).unwrap()),
                used: [i % 2 == 0, i % 3 == 0, false, true],
                unused: [0, 4, c_long::from(u8::try_from(i % 5).unwrap()), 2],
                has_preload: i % 5 != 0,
                preload: (format, 1920, 1080),
            }
        })
        .collect()
}

type Fields = (bool, usize, [(bool, usize, u32, u32, u32); 4]);

struct Side {
    plan: fn(PlanInput) -> Fields,
    cache: fn(CacheInput) -> u64,
}

const RUST: Side = Side {
    plan: |p| {
        let plan: obs_source_frame_copy_plan = rs::obs_source_frame_copy_plan(
            p.format,
            p.full_range,
            p.height,
            obs_source_frame_copy_lines { linesize: p.src },
            obs_source_frame_copy_lines { linesize: p.dst },
        );
        (
            plan.color_range,
            plan.count,
            plan.planes
                .map(|c| (c.whole, c.bytes, c.rows, c.src_stride, c.dst_stride)),
        )
    },
    cache: |x| {
        let key = |(width, height, convert_type)| obs_source_frame_copy_key {
            width,
            height,
            convert_type,
        };
        let shape = |(format, width, height)| obs_source_frame_copy_shape {
            format,
            width,
            height,
        };
        let mut sum = u64::from(rs::obs_source_frame_copy_full_range(x.format, x.full_range));
        sum = sum * 2 + u64::from(rs::obs_source_frame_copy_full_range2(x.format, x.range));
        sum = sum * 4
            + u64::from(rs::obs_source_frame_copy_cache_step(
                x.queued,
                key(x.cache),
                key(x.frame),
            ));
        for (used, unused) in x.used.into_iter().zip(x.unused) {
            let age = rs::obs_source_frame_copy_cache_clean(used, unused);
            sum = sum * 31
                + u64::from(age.counted)
                + 2 * u64::from(age.drop)
                + u64::try_from(age.unused_count).unwrap_or(0);
        }
        sum * 2
            + u64::from(rs::obs_source_frame_copy_preload_changed(
                x.has_preload,
                shape(x.preload),
                shape((x.format, x.frame.0, 1080)),
            ))
    },
};

const C: Side = Side {
    plan: |p| {
        // SAFETY: plain values.
        let plan: OracleCopyPlan = unsafe {
            c::oracle_obs_source_frame_copy_plan(
                p.format,
                p.full_range,
                p.height,
                OracleLines { linesize: p.src },
                OracleLines { linesize: p.dst },
            )
        };
        (
            plan.color_range,
            plan.count,
            plan.planes
                .map(|c| (c.whole, c.bytes, c.rows, c.src_stride, c.dst_stride)),
        )
    },
    // SAFETY: plain values; no count is LONG_MAX.
    cache: |x| unsafe {
        let key = |(width, height, convert_type)| OracleKey {
            width,
            height,
            convert_type,
        };
        let shape = |(format, width, height)| OracleShape {
            format,
            width,
            height,
        };
        let mut sum = u64::from(c::oracle_obs_source_frame_copy_full_range(
            x.format,
            x.full_range,
        ));
        sum = sum * 2
            + u64::from(c::oracle_obs_source_frame_copy_full_range2(
                x.format, x.range,
            ));
        sum = sum * 4
            + u64::from(c::oracle_obs_source_frame_copy_cache_step(
                x.queued,
                key(x.cache),
                key(x.frame),
            ));
        for (used, unused) in x.used.into_iter().zip(x.unused) {
            let age = c::oracle_obs_source_frame_copy_cache_clean(used, unused);
            sum = sum * 31
                + u64::from(age.counted)
                + 2 * u64::from(age.drop)
                + u64::try_from(age.unused_count).unwrap_or(0);
        }
        sum * 2
            + u64::from(c::oracle_obs_source_frame_copy_preload_changed(
                x.has_preload,
                shape(x.preload),
                shape((x.format, x.frame.0, 1080)),
            ))
    },
};

fn plan(side: &Side, inputs: &[PlanInput]) -> u64 {
    let mut sum = 0u64;
    for input in inputs {
        let (color_range, count, planes) = (side.plan)(black_box(*input));
        sum = sum
            .wrapping_mul(31)
            .wrapping_add(u64::from(color_range))
            .wrapping_add(u64::try_from(count).unwrap());
        for (whole, bytes, rows, src, dst) in planes {
            sum = sum
                .wrapping_mul(31)
                .wrapping_add(u64::from(whole))
                .wrapping_add(u64::try_from(bytes).unwrap())
                .wrapping_add(u64::from(rows))
                .wrapping_add(u64::from(src) << 1)
                .wrapping_add(u64::from(dst) << 2);
        }
    }
    sum
}

fn cache(side: &Side, inputs: &[CacheInput]) -> u64 {
    let mut sum = 0u64;
    for input in inputs {
        sum = sum
            .wrapping_mul(31)
            .wrapping_add((side.cache)(black_box(*input)));
    }
    sum
}

pub(crate) fn benchmarks(criterion: &mut Criterion) {
    let plans = plans();
    let caches = caches();
    assert_eq!(
        plan(&RUST, &plans),
        plan(&C, &plans),
        "benchmark fixture differs: plan"
    );
    assert_eq!(
        cache(&RUST, &caches),
        cache(&C, &caches),
        "benchmark fixture differs: cache"
    );

    let mut group = criterion.benchmark_group("source_frame_copy/plan");
    group.throughput(Throughput::Elements(COUNT as u64));
    group.bench_function("rust", |b| b.iter(|| black_box(plan(&RUST, &plans))));
    group.bench_function("c", |b| b.iter(|| black_box(plan(&C, &plans))));
    group.finish();

    let mut group = criterion.benchmark_group("source_frame_copy/cache");
    group.throughput(Throughput::Elements(COUNT as u64));
    group.bench_function("rust", |b| b.iter(|| black_box(cache(&RUST, &caches))));
    group.bench_function("c", |b| b.iter(|| black_box(cache(&C, &caches))));
    group.finish();
}
