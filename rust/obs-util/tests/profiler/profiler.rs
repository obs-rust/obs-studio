//! Tier 1 for `libobs/util/profiler.c`.
//!
//! Mirrors `test/cmocka/test_profiler.c`: names are pointer identity, and
//! measured `time_delta` values are not asserted, only structure and counts.
//! The CSV dump stays in `util/profiler-io.c` and is covered by that C test.
//!
//! Global profiler state is shared, and `profiler_free` clears it, so the
//! three C tests run in one function in the cmocka order.

use core::ffi::c_char;
use std::ffi::CStr;
use std::ptr;

use obs_c_oracle as _;
use obs_util::ffi::profiler::profiler_name_store_free;
use obs_util::ffi::profiler_snapshot::{
    profile_snapshot_free, profiler_snapshot_entry_expected_time_between_calls,
    profiler_snapshot_entry_max_time, profiler_snapshot_entry_max_time_between_calls,
    profiler_snapshot_entry_min_time, profiler_snapshot_entry_min_time_between_calls,
    profiler_snapshot_entry_name, profiler_snapshot_entry_overall_between_calls_count,
    profiler_snapshot_entry_overall_count, profiler_snapshot_entry_times,
    profiler_snapshot_entry_times_between_calls, profiler_snapshot_enumerate_children,
    profiler_snapshot_enumerate_roots, profiler_snapshot_num_children, profiler_snapshot_num_roots,
};
use obs_util::profiler::{
    NameStore, ProfileLog, ProfileName, SnapshotEntry, profile_end, profile_reenable_thread,
    profile_register_root, profile_snapshot_create, profile_start, profiler_free, profiler_start,
    profiler_stop, testing_reset,
};
use obs_util::profiler_snapshot::{FilterStep, enumerate, filter};

const ITERATIONS: u64 = 5;

static ROOT_NAME: &CStr = c"test_root";
static CHILD_NAME: &CStr = c"test_child";
static OTHER_NAME: &CStr = c"test_other";

pub(crate) use super::PROFILER_LOCK as TEST_LOCK;

fn name_of(entry: &SnapshotEntry) -> &str {
    assert!(!entry.name.is_null());
    unsafe { CStr::from_ptr(entry.name.as_ptr()) }
        .to_str()
        .unwrap()
}

fn stored(ptr: *const c_char) -> String {
    assert!(!ptr.is_null());
    unsafe { CStr::from_ptr(ptr) }.to_str().unwrap().to_owned()
}

fn test_profiler_name_store() {
    let store = NameStore::new();
    let a = store.store("alpha-1");
    let b = store.store("beta-22");
    let c = store.store("plain");
    assert!(!a.is_null());
    assert!(!b.is_null());
    assert!(!c.is_null());

    let a2 = store.store("alpha-1");
    assert_ne!(a, a2);
    assert_ne!(a, b);
    assert_eq!(stored(a), "alpha-1");
    assert_eq!(stored(b), "beta-22");
    assert_eq!(stored(c), "plain");
    assert_eq!(stored(a2), "alpha-1");
    drop(store);

    // Null is a no-op. The guard is the shim's now (the core no longer takes
    // the raw pointer), so this is the C boundary's contract.
    unsafe { profiler_name_store_free(ptr::null_mut()) };
}

fn test_profiler_disabled() {
    let root = ProfileName::from_static(ROOT_NAME);
    let child = ProfileName::from_static(CHILD_NAME);
    profile_register_root(root, 1_000_000);
    profile_start(root, 1);
    profile_start(child, 2);
    profile_end(child, &mut || 3, &mut |_| {});
    profile_end(root, &mut || 4, &mut |_| {});

    let snap = profile_snapshot_create();
    assert_eq!(snap.roots.len(), 0);

    profiler_stop();
    profile_reenable_thread();

    unsafe {
        assert_eq!(profiler_snapshot_num_roots(ptr::null_mut()), 0);
        assert_eq!(profiler_snapshot_num_children(ptr::null_mut()), 0);
        assert!(profiler_snapshot_entry_name(ptr::null_mut()).is_null());
        assert!(profiler_snapshot_entry_times(ptr::null_mut()).is_null());
        assert!(profiler_snapshot_entry_times_between_calls(ptr::null_mut()).is_null());
        assert_eq!(profiler_snapshot_entry_overall_count(ptr::null_mut()), 0);
        assert_eq!(profiler_snapshot_entry_min_time(ptr::null_mut()), 0);
        assert_eq!(profiler_snapshot_entry_max_time(ptr::null_mut()), 0);
        assert_eq!(
            profiler_snapshot_entry_expected_time_between_calls(ptr::null_mut()),
            0
        );
        assert_eq!(
            profiler_snapshot_entry_min_time_between_calls(ptr::null_mut()),
            0
        );
        assert_eq!(
            profiler_snapshot_entry_max_time_between_calls(ptr::null_mut()),
            0
        );
        assert_eq!(
            profiler_snapshot_entry_overall_between_calls_count(ptr::null_mut()),
            0
        );
        profiler_snapshot_enumerate_roots(ptr::null_mut(), None, ptr::null_mut());
        profiler_snapshot_enumerate_children(ptr::null_mut(), None, ptr::null_mut());
        profile_snapshot_free(ptr::null_mut());
    }
}

fn clock(n: &mut u64) -> u64 {
    *n += 1_000_000;
    *n
}

#[expect(
    clippy::cognitive_complexity,
    reason = "scenario table: one block per ported step"
)]
fn test_profiler_lifecycle() {
    let root = ProfileName::from_static(ROOT_NAME);
    let child = ProfileName::from_static(CHILD_NAME);
    let other = ProfileName::from_static(OTHER_NAME);
    let mut now = 0u64;

    profiler_start();
    profile_reenable_thread();
    profile_register_root(root, 16_000_000);
    profile_register_root(other, 0);

    for _ in 0..ITERATIONS {
        profile_start(root, clock(&mut now));
        profile_start(child, clock(&mut now));
        profile_end(child, &mut || clock(&mut now), &mut |_| {});
        profile_end(root, &mut || clock(&mut now), &mut |_| {});
    }
    profile_start(other, clock(&mut now));
    profile_end(other, &mut || clock(&mut now), &mut |_| {});

    let mut snap = profile_snapshot_create();
    assert_eq!(snap.roots.len(), 2);
    assert_eq!(snap.roots[0].name, root);
    assert_eq!(snap.roots[1].name, other);

    let mut first = 0;
    enumerate(&mut snap.roots, |_| {
        first += 1;
        false
    });
    assert_eq!(first, 1);

    let root_entry = &snap.roots[0];
    assert_eq!(root_entry.overall_count, ITERATIONS);
    assert!(root_entry.min_time <= root_entry.max_time);
    assert_eq!(root_entry.expected_time_between_calls, 16_000);
    assert_eq!(root_entry.overall_between_calls_count, ITERATIONS - 1);
    assert!(root_entry.min_time_between_calls <= root_entry.max_time_between_calls);
    assert!(!root_entry.times.is_empty());
    assert!(!root_entry.times_between_calls.is_empty());
    assert_eq!(root_entry.children.len(), 1);
    assert_eq!(root_entry.children[0].name, child);
    assert_eq!(root_entry.children[0].overall_count, ITERATIONS);
    assert!(root_entry.children[0].min_time <= root_entry.children[0].max_time);
    assert!(root_entry.children[0].children.is_empty());
    assert_eq!(root_entry.children[0].expected_time_between_calls, 0);
    assert_eq!(root_entry.children[0].overall_between_calls_count, 0);

    let other_entry = &snap.roots[1];
    assert_eq!(other_entry.overall_count, 1);
    assert_eq!(other_entry.expected_time_between_calls, 0);
    assert_eq!(other_entry.overall_between_calls_count, 0);
    assert!(other_entry.children.is_empty());

    let total: u64 = root_entry.times.iter().map(|t| t.count).sum();
    assert_eq!(total, ITERATIONS);

    let mut visited = 0usize;
    filter(&mut snap.roots, |entry| {
        visited += 1;
        FilterStep {
            keep_going: true,
            remove: name_of(entry) == OTHER_NAME.to_str().unwrap(),
        }
    });
    assert_eq!(visited, 2);
    assert_eq!(snap.roots.len(), 1);
    assert_eq!(snap.roots[0].name, root);

    profiler_stop();
    profile_start(root, clock(&mut now));
    profile_start(child, clock(&mut now));
    profile_end(child, &mut || clock(&mut now), &mut |_| {});
    profile_end(root, &mut || clock(&mut now), &mut |_| {});

    let after = profile_snapshot_create();
    assert_eq!(after.roots.len(), 2);
    assert_eq!(after.roots[0].overall_count, ITERATIONS);
    assert_eq!(after.roots[1].overall_count, 1);

    profiler_free();
}

/// `test_profiler_name_store`, `test_profiler_disabled`, `test_profiler_lifecycle`.
#[test]
fn test_profiler_cmocka() {
    let _guard = TEST_LOCK.lock().unwrap();
    // Later tests in this binary share the process-lifetime profiler.
    profiler_free();
    test_profiler_name_store();
    test_profiler_disabled();
    test_profiler_lifecycle();
}

/// ProfileName stores a pointer that the profiler reads back as a C string,
/// so a name must carry its own NUL. from_static takes a &CStr; a &str (no
/// NUL) read on past its end, which made test_profiler_cmocka fail on macOS.
#[test]
fn static_name_reads_back_exactly() {
    let name = ProfileName::from_static(c"abc");
    let read = unsafe { CStr::from_ptr(name.as_ptr()) };
    assert_eq!(read.to_bytes(), b"abc");
}

/// The C test cannot cover the two `LOG_ERROR` lines (`blog`'s output is not
/// capturable from cmocka), so nothing pinned them before. The core reports
/// them through [`profile_end`]'s callback instead, which this can check.
#[test]
fn profile_end_reports_the_log_lines() {
    let _guard = TEST_LOCK.lock().unwrap();
    testing_reset();

    let root = ProfileName::from_static(ROOT_NAME);
    let other = ProfileName::from_static(OTHER_NAME);
    let mut now = 0u64;

    profiler_start();
    profile_reenable_thread();
    profile_register_root(root, 0);
    profile_start(root, clock(&mut now));

    // An end with another name reports the open frame and the name asked for,
    // and leaves the frame open because nothing matched to close.
    let mut reported = Vec::new();
    profile_end(other, &mut || clock(&mut now), &mut |entry| {
        reported.push(entry)
    });
    assert_eq!(
        reported,
        vec![ProfileLog::Mismatch {
            started: root,
            ended: other,
        }]
    );

    // Ending the right name closes it, with nothing to report.
    let mut reported = Vec::new();
    profile_end(root, &mut || clock(&mut now), &mut |entry| {
        reported.push(entry)
    });
    assert!(reported.is_empty());

    // With no active profile, the other line is reported.
    let mut reported = Vec::new();
    profile_end(root, &mut || clock(&mut now), &mut |entry| {
        reported.push(entry)
    });
    assert_eq!(reported, vec![ProfileLog::NoActive]);

    testing_reset();
}
