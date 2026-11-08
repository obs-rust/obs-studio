//! Exact C parity for opaque linked registries. No exclusions or own threads.
use obs_core::ffi::module_name_lookups::{obs_get_disabled_module, obs_get_module};
use proptest::prelude::*;
use std::ffi::{CString, c_char, c_void};

type Query = unsafe extern "C" fn(*const c_char) -> *mut c_void;
unsafe extern "C" {
    fn oracle_module_name_lookup_check(
        names: *const *const c_char,
        count: usize,
        needle: *const c_char,
        disabled: bool,
        loaded: Query,
        inactive: Query,
    ) -> bool;
    fn oracle_module_name_lookup_layout(layout: *mut usize);
}

fn check(names: &[Vec<u8>], needle: &[u8]) {
    let strings: Vec<_> = names
        .iter()
        .map(|n| CString::new(n.as_slice()).unwrap())
        .collect();
    let pointers: Vec<_> = strings.iter().map(|n| n.as_ptr()).collect();
    let needle = CString::new(needle).unwrap();
    for disabled in [false, true] {
        // SAFETY: bounded terminated names remain live; C owns and synchronizes
        // real-header fixtures for both queries and receives callbacks explicitly.
        assert!(unsafe {
            oracle_module_name_lookup_check(
                pointers.as_ptr(),
                pointers.len(),
                needle.as_ptr(),
                disabled,
                obs_get_module,
                obs_get_disabled_module,
            )
        });
    }
}

#[test]
fn real_header_layout() {
    let mut layout = [0; 4];
    // SAFETY: C writes four pointer layout values measured from the real header.
    unsafe { oracle_module_name_lookup_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<*const c_char>(),
            align_of::<*const c_char>(),
            size_of::<*mut c_void>(),
            align_of::<*mut c_void>()
        ]
    );
}

#[test]
fn characterized_edges() {
    let names = [
        b"alpha".to_vec(),
        vec![],
        b"alpha".to_vec(),
        b"x\xff".to_vec(),
    ];
    for needle in [
        b"alpha".as_slice(),
        b"",
        b"x\xff",
        b"Alpha",
        b"alph",
        b"missing",
    ] {
        check(&[], needle);
        check(&names, needle);
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn queries_match_c(names in prop::collection::vec(prop::collection::vec(1u8..=255, 0..32), 0..16), needle in prop::collection::vec(1u8..=255, 0..32)) {
        check(&names, &needle);
        for name in &names { check(&names, name); }
    }
}
