//! Tier 3 for `libobs/util/profiler.c`.
//!
//! The oracle calls `oracle_profiler_gettime_ns`. The Rust shims read the
//! same timestamps from `testing_set_clock`, and use `os_gettime_ns` only
//! when that clock is unset. Print and CSV stay in `util/profiler-io.c` and
//! are not differential-tested here; `test_profiler.c` covers the CSV header.
//!
//! `oracle_profiler_reset` runs the real `profiler_free` and re-inits the
//! mutex that free destroys. Each case starts from that clean state.

use core::ffi::c_char;
use std::slice;

use obs_c_oracle::profiler::{
    oracle_profile_end, oracle_profile_reenable_thread, oracle_profile_register_root,
    oracle_profile_snapshot_create, oracle_profile_start, oracle_profiler_reset,
    oracle_profiler_set_clock, oracle_profiler_start, oracle_profiler_stop,
};
use obs_c_oracle::profiler_snapshot::{
    OracleSnapshot, OracleSnapshotEntry, OracleTimeEntry, oracle_profile_snapshot_free,
};
use obs_util::ffi::profiler::{
    profile_end, profile_reenable_thread, profile_register_root, profile_snapshot_create,
    profile_start, profiler_start, profiler_stop,
};
use obs_util::ffi::profiler_snapshot::{profile_snapshot_free, profiler_snapshot_num_roots};
use obs_util::profiler::{Snapshot, SnapshotEntry, testing_reset, testing_set_clock};
use proptest::prelude::*;

#[derive(Clone, Debug)]
enum Op {
    Reg(u8, u64),
    Start(u8),
    End(u8),
    Stop,
    Go,
    Reenable,
}

/// Oracle statics, the scripted clock, and the Rust profiler are process-global.
pub(crate) use super::PROFILER_LOCK as PARITY_LOCK;

fn name(id: u8) -> *const c_char {
    match id % 3 {
        0 => c"root".as_ptr(),
        1 => c"child".as_ptr(),
        _ => c"other".as_ptr(),
    }
}

fn clock(ops: usize) -> Vec<u64> {
    (0..ops * 8 + 8)
        .map(|i| (i as u64 + 1) * 1_000_000)
        .collect()
}

fn apply_oracle(ops: &[Op]) {
    unsafe {
        oracle_profiler_start();
        for op in ops {
            match *op {
                Op::Reg(id, expected) => oracle_profile_register_root(name(id), expected),
                Op::Start(id) => oracle_profile_start(name(id)),
                Op::End(id) => oracle_profile_end(name(id)),
                Op::Stop => oracle_profiler_stop(),
                Op::Go => oracle_profiler_start(),
                Op::Reenable => oracle_profile_reenable_thread(),
            }
        }
    }
}

fn apply_rust(ops: &[Op]) {
    unsafe {
        profiler_start();
        for op in ops {
            match *op {
                Op::Reg(id, expected) => profile_register_root(name(id), expected),
                Op::Start(id) => profile_start(name(id)),
                Op::End(id) => profile_end(name(id)),
                Op::Stop => profiler_stop(),
                Op::Go => profiler_start(),
                Op::Reenable => profile_reenable_thread(),
            }
        }
    }
}

fn read_times(entries: &[OracleTimeEntry]) -> Vec<(u64, u64)> {
    entries.iter().map(|t| (t.time_delta, t.count)).collect()
}

fn times_of(array: *mut OracleTimeEntry, num: usize) -> Vec<(u64, u64)> {
    if array.is_null() || num == 0 {
        return Vec::new();
    }
    read_times(unsafe { slice::from_raw_parts(array, num) })
}

fn entry_from(entry: &OracleSnapshotEntry) -> Compared {
    let children = if entry.children.array.is_null() || entry.children.num == 0 {
        Vec::new()
    } else {
        unsafe { slice::from_raw_parts(entry.children.array, entry.children.num) }
            .iter()
            .map(entry_from)
            .collect()
    };
    Compared {
        name: entry.name as usize,
        times: times_of(entry.times.array, entry.times.num),
        min_time: entry.min_time,
        max_time: entry.max_time,
        overall_count: entry.overall_count,
        between: times_of(
            entry.times_between_calls.array,
            entry.times_between_calls.num,
        ),
        expected: entry.expected_time_between_calls,
        min_between: entry.min_time_between_calls,
        max_between: entry.max_time_between_calls,
        overall_between: entry.overall_between_calls_count,
        children,
    }
}

fn rust_entry(entry: &SnapshotEntry) -> Compared {
    Compared {
        name: entry.name.as_ptr() as usize,
        times: entry
            .times
            .iter()
            .map(|t| (t.time_delta, t.count))
            .collect(),
        min_time: entry.min_time,
        max_time: entry.max_time,
        overall_count: entry.overall_count,
        between: entry
            .times_between_calls
            .iter()
            .map(|t| (t.time_delta, t.count))
            .collect(),
        expected: entry.expected_time_between_calls,
        min_between: entry.min_time_between_calls,
        max_between: entry.max_time_between_calls,
        overall_between: entry.overall_between_calls_count,
        children: entry.children.iter().map(rust_entry).collect(),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Compared {
    name: usize,
    times: Vec<(u64, u64)>,
    min_time: u64,
    max_time: u64,
    overall_count: u64,
    between: Vec<(u64, u64)>,
    expected: u64,
    min_between: u64,
    max_between: u64,
    overall_between: u64,
    children: Vec<Compared>,
}

fn oracle_view(snap: &OracleSnapshot) -> Vec<Compared> {
    if snap.roots.array.is_null() || snap.roots.num == 0 {
        return Vec::new();
    }
    unsafe { slice::from_raw_parts(snap.roots.array, snap.roots.num) }
        .iter()
        .map(entry_from)
        .collect()
}

fn rust_view(snap: &Snapshot) -> Vec<Compared> {
    snap.roots.iter().map(rust_entry).collect()
}

fn run_both(ops: &[Op]) -> (Vec<Compared>, Vec<Compared>, usize) {
    let _guard = PARITY_LOCK.lock().unwrap();
    let times = clock(ops.len());
    unsafe {
        oracle_profiler_reset();
        testing_reset();
        oracle_profiler_set_clock(times.as_ptr(), times.len());
        apply_oracle(ops);
        let oracle_ptr = oracle_profile_snapshot_create();
        let oracle = oracle_view(&*oracle_ptr);
        oracle_profile_snapshot_free(oracle_ptr);

        oracle_profiler_set_clock(times.as_ptr(), times.len());
        testing_set_clock(Some(&times));
        apply_rust(ops);
        let rust_snap = obs_util::profiler::profile_snapshot_create();
        let c_snap = profile_snapshot_create();
        let c_roots = profiler_snapshot_num_roots(c_snap);
        profile_snapshot_free(c_snap);
        testing_set_clock(None);
        (oracle, rust_view(&rust_snap), c_roots)
    }
}

/// obs-rust/obs-studio#441. On windows-gnu, libgcc emutls calls
/// `pthread_getspecific` with a 32-bit `pthread_key_t`. The oracle's
/// w32-pthreads key is a pointer. Sharing that symbol truncates it and the
/// process exits `0xC0000005`. MSVC has no libgcc, so this is a normal start.
#[test]
fn oracle_profiler_survives_process_tls() {
    let _guard = PARITY_LOCK.lock().unwrap();
    unsafe {
        oracle_profiler_reset();
        oracle_profiler_start();
        oracle_profiler_stop();
    }
}

fn assert_match(ops: &[Op]) {
    let (oracle, rust, c_roots) = run_both(ops);
    assert_eq!(oracle, rust, "{ops:?}");
    assert_eq!(c_roots, rust.len());
}

#[test]
fn lifecycle_matches_oracle() {
    let mut ops = vec![Op::Go, Op::Reenable, Op::Reg(0, 16_000_000), Op::Reg(2, 0)];
    for _ in 0..5 {
        ops.push(Op::Start(0));
        ops.push(Op::Start(1));
        ops.push(Op::End(1));
        ops.push(Op::End(0));
    }
    ops.push(Op::Start(2));
    ops.push(Op::End(2));
    assert_match(&ops);

    let (_oracle, rust, _) = run_both(&ops);
    assert_eq!(rust.len(), 2);
    assert_eq!(rust[0].overall_count, 5);
    assert_eq!(rust[0].expected, 16_000);
    assert_eq!(rust[0].overall_between, 4);
    assert_eq!(rust[0].children.len(), 1);
    assert_eq!(rust[0].children[0].overall_count, 5);
    assert_eq!(rust[1].overall_count, 1);
    assert_eq!(rust[1].expected, 0);
}

#[test]
fn registered_root_with_no_calls_matches_oracle() {
    assert_match(&[Op::Go, Op::Reg(0, 1_000)]);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(48)))]

    #[test]
    fn random_calls_match_oracle(
        ops in proptest::collection::vec(
            prop_oneof![
                (0u8..3u8, 0u64..20_000_000u64).prop_map(|(id, expected)| Op::Reg(id, expected)),
                (0u8..3u8).prop_map(Op::Start),
                (0u8..3u8).prop_map(Op::End),
                Just(Op::Stop),
                Just(Op::Go),
                Just(Op::Reenable),
            ],
            0..16,
        )
    ) {
        assert_match(&ops);
    }
}
