//! Tier 3: original C vs shim vs safe selection, including borrowed NULL IDs
//! and context-sized unsupported objects. No intentional differences.
use core::ffi::{c_char, c_int, c_void};
use obs_core::{ffi::object_id::obs_obj_get_id, object_id};
use proptest::prelude::*;
use proptest::test_runner::{Config, TestRunner};

unsafe extern "C" {
    fn oracle_object_id_check(
        kind: c_int,
        present: bool,
        id: *const c_char,
        original_id: *const c_char,
        query: unsafe extern "C" fn(*mut c_void) -> *const c_char,
        safe_id: *const c_char,
    ) -> bool;
}

fn check(kind: i32, present: bool, bytes: Option<Vec<u8>>) {
    let id = bytes
        .as_ref()
        .map_or(core::ptr::null(), |bytes| bytes.as_ptr().cast());
    let before = bytes.clone();
    let safe = object_id::query(present.then_some(kind), |_| id).unwrap_or(core::ptr::null());
    // SAFETY: C allocates exactly the real-header layout for supported kinds,
    // and only a context otherwise. IDs remain borrowed and live throughout;
    // the oracle compares pointers and all object bytes, then frees its fixture.
    assert!(unsafe {
        oracle_object_id_check(
            kind,
            present,
            id,
            c"original encoder ID".as_ptr(),
            obs_obj_get_id,
            safe,
        )
    });
    assert_eq!(bytes, before);
}

#[test]
fn object_id_matches_original_c() {
    for kind in [i32::MIN, -1, 0, 1, 2, 3, 4, 5, 6, i32::MAX] {
        for present in [false, true] {
            for bytes in [None, Some(vec![0]), Some(b"concrete_id\0".to_vec())] {
                check(kind, present, bytes);
            }
        }
    }
    TestRunner::new(Config {
        cases: obs_c_oracle::parity::cases(128),
        failure_persistence: None,
        ..Config::default()
    })
    .run(
        &(
            any::<i32>(),
            any::<bool>(),
            prop::option::of(prop::collection::vec(any::<u8>(), 0..65)),
        ),
        |(kind, present, bytes)| {
            let bytes = bytes.map(|mut bytes| {
                bytes.push(0);
                bytes
            });
            check(kind, present, bytes.clone());
            // Every generated ID also reaches each supported concrete layout.
            for kind in 0..=6 {
                check(kind, true, bytes.clone());
            }
            Ok(())
        },
    )
    .unwrap();
}
