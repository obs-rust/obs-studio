//! obs-source: the size query routing (`libobs/obs-source-size.c.h`), which
//! `get_base_width()` and `get_base_height()` run for every source and
//! filter they ask, from `obs_source_get_width()` and
//! `obs_source_get_height()` (every scene item, every frame) and the
//! render. Each measurement folds its results into a checksum that both
//! sides must match:
//! - `route`: 1024 nodes, every combination of data, a callback, a filter
//!   parent, enabled and async active, for an input, a filter, a
//!   transition and a scene, at eight rotations (0, 90, 180, 270, 45, -90,
//!   and the ends of `long`);
//! - `entry`: 1024 starts, the four types with and without filters.
//!
//! Measured when it was added (x86_64 Linux, GCC, not pinned to a core, a
//! loaded machine; median per 1024 calls): route 9.21 us Rust, 10.14 us C
//! (0.91x; 14.47 us C in an earlier, noisier run); entry 1.59 us Rust,
//! 1.53 us C (1.04x; 3.66 us Rust in the earlier run). About 9 ns a route
//! and 1.5 ns a start either way, within the noise of the run.

use std::ffi::c_long;
use std::hint::black_box;

use criterion::{Criterion, Throughput};
use obs_c_oracle::source_size::{self as c, OracleSizeNode};
use obs_source::ffi::source_size::{self as rs, obs_source_size_node};

const COUNT: usize = 1024;

const ROTATIONS: [c_long; 8] = [0, 90, 180, 270, 45, -90, c_long::MIN, c_long::MAX];

#[derive(Clone, Copy)]
struct Fields {
    source_type: i32,
    has_data: bool,
    has_callback: bool,
    has_filter_parent: bool,
    enabled: bool,
    async_active: bool,
    async_rotation: c_long,
}

fn nodes() -> Vec<Fields> {
    (0..COUNT)
        .map(|i| {
            let bits = i % 32;
            let bit = |n: usize| bits & (1 << n) != 0;
            Fields {
                source_type: i32::try_from((i / 32) % 4).unwrap(),
                has_data: bit(4),
                has_callback: bit(3),
                has_filter_parent: bit(2),
                enabled: bit(1),
                async_active: bit(0),
                async_rotation: ROTATIONS[i / 128],
            }
        })
        .collect()
}

fn starts() -> Vec<(i32, bool)> {
    (0..COUNT)
        .map(|i| (i32::try_from(i % 4).unwrap(), (i / 4) % 2 == 1))
        .collect()
}

struct Side {
    route: fn(Fields) -> u32,
    entry: fn(i32, bool) -> u32,
}

const RUST: Side = Side {
    route: |f| {
        rs::obs_source_size_route(obs_source_size_node {
            source_type: f.source_type,
            has_data: f.has_data,
            has_callback: f.has_callback,
            has_filter_parent: f.has_filter_parent,
            enabled: f.enabled,
            async_active: f.async_active,
            async_rotation: f.async_rotation,
        })
    },
    entry: |source_type, has_filters| rs::obs_source_size_entry(source_type, has_filters),
};

const C: Side = Side {
    // SAFETY: plain values.
    route: |f| unsafe {
        c::oracle_obs_source_size_route(OracleSizeNode {
            source_type: f.source_type,
            has_data: f.has_data,
            has_callback: f.has_callback,
            has_filter_parent: f.has_filter_parent,
            enabled: f.enabled,
            async_active: f.async_active,
            async_rotation: f.async_rotation,
        })
    },
    // SAFETY: plain values.
    entry: |source_type, has_filters| unsafe {
        c::oracle_obs_source_size_entry(source_type, has_filters)
    },
};

fn route(side: &Side, nodes: &[Fields]) -> u64 {
    let mut sum = 0u64;
    for node in nodes {
        let route = (side.route)(black_box(*node));
        sum = sum.wrapping_mul(31).wrapping_add(u64::from(route));
    }
    sum
}

fn entry(side: &Side, starts: &[(i32, bool)]) -> u64 {
    let mut sum = 0u64;
    for &(source_type, has_filters) in starts {
        let start = (side.entry)(black_box(source_type), black_box(has_filters));
        sum = sum.wrapping_mul(31).wrapping_add(u64::from(start));
    }
    sum
}

pub(crate) fn benchmarks(criterion: &mut Criterion) {
    let nodes = nodes();
    let starts = starts();
    assert_eq!(
        route(&RUST, &nodes),
        route(&C, &nodes),
        "benchmark fixture differs: route"
    );
    assert_eq!(
        entry(&RUST, &starts),
        entry(&C, &starts),
        "benchmark fixture differs: entry"
    );

    let mut group = criterion.benchmark_group("source_size/route");
    group.throughput(Throughput::Elements(COUNT as u64));
    group.bench_function("rust", |b| b.iter(|| black_box(route(&RUST, &nodes))));
    group.bench_function("c", |b| b.iter(|| black_box(route(&C, &nodes))));
    group.finish();

    let mut group = criterion.benchmark_group("source_size/entry");
    group.throughput(Throughput::Elements(COUNT as u64));
    group.bench_function("rust", |b| b.iter(|| black_box(entry(&RUST, &starts))));
    group.bench_function("c", |b| b.iter(|| black_box(entry(&C, &starts))));
    group.finish();
}
