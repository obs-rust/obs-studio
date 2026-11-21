//! Tier 3: the original extracted `obs_encoder_group_num_started()` versus
//! the shim on two recording fixture groups
//! (oracle/encoder_group_num_started.c), through the real-named adapters.
//! Compared: the value returned, every lock and unlock of the group mutex
//! (the queried group's own, never nested, none left held), and both groups
//! byte for byte afterwards, over any counts, with the count rewritten by
//! the lock, the unlock, both or neither, so a read outside the pair or a
//! second read shows.
//!
//! The real-header layout check pins the two fields the adapters touch, and
//! the adapters are checked against the real struct.
//!
//! No intentional differences. Mutants show the probe catches a read before
//! the lock, a read after the unlock, a missing lock, a missing unlock, and
//! a count clamped to `i32::MAX` or to 16 bits.
#![allow(unsafe_code)]
use core::ffi::{CStr, c_char, c_void};
use obs_output::ffi::encoder_group_num_started::obs_encoder_group_num_started;
use proptest::prelude::*;

type Query = unsafe extern "C" fn(*mut c_void) -> u32;
unsafe extern "C" {
    fn oracle_encoder_group_num_started_probe(
        query: Query,
        first: u32,
        second: u32,
        on_lock: u32,
        on_unlock: u32,
        flags: u8,
    ) -> u32;
    fn oracle_encoder_group_num_started_original(
        first: u32,
        second: u32,
        on_lock: u32,
        on_unlock: u32,
        flags: u8,
        trace: *mut c_char,
        trace_size: usize,
        after: *mut u32,
    ) -> u32;
    fn oracle_encoder_group_num_started_layout(
        mutex: *mut usize,
        count: *mut usize,
        count_size: *mut usize,
    );
    fn oracle_encoder_group_num_started_adapters() -> bool;
    fn obs_encoder_group_num_started_lock(group: *mut c_void);
    fn obs_encoder_group_num_started_read(group: *const c_void) -> u32;
    fn obs_encoder_group_num_started_unlock(group: *mut c_void);
}

/// Case flags, as the oracle reads them.
const ON_LOCK: u8 = 0x01;
const ON_UNLOCK: u8 = 0x02;
const SECOND: u8 = 0x04;

/// The test's counts.
const VALUES: [u32; 6] = [0, 1, 2, 32, u32::MAX - 1, u32::MAX];

fn probe(query: Query, first: u32, second: u32, on_lock: u32, on_unlock: u32, flags: u8) -> u32 {
    // SAFETY: the oracle owns the fixture groups every candidate reaches.
    unsafe {
        oracle_encoder_group_num_started_probe(query, first, second, on_lock, on_unlock, flags)
    }
}

/// The original's result, trace (`L` lock, `U` unlock, `K` left held) and
/// the queried group's count afterwards.
fn original(
    first: u32,
    second: u32,
    on_lock: u32,
    on_unlock: u32,
    flags: u8,
) -> (u32, String, u32) {
    let mut trace = [0 as c_char; 64];
    let mut after = 0;
    // SAFETY: the buffer and `after` are writable; the oracle writes
    // NUL-terminated text.
    let value = unsafe {
        oracle_encoder_group_num_started_original(
            first,
            second,
            on_lock,
            on_unlock,
            flags,
            trace.as_mut_ptr(),
            trace.len(),
            &raw mut after,
        )
    };
    // SAFETY: NUL-terminated by the oracle.
    let trace = unsafe { CStr::from_ptr(trace.as_ptr()) }
        .to_str()
        .unwrap()
        .to_owned();
    (value, trace, after)
}

#[test]
fn real_header_layout_and_adapters() {
    let (mut mutex, mut count, mut count_size) = (usize::MAX, 0, 0);
    // SAFETY: three writable sizes.
    unsafe {
        oracle_encoder_group_num_started_layout(&raw mut mutex, &raw mut count, &raw mut count_size)
    };
    // The mutex leads the struct; the count is a full uint32_t after it.
    assert_eq!(mutex, 0);
    assert!(count > mutex);
    assert_eq!(count_size, size_of::<u32>());
    // SAFETY: the oracle checks the adapters on its own fixture.
    assert!(unsafe { oracle_encoder_group_num_started_adapters() });
}

#[test]
fn original_matches_the_characterization() {
    // exact_count_and_group
    for value in VALUES {
        assert_eq!(
            original(value, !value, 0, 0, 0),
            (value, "LU".into(), value)
        );
        assert_eq!(
            original(value, !value, 0, 0, SECOND),
            (!value, "LU".into(), !value)
        );
    }
    // snapshot_inside_mutex_calls
    for value in [0, 1, u32::MAX] {
        assert_eq!(
            original(!value, 0, value, !value, ON_LOCK | ON_UNLOCK),
            (value, "LU".into(), !value)
        );
    }
}

#[test]
fn characterized_edges() {
    for first in VALUES {
        for second in [0, u32::MAX] {
            for flags in 0..8u8 {
                for (on_lock, on_unlock) in [(1, 2), (u32::MAX, 0), (0, u32::MAX)] {
                    assert_eq!(
                        probe(
                            obs_encoder_group_num_started,
                            first,
                            second,
                            on_lock,
                            on_unlock,
                            flags
                        ),
                        0,
                        "{first} {second} {on_lock} {on_unlock} {flags:#x}"
                    );
                }
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(512), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn any_case_matches_c(
        first in any::<u32>(),
        second in any::<u32>(),
        on_lock in any::<u32>(),
        on_unlock in any::<u32>(),
        flags in 0..8u8,
    ) {
        prop_assert_eq!(probe(obs_encoder_group_num_started, first, second, on_lock, on_unlock, flags), 0);
    }
}

// ---------------------------------------------------------------------------
// mutants, over the same adapters

/// The unbroken steps, written directly over the adapters.
unsafe extern "C" fn unbroken(group: *mut c_void) -> u32 {
    // SAFETY: mutants run only inside the probe, on its fixture groups.
    unsafe {
        obs_encoder_group_num_started_lock(group);
        let value = obs_encoder_group_num_started_read(group);
        obs_encoder_group_num_started_unlock(group);
        value
    }
}

unsafe extern "C" fn read_before_lock(group: *mut c_void) -> u32 {
    // SAFETY: as in `unbroken`.
    unsafe {
        let value = obs_encoder_group_num_started_read(group);
        obs_encoder_group_num_started_lock(group);
        obs_encoder_group_num_started_unlock(group);
        value
    }
}

unsafe extern "C" fn read_after_unlock(group: *mut c_void) -> u32 {
    // SAFETY: as in `unbroken`.
    unsafe {
        obs_encoder_group_num_started_lock(group);
        obs_encoder_group_num_started_unlock(group);
        obs_encoder_group_num_started_read(group)
    }
}

unsafe extern "C" fn without_lock(group: *mut c_void) -> u32 {
    // SAFETY: as in `unbroken`.
    unsafe {
        let value = obs_encoder_group_num_started_read(group);
        obs_encoder_group_num_started_unlock(group);
        value
    }
}

unsafe extern "C" fn without_unlock(group: *mut c_void) -> u32 {
    // SAFETY: as in `unbroken`; the probe records the mutex left held.
    unsafe {
        obs_encoder_group_num_started_lock(group);
        obs_encoder_group_num_started_read(group)
    }
}

unsafe extern "C" fn clamped_to_i32(group: *mut c_void) -> u32 {
    // SAFETY: as in `unbroken`.
    let value = unsafe { unbroken(group) };
    value.min(i32::MAX.unsigned_abs())
}

unsafe extern "C" fn clamped_to_u16(group: *mut c_void) -> u32 {
    // SAFETY: as in `unbroken`.
    let value = unsafe { unbroken(group) };
    value.min(u32::from(u16::MAX))
}

#[test]
fn mutants_are_caught() {
    let sweep = |query: Query| {
        let mut diff = 0;
        for first in VALUES {
            for flags in 0..8u8 {
                diff |= probe(query, first, !first, first ^ 1, first ^ 2, flags);
            }
        }
        diff
    };
    // The unbroken model of the steps passes the same sweep.
    assert_eq!(sweep(unbroken), 0);
    for (name, mutant) in [
        ("read_before_lock", read_before_lock as Query),
        ("read_after_unlock", read_after_unlock),
        ("without_lock", without_lock),
        ("without_unlock", without_unlock),
        ("clamped_to_i32", clamped_to_i32),
        ("clamped_to_u16", clamped_to_u16),
    ] {
        assert_ne!(sweep(mutant), 0, "{name} survived");
    }
}
