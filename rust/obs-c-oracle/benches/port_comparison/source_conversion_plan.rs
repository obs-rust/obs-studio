//! obs-source: the async video GPU conversion plan
//! (`libobs/obs-source-conversion-plan.c.h`), which `obs-source.c.h` asks
//! for every async frame it uploads (`upload_raw_frame()`,
//! `update_async_texrender()`, `update_async_textures()`) and when the frame
//! format or size changes (`set_async_texture_size()`). The measurement
//! folds every field of each plan (the technique by its bytes) into a
//! checksum that both sides must match:
//! - `plan`: 1024 frames, every `enum video_format` with both ranges and
//!   the four transfers, at 1920 x 1080, 1281 x 721 and 3840 x 2160.
//!
//! Measured when it was added (x86_64 Linux, GCC, not pinned to a core, a
//! loaded machine; median per 1024 plans): plan 32.18 us Rust, 20.78 us C
//! (1.55x; 31.23 us Rust in an earlier run). About 11 ns more a plan: the
//! core builds a `Plan` and the shim copies it out, where the C writes the
//! caller's struct in place. `obs-source.c.h` asks for about six plans a
//! frame of each async source (`set_async_texture_size()`,
//! `upload_raw_frame()`, `update_async_texrender()` twice and
//! `async_texture_changed()` twice), so about 65 ns a frame.

use std::ffi::{CStr, c_char};
use std::hint::black_box;
use std::mem::MaybeUninit;

use criterion::{Criterion, Throughput};
use obs_c_oracle::source_conversion_plan::{self as c, OracleConversionPlan};
use obs_source::ffi::source_conversion_plan::{self as rs, obs_source_conversion_plan};

const COUNT: usize = 1024;

#[derive(Clone, Copy)]
struct Frame {
    format: i32,
    full_range: bool,
    trc: i32,
    width: u32,
    height: u32,
}

fn frames() -> Vec<Frame> {
    const SIZES: [(u32, u32); 3] = [(1920, 1080), (1281, 721), (3840, 2160)];
    (0..COUNT)
        .map(|i| {
            let i32_of = |n: usize| i32::try_from(n).unwrap();
            let (width, height) = SIZES[(i / 7) % SIZES.len()];
            Frame {
                format: i32_of(i % 26),
                full_range: (i / 26) % 2 == 1,
                trc: i32_of((i / 52) % 4),
                width,
                height,
            }
        })
        .collect()
}

/// The fields of a plan, folded.
#[allow(clippy::too_many_arguments)]
fn fold(
    sum: u64,
    convert_type: i32,
    channel_count: i32,
    width: &[u32; 4],
    height: &[u32; 4],
    format: &[i32; 4],
    technique: *const c_char,
    linear: bool,
) -> u64 {
    let mut sum = sum
        .wrapping_mul(31)
        .wrapping_add(u64::from(convert_type.cast_unsigned()));
    sum = sum
        .wrapping_mul(31)
        .wrapping_add(u64::from(channel_count.cast_unsigned()));
    for p in 0..4 {
        sum = sum.wrapping_mul(31).wrapping_add(u64::from(width[p]));
        sum = sum.wrapping_mul(31).wrapping_add(u64::from(height[p]));
        sum = sum
            .wrapping_mul(31)
            .wrapping_add(u64::from(format[p].cast_unsigned()));
    }
    if !technique.is_null() {
        // SAFETY: a string literal of either side.
        for &b in unsafe { CStr::from_ptr(technique) }.to_bytes() {
            sum = sum.wrapping_mul(31).wrapping_add(u64::from(b));
        }
    }
    sum.wrapping_mul(31).wrapping_add(u64::from(linear))
}

fn rust(frames: &[Frame]) -> u64 {
    let mut sum = 0u64;
    for f in frames {
        let f = black_box(*f);
        let mut plan = MaybeUninit::<obs_source_conversion_plan>::uninit();
        // SAFETY: a writable plan; the shim writes all of it.
        let plan = unsafe {
            rs::obs_source_conversion_plan_fill(
                f.format,
                f.full_range,
                f.trc,
                f.width,
                f.height,
                plan.as_mut_ptr(),
            );
            plan.assume_init()
        };
        sum = fold(
            sum,
            plan.convert_type,
            plan.channel_count,
            &plan.width,
            &plan.height,
            &plan.format,
            plan.technique,
            plan.linear,
        );
    }
    sum
}

fn c(frames: &[Frame]) -> u64 {
    let mut sum = 0u64;
    for f in frames {
        let f = black_box(*f);
        let mut plan = MaybeUninit::<OracleConversionPlan>::uninit();
        // SAFETY: a writable plan; the C writes all of it.
        let plan = unsafe {
            c::oracle_obs_source_conversion_plan_fill(
                f.format,
                f.full_range,
                f.trc,
                f.width,
                f.height,
                plan.as_mut_ptr(),
            );
            plan.assume_init()
        };
        sum = fold(
            sum,
            plan.convert_type,
            plan.channel_count,
            &plan.width,
            &plan.height,
            &plan.format,
            plan.technique,
            plan.linear,
        );
    }
    sum
}

pub(crate) fn benchmarks(criterion: &mut Criterion) {
    let frames = frames();
    assert_eq!(rust(&frames), c(&frames), "benchmark fixture differs: plan");

    let mut group = criterion.benchmark_group("source_conversion_plan/plan");
    group.throughput(Throughput::Elements(COUNT as u64));
    group.bench_function("rust", |b| b.iter(|| black_box(rust(&frames))));
    group.bench_function("c", |b| b.iter(|| black_box(c(&frames))));
    group.finish();
}
