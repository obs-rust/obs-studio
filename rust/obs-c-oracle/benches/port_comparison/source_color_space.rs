//! obs-source: the colour space query routing
//! (`libobs/obs-source-color-space.c.h`), which `obs_source_get_color_space()`
//! runs for every source and filter it asks, from the render of each source
//! and filter in a scene. Each measurement folds its results into a
//! checksum that both sides must match:
//! - `route`: 1024 nodes, every combination of a filter or not, the VIDEO
//!   and ASYNC flags, data, enabled, a filter parent and a filter target,
//!   with and without `OBS_SOURCE_AUDIO`, eight times over;
//! - `async_pick`: 1024 picks of the four frame spaces from lists of 0 to 4
//!   preferred spaces (the HDR Tone Mapping filter's among them), matched
//!   first, last, in the middle or not at all.
//!
//! Measured when it was added (x86_64 Linux, GCC, not pinned to a core;
//! median per 1024 calls): route 2.05 us Rust, 2.26 us C (0.91x);
//! async_pick 2.21 us Rust, 2.69 us C (0.82x; 2.54 us Rust in an earlier
//! run). About 2 ns a call either way, below the noise of the run.

use std::hint::black_box;

use criterion::{Criterion, Throughput};
use obs_c_oracle::source_color_space::{self as c, OracleColorSpaceNode};
use obs_source::ffi::source_color_space::{self as rs, obs_source_color_space_node};

const COUNT: usize = 1024;

#[derive(Clone, Copy)]
struct Fields {
    source_type: i32,
    output_flags: u32,
    has_data: bool,
    enabled: bool,
    has_filter_parent: bool,
    has_filter_target: bool,
}

fn nodes() -> Vec<Fields> {
    (0..COUNT)
        .map(|i| {
            let bits = u32::try_from(i % 256).unwrap();
            let bit = |n: u32| bits & (1 << n) != 0;
            let mut flags = 0;
            if bit(5) {
                flags |= 1; // OBS_SOURCE_VIDEO
            }
            if bit(4) {
                flags |= 1 << 2; // OBS_SOURCE_ASYNC
            }
            if bit(7) {
                flags |= 1 << 1; // OBS_SOURCE_AUDIO
            }
            Fields {
                source_type: i32::from(bit(6)), // OBS_SOURCE_TYPE_FILTER or INPUT
                output_flags: flags,
                has_data: bit(3),
                enabled: bit(2),
                has_filter_parent: bit(1),
                has_filter_target: bit(0),
            }
        })
        .collect()
}

/// The preferred lists, from none to four spaces.
const LISTS: [&[i32]; 8] = [
    &[],
    &[0],
    &[0, 1, 2],
    &[2, 0, 1],
    &[0, 2, 3],
    &[1, 0],
    &[3, 3, 0, 1],
    &[0, 1],
];

fn picks() -> Vec<(i32, &'static [i32])> {
    (0..COUNT)
        .map(|i| {
            let space = i32::try_from(i % 4).unwrap();
            (space, LISTS[(i / 4) % LISTS.len()])
        })
        .collect()
}

struct Side {
    route: fn(Fields) -> u32,
    pick: fn(i32, &[i32]) -> i32,
}

const RUST: Side = Side {
    route: |f| {
        rs::obs_source_color_space_route(obs_source_color_space_node {
            source_type: f.source_type,
            output_flags: f.output_flags,
            has_data: f.has_data,
            enabled: f.enabled,
            has_filter_parent: f.has_filter_parent,
            has_filter_target: f.has_filter_target,
        })
    },
    // SAFETY: the list holds its length of ints.
    pick: |space, list| unsafe {
        rs::obs_source_color_space_pick(space, list.len(), list.as_ptr())
    },
};

const C: Side = Side {
    // SAFETY: plain values.
    route: |f| unsafe {
        c::oracle_obs_source_color_space_route(OracleColorSpaceNode {
            source_type: f.source_type,
            output_flags: f.output_flags,
            has_data: f.has_data,
            enabled: f.enabled,
            has_filter_parent: f.has_filter_parent,
            has_filter_target: f.has_filter_target,
        })
    },
    // SAFETY: the list holds its length of ints.
    pick: |space, list| unsafe {
        c::oracle_obs_source_color_space_pick(space, list.len(), list.as_ptr())
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

fn pick(side: &Side, picks: &[(i32, &'static [i32])]) -> u64 {
    let mut sum = 0u64;
    for &(space, list) in picks {
        let picked = (side.pick)(black_box(space), black_box(list));
        sum = sum
            .wrapping_mul(31)
            .wrapping_add(u64::from(picked.cast_unsigned()));
    }
    sum
}

pub(crate) fn benchmarks(criterion: &mut Criterion) {
    let nodes = nodes();
    let picks = picks();
    assert_eq!(
        route(&RUST, &nodes),
        route(&C, &nodes),
        "benchmark fixture differs: route"
    );
    assert_eq!(
        pick(&RUST, &picks),
        pick(&C, &picks),
        "benchmark fixture differs: async_pick"
    );

    let mut group = criterion.benchmark_group("source_color_space/route");
    group.throughput(Throughput::Elements(COUNT as u64));
    group.bench_function("rust", |b| b.iter(|| black_box(route(&RUST, &nodes))));
    group.bench_function("c", |b| b.iter(|| black_box(route(&C, &nodes))));
    group.finish();

    let mut group = criterion.benchmark_group("source_color_space/async_pick");
    group.throughput(Throughput::Elements(COUNT as u64));
    group.bench_function("rust", |b| b.iter(|| black_box(pick(&RUST, &picks))));
    group.bench_function("c", |b| b.iter(|| black_box(pick(&C, &picks))));
    group.finish();
}
