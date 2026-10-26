//! obs-source: the per-frame decisions of `obs_source_video_tick()` and
//! `async_tick()` (`libobs/obs-source-video-tick.c.h`). Every source runs
//! them once per rendered frame, so each tick asks up to nine of them. Each
//! side runs 1024 ticks over fixed sources (every type, the async and media
//! flags on and off, counts moving between 0 and not 0, deinterlacing on
//! and off), each asking the decisions in `obs_source_video_tick()`'s
//! order, nothing is allocated, and the results fold into a checksum both
//! must match.

use super::pair;

use criterion::Criterion;
use obs_c_oracle::source_video_tick as oracle;
use obs_source::ffi::source_video_tick as shim;
use std::hint::black_box;
use std::os::raw::{c_int, c_long};

const N: u32 = 1024;

/// Tick `i`: type, flags, defer count, show refs, activate refs, the two
/// stored flags, data, callback and deinterlacing.
fn tick(
    i: u32,
) -> (
    c_int,
    u32,
    c_long,
    c_long,
    c_long,
    bool,
    bool,
    bool,
    bool,
    bool,
) {
    let ty = c_int::try_from(i % 5).unwrap_or(0);
    let flags = ((i & 1) << 2) | ((i & 2) << 12) | (i & 0x10);
    let count = |n: u32| c_long::from(u8::try_from(n % 3).unwrap_or(0));
    (
        ty,
        flags,
        count(i / 3),
        count(i / 5),
        count(i / 7),
        i & 4 != 0,
        i & 8 != 0,
        !i.is_multiple_of(11),
        !i.is_multiple_of(13),
        i & 32 != 0,
    )
}

fn fold(acc: u64, bits: [u32; 9]) -> u64 {
    bits.iter()
        .fold(acc, |a, &b| a.wrapping_mul(5).wrapping_add(u64::from(b)))
}

fn rust() -> u64 {
    (0..N).fold(0, |acc, i| {
        let (ty, flags, defer, show, act, showing, active, data, cb, di) = black_box(tick(i));
        fold(
            acc,
            [
                u32::from(shim::obs_source_video_tick_transition(ty)),
                u32::from(shim::obs_source_video_tick_async(flags)),
                shim::obs_source_video_tick_async_route(di),
                u32::from(shim::obs_source_video_tick_media(flags)),
                u32::from(shim::obs_source_video_tick_deferred_update(defer)),
                shim::obs_source_video_tick_edge(show, showing),
                shim::obs_source_video_tick_edge(act, active),
                u32::from(shim::obs_source_video_tick_calls_tick(data, cb)),
                shim::obs_source_video_tick_async_route(di),
            ],
        )
    })
}

fn c() -> u64 {
    (0..N).fold(0, |acc, i| {
        let (ty, flags, defer, show, act, showing, active, data, cb, di) = black_box(tick(i));
        // SAFETY: plain values in and out; the C touches no memory.
        let bits = unsafe {
            [
                u32::from(oracle::oracle_obs_source_video_tick_transition(ty)),
                u32::from(oracle::oracle_obs_source_video_tick_async(flags)),
                oracle::oracle_obs_source_video_tick_async_route(di),
                u32::from(oracle::oracle_obs_source_video_tick_media(flags)),
                u32::from(oracle::oracle_obs_source_video_tick_deferred_update(defer)),
                oracle::oracle_obs_source_video_tick_edge(show, showing),
                oracle::oracle_obs_source_video_tick_edge(act, active),
                u32::from(oracle::oracle_obs_source_video_tick_calls_tick(data, cb)),
                oracle::oracle_obs_source_video_tick_async_route(di),
            ]
        };
        fold(acc, bits)
    })
}

pub fn benchmarks(criterion: &mut Criterion) {
    pair(
        criterion,
        "source_video_tick/obs_source_video_tick/1024",
        rust,
        c,
    );
}
