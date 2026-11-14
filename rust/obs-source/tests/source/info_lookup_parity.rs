//! Tier 3: the Rust get_source_info against the original C of
//! libobs/obs-source-info-lookup.c.h, over the production registry adapter on
//! a per-thread stand-in core: the same entry (by address) or none, and the
//! registry left unchanged.
//!
//! Intentional differences, excluded from the comparison and checked on the
//! Rust side alone:
//! - a NULL asked ID finds nothing; the C passes it to strcmp, which crashes
//!   once any type is registered (with none registered both find nothing,
//!   and that case is compared);
//! - a registered NULL ID matches nothing; the C would pass it to strcmp.
use core::ffi::c_char;
use obs_source::ffi::source_info_lookup::get_source_info;
use proptest::prelude::*;
use std::ffi::{CString, c_void};

unsafe extern "C" {
    fn oracle_source_info_lookup_check(
        ids: *const *const c_char,
        num: usize,
        id: *const c_char,
        lookup: unsafe extern "C" fn(*const c_char) -> *mut c_void,
        rust_index: *mut isize,
        c_index: *mut isize,
    ) -> bool;
    fn oracle_source_info_lookup_unread_id(
        lookup: unsafe extern "C" fn(*const c_char) -> *mut c_void,
    ) -> bool;
    fn oracle_source_info_lookup_null_ids(
        lookup: unsafe extern "C" fn(*const c_char) -> *mut c_void,
    ) -> bool;
}

/// Registers `ids` on this thread and looks `id` up on both sides; returns
/// the index found (both sides agree, or the test fails).
fn compare(ids: &[&[u8]], id: Option<&[u8]>) -> Option<usize> {
    let owned: Vec<CString> = ids.iter().map(|id| CString::new(*id).unwrap()).collect();
    let pointers: Vec<*const c_char> = owned.iter().map(|id| id.as_ptr()).collect();
    let id = id.map(|id| CString::new(id).unwrap());
    let (mut rust, mut c) = (-2, -2);
    // SAFETY: every string outlives the call; the oracle builds the registry
    // on its own stack and clears the thread's core before returning.
    let same = unsafe {
        oracle_source_info_lookup_check(
            pointers.as_ptr(),
            pointers.len(),
            id.as_ref().map_or(core::ptr::null(), |id| id.as_ptr()),
            get_source_info,
            &mut rust,
            &mut c,
        )
    };
    assert!(same, "ids {ids:?}: rust found {rust}, C found {c}");
    assert_eq!(rust, c);
    usize::try_from(rust).ok()
}

#[test]
fn characterized_cases_match_c() {
    assert_eq!(compare(&[], Some(b"lookup_first")), None);
    assert_eq!(compare(&[], Some(b"")), None);
    let ids: [&[u8]; 2] = [b"lookup_first", b"lookup_second"];
    assert_eq!(compare(&ids, Some(b"lookup_first")), Some(0));
    assert_eq!(compare(&ids, Some(b"lookup_second")), Some(1));
    for miss in [
        &b""[..],
        b"lookup",
        b"lookup_firs",
        b"lookup_first ",
        b"lookup_firstx",
        b"LOOKUP_FIRST",
        b"lookup_third",
    ] {
        assert_eq!(compare(&ids, Some(miss)), None);
    }
    let dup: [&[u8]; 3] = [b"a", b"lookup_dup_v1", b"lookup_dup_v1"];
    assert_eq!(compare(&dup, Some(b"lookup_dup_v1")), Some(1));
    assert_eq!(compare(&[b"", b"a"], Some(b"")), Some(0));
    assert_eq!(compare(&[b"\xc3\xa9", b"\xff"], Some(b"\xff")), Some(1));
    assert_eq!(compare(&[b"\xc3\xa9"], Some(b"e\xcc\x81")), None);
    // A NULL ID with nothing registered: the C loop never runs.
    assert_eq!(compare(&[], None), None);
}

/// With nothing registered, neither side reads the asked ID: both take an
/// unreadable pointer and find nothing.
#[test]
fn empty_registry_never_reads_the_id() {
    // SAFETY: neither side dereferences the pointer with no entries; the
    // oracle clears this thread's registry first.
    assert!(unsafe { oracle_source_info_lookup_unread_id(get_source_info) });
}

/// Intentional differences: NULL asked and registered IDs (Rust only).
#[test]
fn null_ids_find_nothing() {
    // SAFETY: the oracle builds and clears its own registry.
    assert!(unsafe { oracle_source_info_lookup_null_ids(get_source_info) });
}

fn id_bytes() -> impl Strategy<Value = Vec<u8>> {
    // A small alphabet so registries hold duplicates and asked IDs match;
    // bytes 1..=255 (no interior NUL), multibyte UTF-8 and invalid bytes too.
    prop::collection::vec(
        prop_oneof![Just(b'a'), Just(b'b'), Just(b'_'), 1u8..=255],
        0..4,
    )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(512)))]
    #[test]
    fn random_registries_match_c(
        ids in prop::collection::vec(id_bytes(), 0..12),
        id in id_bytes(),
    ) {
        let ids: Vec<&[u8]> = ids.iter().map(Vec::as_slice).collect();
        let expected = ids.iter().position(|entry| *entry == id.as_slice());
        prop_assert_eq!(compare(&ids, Some(&id)), expected);
    }
}
