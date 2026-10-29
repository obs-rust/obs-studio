//! obs-source: the source and scene duplicate plans
//! (`libobs/obs-source-duplicate.c.h`), which `obs_scene_duplicate()` runs
//! once per item and per earlier item, and `obs_source_duplicate()` once
//! per source it copies (every Duplicate, every Studio Mode scene copy,
//! every filter copy). Each measurement folds its results into a checksum
//! that both sides must match:
//! - `scene`: the plan of a 16-item scene of each duplicate type, with
//!   sources used up to four times and two groups: the flags, the group
//!   override, the item routes, the reuse steps of every repeated item
//!   against the items before it (every copy added, where the two sides
//!   agree), the names and the resize;
//! - `source`: the route of 64 sources (scenes, groups, unknown scene
//!   types, inputs with and without `DO_NOT_DUPLICATE`, filters), public
//!   and private, with the filter copy and the first filter numbers.
//!
//! Measured when it was added (x86_64 Linux, GCC, not pinned to a core, a
//! loaded machine; median): scene 354 ns Rust, 371 ns C per four plans
//! (0.96x); source 646 ns Rust, 662 ns C per 128 routes (0.97x). A few
//! nanoseconds an item either way, and once per item copied.

use std::ffi::{c_int, c_void};
use std::hint::black_box;

use criterion::{Criterion, Throughput};
use obs_c_oracle::source_duplicate as c;
use obs_source::ffi::source_duplicate as rs;

const SOURCES: usize = 64;

static TARGETS: [u8; 8] = [0; 8];

/// The source of each item: 0 to 7, repeated, 6 and 7 being groups.
const SCENE: [usize; 16] = [0, 1, 0, 2, 6, 1, 3, 0, 4, 7, 2, 5, 0, 3, 6, 1];

fn source(i: usize) -> *const c_void {
    std::ptr::from_ref(&TARGETS[i]).cast()
}

struct Side {
    unique: fn(c_int) -> bool,
    private: fn(c_int) -> bool,
    keeps_unique: fn(bool, bool, c_int) -> bool,
    item_route: fn(bool, bool) -> c_int,
    reuse: fn(*const c_void, *const c_void, bool) -> c_int,
    keeps_name: fn(bool) -> bool,
    resizes: fn(bool) -> bool,
    type_step: fn(c_int) -> c_int,
    scene_step: fn(bool, bool) -> c_int,
    group_step: fn(bool) -> c_int,
    scene_type: fn(bool) -> c_int,
    flags_step: fn(u32) -> c_int,
    created_step: fn(bool) -> c_int,
    copies_filters: fn(c_int) -> bool,
    filter_number: fn(c_int) -> c_int,
}

const RUST: Side = Side {
    unique: |t| rs::obs_scene_duplicate_unique(t),
    private: |t| rs::obs_scene_duplicate_private(t),
    keeps_unique: |u, g, t| rs::obs_scene_duplicate_keeps_unique(u, g, t),
    item_route: |g, u| rs::obs_scene_duplicate_item_route(g, u),
    reuse: |e, s, k| rs::obs_scene_duplicate_reuse(e, s, k),
    keeps_name: |p| rs::obs_scene_duplicate_keeps_name(p),
    resizes: |g| rs::obs_scene_duplicate_resizes(g),
    type_step: |t| rs::obs_source_duplicate_type_step(t),
    scene_step: |s, p| rs::obs_source_duplicate_scene_step(s, p),
    group_step: |g| rs::obs_source_duplicate_group_step(g),
    scene_type: |p| rs::obs_source_duplicate_scene_type(p),
    flags_step: |f| rs::obs_source_duplicate_flags_step(f),
    created_step: |h| rs::obs_source_duplicate_created_step(h),
    copies_filters: |t| rs::obs_source_duplicate_copies_filters(t),
    filter_number: |n| rs::obs_source_duplicate_filter_number(n),
};

// SAFETY (every closure): plain values in and out; the reuse step only
// compares its pointers.
const C: Side = Side {
    unique: |t| unsafe { c::oracle_obs_scene_duplicate_unique(t) },
    private: |t| unsafe { c::oracle_obs_scene_duplicate_private(t) },
    keeps_unique: |u, g, t| unsafe { c::oracle_obs_scene_duplicate_keeps_unique(u, g, t) },
    item_route: |g, u| unsafe { c::oracle_obs_scene_duplicate_item_route(g, u) },
    reuse: |e, s, k| unsafe { c::oracle_obs_scene_duplicate_reuse(e, s, k) },
    keeps_name: |p| unsafe { c::oracle_obs_scene_duplicate_keeps_name(p) },
    resizes: |g| unsafe { c::oracle_obs_scene_duplicate_resizes(g) },
    type_step: |t| unsafe { c::oracle_obs_source_duplicate_type_step(t) },
    scene_step: |s, p| unsafe { c::oracle_obs_source_duplicate_scene_step(s, p) },
    group_step: |g| unsafe { c::oracle_obs_source_duplicate_group_step(g) },
    scene_type: |p| unsafe { c::oracle_obs_source_duplicate_scene_type(p) },
    flags_step: |f| unsafe { c::oracle_obs_source_duplicate_flags_step(f) },
    created_step: |h| unsafe { c::oracle_obs_source_duplicate_created_step(h) },
    copies_filters: |t| unsafe { c::oracle_obs_source_duplicate_copies_filters(t) },
    filter_number: |n| unsafe { c::oracle_obs_source_duplicate_filter_number(n) },
};

fn fold(sum: u64, value: c_int) -> u64 {
    sum.wrapping_mul(31)
        .wrapping_add(u64::from(value.unsigned_abs()))
}

/// The plan of `SCENE` for each duplicate type, every copy added.
fn scene(side: &Side) -> u64 {
    let mut sum = 0u64;
    for dup_type in 0..4 {
        let dup_type = black_box(dup_type);
        let mut unique = (side.unique)(dup_type);
        let private = (side.private)(dup_type);
        unique = (side.keeps_unique)(unique, black_box(false), dup_type);
        for (idx, &item) in SCENE.iter().enumerate() {
            let is_group = item >= 6;
            let route = (side.item_route)(is_group, unique);
            sum = fold(sum, route);
            if route == rs::OBS_SCENE_DUPLICATE_ITEM_CHILD {
                for (i, &earlier) in SCENE.iter().enumerate().take(idx) {
                    let r = (side.reuse)(source(earlier), source(item), true);
                    if r != rs::OBS_SCENE_DUPLICATE_REUSE_NONE {
                        sum = fold(sum, c_int::try_from(i).unwrap());
                        break;
                    }
                }
            }
            sum = fold(sum, c_int::from((side.keeps_name)(private)));
        }
        sum = fold(sum, c_int::from((side.resizes)(black_box(false))));
    }
    sum
}

/// The route of each source, public and private.
fn sources(side: &Side) -> u64 {
    let mut sum = 0u64;
    for i in 0..SOURCES {
        let source_type = c_int::try_from(i % 4).unwrap();
        let has_scene = i % 8 < 4;
        let has_group = i % 16 < 8;
        let flags = if i % 3 == 0 { rs_flag() } else { 0 };
        for create_private in [false, true] {
            let create_private = black_box(create_private);
            let mut step = (side.type_step)(black_box(source_type));
            if step == rs::OBS_SOURCE_DUPLICATE_ASK_SCENE {
                step = (side.scene_step)(has_scene, create_private);
                if step == rs::OBS_SOURCE_DUPLICATE_ASK_GROUP {
                    step = (side.group_step)(has_group);
                }
                if step == rs::OBS_SOURCE_DUPLICATE_SCENE_COPY {
                    sum = fold(sum, (side.scene_type)(create_private));
                }
            } else {
                step = (side.flags_step)(black_box(flags));
                if step == rs::OBS_SOURCE_DUPLICATE_CREATE {
                    step = (side.created_step)(true);
                    if (side.copies_filters)(source_type) {
                        sum = fold(sum, (side.filter_number)(black_box(1)));
                    }
                }
            }
            sum = fold(sum, step);
        }
    }
    sum
}

const fn rs_flag() -> u32 {
    obs_source::source_duplicate::DO_NOT_DUPLICATE
}

pub(crate) fn benchmarks(criterion: &mut Criterion) {
    assert_eq!(scene(&RUST), scene(&C), "benchmark fixture differs: scene");
    assert_eq!(
        sources(&RUST),
        sources(&C),
        "benchmark fixture differs: source"
    );

    let mut group = criterion.benchmark_group("source_duplicate/scene");
    group.throughput(Throughput::Elements(4));
    group.bench_function("rust", |b| b.iter(|| black_box(scene(&RUST))));
    group.bench_function("c", |b| b.iter(|| black_box(scene(&C))));
    group.finish();

    let mut group = criterion.benchmark_group("source_duplicate/source");
    group.throughput(Throughput::Elements(2 * SOURCES as u64));
    group.bench_function("rust", |b| b.iter(|| black_box(sources(&RUST))));
    group.bench_function("c", |b| b.iter(|| black_box(sources(&C))));
    group.finish();
}
