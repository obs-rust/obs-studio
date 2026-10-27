//! obs-source: the transition load, scale type and swap checks
//! (`libobs/obs-source-transition-load.c.h`). Every transition getter and
//! setter asks the type check, each scene collection load asks the load
//! steps and each Studio Mode swap asks the swap gate and the store rule.
//! Each side runs 1024 loads over every combination of the flags the C
//! reads, with source types in and out of the enum and saved scale types at
//! 0, 1, 2, 3, -1 and past 32 bits, each asking, in the C's order, the type
//! check, the lookup, the source outcome and the scale type, then a swap's
//! gate and the store rule of both slots; nothing is allocated, and the
//! results fold into a checksum both must match.

use super::pair;

use criterion::Criterion;
use obs_c_oracle::source_transition_load as oracle;
use obs_source::ffi::source_transition_load as shim;
use std::hint::black_box;
use std::os::raw::c_int;

const N: u32 = 1024;

/// Load `i`: its flags, source type and saved scale type.
fn load(i: u32) -> (u32, c_int, i64) {
    let ty = [2, 0, 1, 3, -1][usize::try_from(i % 5).unwrap_or(0)];
    let scale = [0, 1, 2, 3, -1, (1_i64 << 32) + 1][usize::try_from(i % 6).unwrap_or(0)];
    (i, ty, scale)
}

fn bit(flags: u32, n: u32) -> bool {
    flags & (1 << n) != 0
}

fn fold(acc: u64, values: [u64; 9]) -> u64 {
    values
        .iter()
        .fold(acc, |a, &v| a.wrapping_mul(31).wrapping_add(v))
}

/// A scale type as the checksum's `u64` (it is 0, 1 or 2).
fn scale(v: c_int) -> u64 {
    u64::try_from(v).unwrap_or(u64::MAX)
}

fn rust() -> u64 {
    (0..N).fold(0, |acc, i| {
        let (f, ty, sc) = black_box(load(i));
        let b = |n| bit(f, n);
        fold(
            acc,
            [
                u64::from(shim::obs_source_transition_load_is_transition(ty)),
                u64::from(shim::obs_source_transition_load_looks_up(b(0))),
                u64::from(shim::obs_source_transition_load_source(b(1), b(2))),
                scale(shim::obs_source_transition_load_scale_type(sc)),
                u64::from(shim::obs_source_transition_load_swaps(b(3))),
                u64::from(shim::obs_source_transition_load_copy_stores(b(4), b(5))),
                u64::from(shim::obs_source_transition_load_copy_stores(b(6), b(7))),
                u64::from(shim::obs_source_transition_load_swaps(b(3))),
                u64::from(i),
            ],
        )
    })
}

fn c() -> u64 {
    (0..N).fold(0, |acc, i| {
        let (f, ty, sc) = black_box(load(i));
        let b = |n| bit(f, n);
        // SAFETY: plain values in and out; the C touches no memory.
        let values = unsafe {
            [
                u64::from(oracle::oracle_obs_source_transition_load_is_transition(ty)),
                u64::from(oracle::oracle_obs_source_transition_load_looks_up(b(0))),
                u64::from(oracle::oracle_obs_source_transition_load_source(b(1), b(2))),
                scale(oracle::oracle_obs_source_transition_load_scale_type(sc)),
                u64::from(oracle::oracle_obs_source_transition_load_swaps(b(3))),
                u64::from(oracle::oracle_obs_source_transition_load_copy_stores(
                    b(4),
                    b(5),
                )),
                u64::from(oracle::oracle_obs_source_transition_load_copy_stores(
                    b(6),
                    b(7),
                )),
                u64::from(oracle::oracle_obs_source_transition_load_swaps(b(3))),
                u64::from(i),
            ]
        };
        fold(acc, values)
    })
}

pub fn benchmarks(criterion: &mut Criterion) {
    pair(
        criterion,
        "source_transition_load/obs_transition_load/1024",
        rust,
        c,
    );
}
