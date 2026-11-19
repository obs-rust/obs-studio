//! Tier 3 parity: the null audio monitoring shims against the original C
//! backend (oracle/null_monitoring.c, the C reference's copy).
//!
//! Both sides get the same arguments: NULL, pointers to caller storage that
//! is not an OBS object, unterminated and non-UTF-8 IDs, and arbitrary
//! addresses that are never valid (neither side may read them). Every result
//! must match, no callback may run, and the caller's storage must be
//! unchanged. A mutation check proves the comparison sees a backend that
//! answers differently. No intentional differences.

use core::ffi::{c_char, c_void};
use std::cell::Cell;
use std::ptr;

use obs_audio_monitoring::ffi::null_monitoring::{
    EnumDeviceCallback, obs_null_audio_monitor_create, obs_null_audio_monitor_destroy,
    obs_null_audio_monitor_reset, obs_null_audio_monitoring_available,
    obs_null_audio_monitoring_devices_match, obs_null_audio_monitoring_enum_devices,
};
use obs_c_oracle::null_monitoring::*;
use proptest::prelude::*;

thread_local! { static CALLS: Cell<u32> = const { Cell::new(0) }; }

unsafe extern "C" fn count_device(_: *mut c_void, _: *const c_char, _: *const c_char) -> bool {
    CALLS.with(|calls| calls.set(calls.get() + 1));
    // Would stop enumeration: no backend may call it even once.
    false
}

unsafe extern "C" fn keep_going(_: *mut c_void, _: *const c_char, _: *const c_char) -> bool {
    CALLS.with(|calls| calls.set(calls.get() + 1));
    true
}

/// One backend's six functions.
struct Backend {
    available: unsafe extern "C" fn() -> bool,
    enum_devices: unsafe extern "C" fn(EnumDeviceCallback, *mut c_void),
    create: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
    reset: unsafe extern "C" fn(*mut c_void),
    destroy: unsafe extern "C" fn(*mut c_void),
    devices_match: unsafe extern "C" fn(*const c_char, *const c_char) -> bool,
}

const ORACLE: Backend = Backend {
    available: oracle_null_audio_monitoring_available,
    enum_devices: oracle_null_audio_monitoring_enum_devices,
    create: oracle_null_audio_monitor_create,
    reset: oracle_null_audio_monitor_reset,
    destroy: oracle_null_audio_monitor_destroy,
    devices_match: oracle_null_audio_monitoring_devices_match,
};

const RUST: Backend = Backend {
    available: obs_null_audio_monitoring_available,
    enum_devices: obs_null_audio_monitoring_enum_devices,
    create: obs_null_audio_monitor_create,
    reset: obs_null_audio_monitor_reset,
    destroy: obs_null_audio_monitor_destroy,
    devices_match: obs_null_audio_monitoring_devices_match,
};

/// A backend that matches equal, non-NULL ID pointers: what a real backend
/// that compares IDs would answer for the same ID twice.
unsafe extern "C" fn mutant_devices_match(id1: *const c_char, id2: *const c_char) -> bool {
    !id1.is_null() && id1 == id2
}

const MUTANT: Backend = Backend {
    devices_match: mutant_devices_match,
    ..RUST
};

/// Everything one backend answers for `handles` and every pair of `ids`,
/// with the number of callbacks it made.
#[derive(Debug, PartialEq, Eq)]
struct Answers {
    available: bool,
    created: Vec<usize>,
    matches: Vec<bool>,
    callbacks: u32,
}

fn answers(backend: &Backend, handles: &[*mut c_void], ids: &[*const c_char]) -> Answers {
    CALLS.with(|calls| calls.set(0));
    // SAFETY: the null backends read none of their arguments, so any address
    // is accepted; the callbacks only count.
    unsafe {
        let available = (backend.available)();
        for &data in handles {
            for cb in [None, Some(count_device as _), Some(keep_going as _)] {
                (backend.enum_devices)(cb, data);
            }
        }
        let mut created = Vec::new();
        for &handle in handles {
            created.push((backend.create)(handle) as usize);
            (backend.reset)(handle);
            (backend.destroy)(handle);
            // Repeated: no ownership or state changes.
            (backend.reset)(handle);
            (backend.destroy)(handle);
            created.push((backend.create)(handle) as usize);
        }
        let mut matches = Vec::new();
        for &id1 in ids {
            for &id2 in ids {
                matches.push((backend.devices_match)(id1, id2));
            }
        }
        Answers {
            available,
            created,
            matches,
            callbacks: CALLS.with(Cell::get),
        }
    }
}

/// The C test's inputs, over this caller storage.
fn fixed(storage: &mut [u8; 64], ids: &mut [[u8; 5]; 4]) -> (Vec<*mut c_void>, Vec<*const c_char>) {
    let handles = vec![ptr::null_mut(), storage.as_mut_ptr().cast()];
    let [equal1, equal2, bytes, unterminated] = ids;
    let ids = vec![
        ptr::null(),
        c"".as_ptr(),
        c"default".as_ptr(),
        equal1.as_ptr().cast(),
        equal2.as_ptr().cast(),
        c"different".as_ptr(),
        bytes.as_ptr().cast(),
        unterminated.as_ptr().cast(),
    ];
    (handles, ids)
}

fn the_c_inputs() -> [[u8; 5]; 4] {
    [*b"same\0", *b"same\0", [0xff, 0x80, b'x', 0, 0], *b"xyxyx"]
}

#[test]
fn the_c_characterization_matches() {
    let mut storage = [0xa5u8; 64];
    let mut ids = the_c_inputs();
    let (handles, id_ptrs) = fixed(&mut storage, &mut ids);
    let expected = answers(&ORACLE, &handles, &id_ptrs);
    assert_eq!(answers(&RUST, &handles, &id_ptrs), expected);
    // The C's own answers, as test_null_audio_monitoring.c pins them.
    assert!(!expected.available);
    assert!(expected.created.iter().all(|&monitor| monitor == 0));
    assert!(expected.matches.iter().all(|&matched| !matched));
    assert_eq!(expected.callbacks, 0);
    assert_eq!(storage, [0xa5; 64]);
    assert_eq!(ids, the_c_inputs());
}

#[test]
fn the_comparison_catches_a_backend_that_matches_ids() {
    let mut storage = [0xa5u8; 64];
    let mut ids = the_c_inputs();
    let (handles, id_ptrs) = fixed(&mut storage, &mut ids);
    assert_ne!(
        answers(&MUTANT, &handles, &id_ptrs),
        answers(&ORACLE, &handles, &id_ptrs)
    );
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(256)))]
    /// Arbitrary addresses, never valid: both backends must leave them unread
    /// and answer the same.
    #[test]
    fn arbitrary_addresses(
        handles in proptest::collection::vec(any::<usize>(), 0..6),
        ids in proptest::collection::vec(any::<usize>(), 0..6),
    ) {
        let handles: Vec<*mut c_void> = handles.into_iter().map(|a| a as *mut c_void).collect();
        let ids: Vec<*const c_char> = ids.into_iter().map(|a| a as *const c_char).collect();
        let expected = answers(&ORACLE, &handles, &ids);
        prop_assert_eq!(answers(&RUST, &handles, &ids), expected);
    }
}
