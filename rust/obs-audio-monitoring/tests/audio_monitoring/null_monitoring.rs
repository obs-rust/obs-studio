//! Tier 1: `obs_audio_monitoring::null_monitoring`, the audio monitoring
//! backend Linux and FreeBSD select when PulseAudio is off.
//!
//! Ports every assertion of `test/cmocka/test_null_audio_monitoring.c`, 1:1,
//! under the same names and in the same order, against the safe core. The C
//! test's teardown also checks that no case logs or allocates; the core has
//! no path that does either, and the unchanged C test checks it against the
//! Rust through `test_null_audio_monitoring_rust` (bmem and the log handler
//! are process-wide, so a parallel Rust test binary cannot count them).

use std::ptr;

use obs_audio_monitoring::null_monitoring::{
    available, create, destroy, devices_match, enum_devices, reset,
};

/// Bytes that are not an OBS object, as the C test's `storage` union.
fn storage() -> [u8; 64] {
    [0xa5; 64]
}

#[test]
fn availability_is_false_without_startup() {
    assert!(!available());
}

#[test]
fn enumeration_never_calls_back_or_writes_data() {
    let mut data = [0xa5u8, 0x00, 0xff, 0x5a];
    let saved = data;
    // No callback, even one that would stop enumeration, may be invoked.
    let unexpected = |_: &mut [u8; 4], _: &[u8], _: &[u8]| -> bool {
        panic!("the null backend called a device callback")
    };
    enum_devices((Some(unexpected), Some(&mut data)));
    enum_devices((Some(unexpected), None::<&mut [u8; 4]>));
    enum_devices((None::<fn(&mut [u8; 4], &[u8], &[u8]) -> bool>, Some(&mut data)));
    enum_devices((
        None::<fn(&mut [u8; 4], &[u8], &[u8]) -> bool>,
        None::<&mut [u8; 4]>,
    ));
    assert_eq!(data, saved);
}

#[test]
fn lifecycle_ignores_null_and_nonnull_handles() {
    // The null backend never dereferences, initializes, takes ownership of,
    // or frees the caller's pointer.
    let mut storage = storage();
    let saved = storage;
    let handle: *mut u8 = storage.as_mut_ptr();
    assert!(create(ptr::null_mut::<u8>()).is_none());
    assert!(create(handle).is_none());
    reset(ptr::null_mut::<u8>());
    reset(handle);
    destroy(ptr::null_mut::<u8>());
    destroy(handle);
    // Repeat to characterize the absence of ownership or state changes.
    reset(handle);
    destroy(handle);
    assert!(create(handle).is_none());
    assert_eq!(storage, saved);
}

#[test]
fn device_matching_is_always_false() {
    let equal1 = *b"same\0";
    let equal2 = *b"same\0";
    let bytes = [0xffu8, 0x80, b'x', 0];
    let unterminated = *b"xy";
    let ids: [Option<&[u8]>; 8] = [
        None,
        Some(b"\0"),
        Some(b"default\0"),
        Some(&equal1),
        Some(&equal2),
        Some(b"different\0"),
        Some(&bytes),
        Some(&unterminated),
    ];
    for i in ids {
        for j in ids {
            assert!(!devices_match(i, j), "{i:?} {j:?}");
        }
    }
    assert_eq!(&equal1, b"same\0");
    assert_eq!(&equal2, b"same\0");
    assert_eq!(bytes, [0xff, 0x80, b'x', 0]);
    assert_eq!(&unterminated, b"xy");
}
