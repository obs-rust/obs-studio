//! Original C and Rust queries share real-header fixtures; no intentional differences.
use obs_core::ffi::output_protocol_registry::{
    obs_enum_output_protocols, obs_is_output_protocol_registered,
};
use proptest::prelude::*;
use std::ffi::{CString, c_char};

type Registered = unsafe extern "C" fn(*const c_char) -> bool;
type Enumerate = unsafe extern "C" fn(usize, *mut *mut c_char) -> bool;
unsafe extern "C" {
    fn oracle_output_protocol_registry_check(
        names: *mut *mut c_char,
        count: usize,
        needle: *const c_char,
        index: usize,
        registered: Registered,
        enumerate: Enumerate,
    ) -> bool;
    fn oracle_output_protocol_registry_layout(layout: *mut usize);
}

fn check(names: &[Vec<u8>], needle: Option<&[u8]>, index: usize) {
    let strings: Vec<_> = names
        .iter()
        .map(|name| CString::new(name.as_slice()).unwrap())
        .collect();
    let mut pointers: Vec<_> = strings
        .iter()
        .map(|name| name.as_ptr().cast_mut())
        .collect();
    let needle = needle.map(|needle| CString::new(needle).unwrap());
    // SAFETY: at most 16 terminated names remain live throughout the callback.
    // C supplies a thread-local real core and compares results and borrowed pointer
    // identity, including unchanged output on failure. No callback writes strings.
    assert!(unsafe {
        oracle_output_protocol_registry_check(
            pointers.as_mut_ptr(),
            pointers.len(),
            needle
                .as_ref()
                .map_or(std::ptr::null(), |needle| needle.as_ptr()),
            index,
            obs_is_output_protocol_registered,
            obs_enum_output_protocols,
        )
    });
    for (string, original) in strings.iter().zip(names) {
        assert_eq!(string.as_bytes(), original);
    }
}

#[test]
fn real_header_scalar_layout() {
    let mut layout = [0; 4];
    // SAFETY: the oracle writes exactly four real-header scalar measurements.
    unsafe { oracle_output_protocol_registry_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<usize>(),
            align_of::<usize>(),
            size_of::<*mut c_char>(),
            align_of::<*mut c_char>()
        ]
    );
}

#[test]
fn characterized_edges_match_c() {
    let names: Vec<_> = [
        b"RTMP".as_slice(),
        b"RTMPS",
        b"RTMP",
        b"rtmp",
        b" RIST",
        b"",
        b"\xff",
    ]
    .into_iter()
    .map(<[u8]>::to_vec)
    .collect();
    for needle in [
        None,
        Some(b"RTMP".as_slice()),
        Some(b"Rtmp"),
        Some(b"RTM"),
        Some(b"RTMP;RTMPS"),
        Some(b"RIST"),
        Some(b""),
        Some(b"\xff"),
    ] {
        for index in [0, 1, 2, 3, 4, 5, 6, 7, usize::MAX] {
            check(&names, needle, index);
            check(&[], needle, index);
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn queries_match_c(
        names in prop::collection::vec(prop::collection::vec(1u8..=255, 0..32), 0..16),
        needle in prop::option::of(prop::collection::vec(1u8..=255, 0..32)),
        index in any::<usize>(),
    ) {
        check(&names, needle.as_deref(), index);
        check(&names, needle.as_deref(), names.len());
        for (index, name) in names.iter().enumerate() { check(&names, Some(name), index); }
    }
}
