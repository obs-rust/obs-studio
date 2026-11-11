//! Original C scene wrappers and original source lifetime helpers, real layouts.
//! No intentional differences. Excludes overflowing increments and release
//! without strong ownership; expired acquisition retains allocated fixture storage.
use core::{
    ffi::{c_long, c_void},
    mem::{align_of, size_of},
};
use obs_scene::ffi::refs::{obs_scene_get_ref, obs_scene_release};
use proptest::test_runner::{Config, TestRunner};

unsafe extern "C" {
    fn oracle_scene_ref_probe(
        operation: u32,
        flags: u32,
        acquisitions: u32,
        refs: c_long,
        get: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
        release: unsafe extern "C" fn(*mut c_void),
    ) -> bool;
    fn oracle_scene_source_layout(layout: *mut usize);
}

#[test]
fn real_header_pointer_layout() {
    let mut layout = [0; 4];
    // SAFETY: the existing source-field oracle writes four pointer layout values.
    // The scene itself remains opaque; the shim mirrors no private struct.
    unsafe { oracle_scene_source_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<*mut c_void>(),
            align_of::<*mut c_void>(),
            size_of::<*mut c_void>(),
            align_of::<*mut c_void>()
        ]
    );
}

fn check(operation: u32, flags: u32, acquisitions: u32, refs: c_long) {
    // SAFETY: C allocates and resets separate original/ported real-header states.
    // Valid owners, nonoverflowing counts and an external weak owner keep all
    // source/control storage alive until after the byte-for-byte comparison.
    assert!(
        unsafe {
            oracle_scene_ref_probe(
                operation,
                flags,
                acquisitions,
                refs,
                obs_scene_get_ref,
                obs_scene_release,
            )
        },
        "operation={operation} flags={flags} acquisitions={acquisitions} refs={refs}"
    );
}

#[test]
fn characterized_edges_and_random_sequences_match_original_c() {
    for refs in [c_long::MIN, -3, -2, -1, 0, 1, 7, 1023, c_long::MAX - 1] {
        for flags in 0..32 {
            check(0, flags, 0, refs);
        }
    }
    for refs in [0, 1, 7, 1023, c_long::MAX] {
        for flags in [0, 1, 2, 3, 4, 8] {
            check(1, flags, 0, refs);
        }
    }
    // NULL remains safe before startup and after shutdown as in test_scene_ref.
    for flags in [1, 3, 17, 19] {
        check(1, flags, 0, 0);
    }
    for acquisitions in [0, 1, 2, 7, 32] {
        for flags in [0, 1, 8] {
            check(2, flags, acquisitions, 0);
        }
    }
    let counts = -3i32..4096;
    TestRunner::new(Config {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..Config::default()
    })
    .run(
        &(0u32..3, 0u32..32, 0u32..65, counts),
        |(operation, mut flags, acquisitions, mut refs)| {
            if operation == 1 {
                flags &= 15;
                refs = refs.max(0);
            }
            if operation == 2 {
                flags &= 9;
                refs = 0;
            }
            check(operation, flags, acquisitions, c_long::from(refs));
            Ok(())
        },
    )
    .unwrap();
}
