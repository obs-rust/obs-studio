//! obs-source: the transition start checks and child handover
//! (`libobs/obs-source-transition-start.c.h`). Every `obs_transition_start()`
//! (each Studio Mode switch, T-bar grab and scene change) asks these
//! decisions. Each side runs 1024 starts over every combination of the
//! flags the C reads, with manual and auto modes, durations from 0 to
//! `UINT32_MAX` ms and sizes of 0 and full HD, each asking, in the C's
//! order, the first refusal, the walk, the second admission, the duration,
//! the clock rule, the handover, the callback gate, the size rule, the null
//! destination, the slot activation and the active index, then a manual
//! time and the set's two active child rules; nothing is allocated, and the
//! results fold into a checksum both must match.

use super::pair;

use criterion::Criterion;
use obs_c_oracle::source_transition_start as oracle;
use obs_source::ffi::source_transition_start as shim;
use std::hint::black_box;
use std::os::raw::c_int;

const N: u32 = 1024;

/// Start `i`: its flags, mode, duration, size and manual time.
fn start(i: u32) -> (u32, c_int, u32, u32, f32) {
    let mode = c_int::from(i & 32 != 0);
    let ms = [0, 300, 1000, u32::MAX][usize::try_from(i % 4).unwrap_or(0)];
    let cx = if i.is_multiple_of(7) { 0 } else { 1920 };
    let t = [0.0f32, -0.0, 0.5, 1.0, f32::NAN][usize::try_from(i % 5).unwrap_or(0)];
    (i, mode, ms, cx, t)
}

fn bit(flags: u32, n: u32) -> bool {
    flags & (1 << n) != 0
}

fn fold(acc: u64, values: [u64; 16]) -> u64 {
    values
        .iter()
        .fold(acc, |a, &v| a.wrapping_mul(31).wrapping_add(v))
}

fn rust() -> u64 {
    (0..N).fold(0, |acc, i| {
        let (f, mode, ms, cx, t) = black_box(start(i));
        let b = |n| bit(f, n);
        fold(
            acc,
            [
                u64::from(shim::obs_source_transition_start_refused(b(0), b(1))),
                u64::from(shim::obs_source_transition_start_walks(b(2), b(3))),
                u64::from(shim::obs_source_transition_start_admit(
                    b(0),
                    b(4),
                    b(5),
                    mode,
                    b(1),
                )),
                shim::obs_source_transition_start_duration_ns(ms),
                u64::from(shim::obs_source_transition_start_restart(b(1), b(4), b(0))),
                u64::from(shim::obs_source_transition_start_adds(b(6), b(2))),
                u64::from(shim::obs_source_transition_start_removes(b(6), b(7), b(8))),
                u64::from(shim::obs_source_transition_start_calls_back(true, b(8))),
                u64::from(
                    shim::obs_source_transition_start_unsized(cx)
                        || shim::obs_source_transition_start_unsized(cx >> 1),
                ),
                u64::from(shim::obs_source_transition_start_null_dest(
                    b(2),
                    b(4),
                    b(0),
                )),
                u64::from(shim::obs_source_transition_start_activates(b(9))),
                u64::try_from(shim::obs_source_transition_start_active_index(b(1))).unwrap_or(0),
                u64::from(shim::obs_source_transition_start_manual_reset(mode, t)),
                u64::from(shim::obs_source_transition_start_child_active(b(7), b(6))),
                u64::from(shim::obs_source_transition_start_child_active(b(2), b(9))),
                u64::from(i),
            ],
        )
    })
}

fn c() -> u64 {
    (0..N).fold(0, |acc, i| {
        let (f, mode, ms, cx, t) = black_box(start(i));
        let b = |n| bit(f, n);
        // SAFETY: plain values in and out; the C touches no memory.
        let values = unsafe {
            [
                u64::from(oracle::oracle_obs_source_transition_start_refused(
                    b(0),
                    b(1),
                )),
                u64::from(oracle::oracle_obs_source_transition_start_walks(b(2), b(3))),
                u64::from(oracle::oracle_obs_source_transition_start_admit(
                    b(0),
                    b(4),
                    b(5),
                    mode,
                    b(1),
                )),
                oracle::oracle_obs_source_transition_start_duration_ns(ms),
                u64::from(oracle::oracle_obs_source_transition_start_restart(
                    b(1),
                    b(4),
                    b(0),
                )),
                u64::from(oracle::oracle_obs_source_transition_start_adds(b(6), b(2))),
                u64::from(oracle::oracle_obs_source_transition_start_removes(
                    b(6),
                    b(7),
                    b(8),
                )),
                u64::from(oracle::oracle_obs_source_transition_start_calls_back(
                    true,
                    b(8),
                )),
                u64::from(
                    oracle::oracle_obs_source_transition_start_unsized(cx) != 0
                        || oracle::oracle_obs_source_transition_start_unsized(cx >> 1) != 0,
                ),
                u64::from(oracle::oracle_obs_source_transition_start_null_dest(
                    b(2),
                    b(4),
                    b(0),
                )),
                u64::from(oracle::oracle_obs_source_transition_start_activates(b(9))),
                u64::try_from(oracle::oracle_obs_source_transition_start_active_index(b(
                    1,
                )))
                .unwrap_or(0),
                u64::from(oracle::oracle_obs_source_transition_start_manual_reset(
                    mode, t,
                )),
                u64::from(oracle::oracle_obs_source_transition_start_child_active(
                    b(7),
                    b(6),
                )),
                u64::from(oracle::oracle_obs_source_transition_start_child_active(
                    b(2),
                    b(9),
                )),
                u64::from(i),
            ]
        };
        fold(acc, values)
    })
}

pub fn benchmarks(criterion: &mut Criterion) {
    pair(
        criterion,
        "source_transition_start/obs_transition_start/1024",
        rust,
        c,
    );
}
