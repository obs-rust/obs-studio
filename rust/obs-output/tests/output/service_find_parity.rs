//! Tier 3: original whole C lookup versus Rust through real private projections.
//! Original initialized-core / valid-C-string domain; no intentional differences.
use core::ffi::{c_char, c_void};
use obs_output::ffi::service_find::find_service;
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_service_find_check(
        ids: *const *const c_char,
        count: usize,
        capacity: usize,
        query: *const c_char,
        rust_find: unsafe extern "C" fn(*const c_char) -> *const c_void,
    ) -> bool;
}

fn check(ids: &[Vec<u8>], count: usize, query: &[u8]) {
    let ids: Vec<_> = ids
        .iter()
        .map(|id| std::ffi::CString::new(id.as_slice()).unwrap())
        .collect();
    let pointers: Vec<_> = ids.iter().map(|id| id.as_ptr()).collect();
    let query = std::ffi::CString::new(query).unwrap();
    // SAFETY: bounded arrays and valid C strings live through the call. The C
    // fixture owns the real registry layouts; SERVICE_GLOBALS excludes other users.
    assert!(unsafe {
        oracle_service_find_check(
            pointers.as_ptr(),
            count,
            ids.len(),
            query.as_ptr(),
            find_service,
        )
    });
}

#[test]
fn characterized_registry_edges() {
    let _lock = super::SERVICE_GLOBALS.lock().unwrap();
    let ids: Vec<_> = [
        b"svc".as_slice(),
        b"svc.suffix",
        b"SVC",
        b"",
        b"\xff\x80x",
        b"svc",
        b"last",
        b"outside",
    ]
    .iter()
    .map(|id| id.to_vec())
    .collect();
    for query in [b"".as_slice(), b"missing"] {
        check(&[], 0, query);
        check(&ids, 0, query);
    }
    for query in [
        b"svc".as_slice(),
        b"sv",
        b"svc.suffix",
        b"svc.suffix.more",
        b"SVC",
        b"Svc",
        b"",
        b"\xff\x80x",
        b"\xff\x81x",
        b"last",
        b"outside",
        b"missing",
    ] {
        check(&ids, 7, query);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(128)))]
    #[test]
    fn arbitrary_byte_ids_match_original(
        ids in prop::collection::vec(prop::collection::vec(1u8..=255, 0..24), 0..=8),
        query in prop::collection::vec(1u8..=255, 0..24),
        count_seed in any::<usize>(),
    ) {
        let _lock = super::SERVICE_GLOBALS.lock().unwrap();
        let count = count_seed % (ids.len() + 1);
        check(&ids, count, &query);
        for id in &ids { check(&ids, count, id); }
    }
}
