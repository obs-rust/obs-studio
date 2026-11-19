//! Tier 3: the original C acquisition and the Rust export over real-header
//! sources and controls, with the real control adapter and the original
//! addref body. Counts stay far from overflow (C UB). No intentional differences.
use core::ffi::{c_long, c_void};
use obs_source::ffi::source_get_weak::obs_source_get_weak_source;
use proptest::prelude::*;

type Entry = unsafe extern "C" fn(*mut c_void) -> *mut c_void;

unsafe extern "C" {
    fn oracle_source_get_weak_check(
        entry: Entry,
        flags: u32,
        noise: u32,
        refs: c_long,
        weak_refs: c_long,
        count: u32,
        returned_control: *mut bool,
        addrefs: *mut u32,
        final_weak_refs: *mut c_long,
    ) -> bool;
}

/// Both sides agree; returns the original's (returned control, addrefs, weak count).
fn check(
    flags: u32,
    noise: u32,
    refs: c_long,
    weak_refs: c_long,
    count: u32,
) -> (bool, u32, c_long) {
    let (mut returned, mut addrefs, mut weak) = (false, 0, 0);
    // SAFETY: the oracle owns heap fixtures for both sides and frees them; the
    // out-pointers are live locals.
    let equal = unsafe {
        oracle_source_get_weak_check(
            obs_source_get_weak_source,
            flags,
            noise,
            refs,
            weak_refs,
            count,
            &raw mut returned,
            &raw mut addrefs,
            &raw mut weak,
        )
    };
    assert!(
        equal,
        "flags={flags}, noise={noise:#x}, refs={refs}, weak_refs={weak_refs}, count={count}"
    );
    (returned, addrefs, weak)
}

#[test]
fn acquisition_returns_counts_and_bytes_match_original_c() {
    for flags in 0..4 {
        let source = flags & 1 != 0;
        let control = flags & 2 != 0;
        for noise in [0, 1, 0x5a5a_5a5a, u32::MAX] {
            for (refs, weak_refs) in [(0, 0), (7, 3), (-1, 1), (1, 100_000)] {
                for count in [0_u8, 1, 3] {
                    // The pinned outcome catches a mutation on both sides.
                    let calls = if source { u32::from(count) } else { 0 };
                    let added = if source && control { count } else { 0 };
                    assert_eq!(
                        check(flags, noise, refs, weak_refs, u32::from(count)),
                        (true, calls, weak_refs + c_long::from(added)),
                        "flags={flags}, count={count}"
                    );
                }
            }
        }
    }
    let mut runner = proptest::test_runner::TestRunner::new(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..ProptestConfig::default()
    });
    runner
        .run(
            &(
                0u32..4,
                any::<u32>(),
                (-1000_i32..1000).prop_map(c_long::from),
                (0_i32..1000).prop_map(c_long::from),
                0u32..16,
            ),
            |(flags, noise, refs, weak_refs, count)| {
                check(flags, noise, refs, weak_refs, count);
                Ok(())
            },
        )
        .unwrap();
}
