//! Tier 3 against the original C wrappers, both over the ported lookup and a
//! per-thread registry. No-core calls, NULL IDs in populated registries and
//! matched name queries without get_name are outside the characterized C
//! domain; the lookup's own parity is in `info_lookup_parity`.
use core::ffi::c_char;
use obs_source::ffi::type_info::{
    Fields, obs_get_source_output_flags, obs_is_source_configurable, obs_source_get_display_name,
};
use proptest::prelude::*;

type Name = unsafe extern "C" fn(*const c_char) -> *const c_char;
type Flags = unsafe extern "C" fn(*const c_char) -> u32;
type Configurable = unsafe extern "C" fn(*const c_char) -> bool;
unsafe extern "C" {
    fn oracle_source_type_info_check(
        ids: *const *const c_char,
        flags: *const u32,
        masks: *const u8,
        count: usize,
        query: *const c_char,
        results: *const *const c_char,
        result_count: usize,
        null_data: u32,
        names: bool,
        name: Name,
        flags_query: Flags,
        configurable: Configurable,
    ) -> bool;
    fn oracle_source_type_info_layout(out: *mut usize);
    fn oracle_source_type_info_mutable(name: Name) -> bool;
}
fn terminated(bytes: &[u8]) -> Vec<u8> {
    let mut bytes = bytes.to_vec();
    bytes.push(0);
    bytes
}
fn check(entries: &[(Vec<u8>, u32, u8)], query: Option<&[u8]>, null_data: u32, names: bool) {
    assert!(entries.len() <= 8);
    assert!(entries.is_empty() || query.is_some());
    let ids: Vec<_> = entries.iter().map(|e| terminated(&e.0)).collect();
    let id_ptrs: Vec<_> = ids.iter().map(|s| s.as_ptr().cast()).collect();
    let flags: Vec<_> = entries.iter().map(|e| e.1).collect();
    let masks: Vec<_> = entries.iter().map(|e| e.2).collect();
    let query = query.map(terminated);
    let query = query
        .as_ref()
        .map_or(core::ptr::null(), |s| s.as_ptr().cast());
    let first = terminated(b"Mixed Name");
    let second = terminated(b"Mixed Name");
    assert_ne!(first.as_ptr(), second.as_ptr());
    let results = [
        core::ptr::null(),
        c"".as_ptr(),
        first.as_ptr().cast(),
        second.as_ptr().cast(),
        c"\xfe\x81".as_ptr(),
        first.as_ptr().cast(),
    ];
    // SAFETY: C owns the real-layout thread-local registry for each bounded
    // call. Every array/string stays live. NULL IDs reach only empty registries;
    // absent get_name is exercised only by flags/configurability queries.
    assert!(unsafe {
        oracle_source_type_info_check(
            id_ptrs.as_ptr(),
            flags.as_ptr(),
            masks.as_ptr(),
            entries.len(),
            query,
            results.as_ptr(),
            results.len(),
            null_data,
            names,
            obs_source_get_display_name,
            obs_get_source_output_flags,
            obs_is_source_configurable,
        )
    });
}
#[test]
fn real_header_source_type_info_snapshot_layout() {
    let mut c = [0; 7];
    // SAFETY: C writes exactly seven real-header size/alignment/offset values.
    unsafe { oracle_source_type_info_layout(c.as_mut_ptr()) };
    assert_eq!(
        c,
        [
            size_of::<Fields>(),
            align_of::<Fields>(),
            core::mem::offset_of!(Fields, get_name),
            core::mem::offset_of!(Fields, type_data),
            core::mem::offset_of!(Fields, output_flags),
            core::mem::offset_of!(Fields, has_properties),
            core::mem::offset_of!(Fields, has_properties2)
        ]
    );
}
#[test]
fn characterized_source_type_info_edges() {
    check(&[], None, 0, true);
    for id in [b"".as_slice(), b"unknown"] {
        check(&[], Some(id), 0, true);
    }
    for mask in 0..16 {
        for flags in (0..32)
            .map(|bit| 1 << bit)
            .chain([0, u32::MAX, u32::MAX ^ 0x40])
        {
            let entries = vec![(b"flags".to_vec(), flags, mask)];
            check(&entries, Some(b"flags"), 0, true);
            check(&entries, Some(b"flags"), 1, false);
        }
    }
    let entries = vec![
        (b"prefix".to_vec(), 4, 0),
        (b"first".to_vec(), 2, 2),
        (b"first".to_vec(), 0, 0),
        (b"first-suffix".to_vec(), 0x8000_0000, 1),
        (b"\xfeid\0".to_vec(), u32::MAX, 3),
        (vec![], 0x8000_0000, 12),
    ];
    for id in [
        b"prefix".as_slice(),
        b"first\0ignored",
        b"first-suffix",
        b"\xfeid\0x",
        b"",
        b"First",
        b"firs",
        b"first-suffix-missing",
        b"\xfe",
        b"unknown",
    ] {
        for mask in [0, u32::MAX, 0b101010] {
            check(&entries, Some(id), mask, true);
        }
        check(&entries, Some(id), 0, false);
    }
    // SAFETY: fixture owns mutable storage and both borrowed results throughout.
    assert!(unsafe { oracle_source_type_info_mutable(obs_source_get_display_name) });
}
#[test]
fn empty_source_type_registry_does_not_read_query() {
    let results = [core::ptr::null()];
    // SAFETY: original lookup reads no query bytes in a live empty registry.
    // This non-string pointer is never dereferenced; result array remains live.
    assert!(unsafe {
        oracle_source_type_info_check(
            core::ptr::null(),
            core::ptr::null(),
            core::ptr::null(),
            0,
            core::ptr::dangling(),
            results.as_ptr(),
            1,
            0,
            true,
            obs_source_get_display_name,
            obs_get_source_output_flags,
            obs_is_source_configurable,
        )
    });
}
proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(128)))]
    #[test]
    fn arbitrary_source_type_info(
        entries in prop::collection::vec((prop::collection::vec(any::<u8>(), 0..24), any::<u32>(), 0u8..16), 0..9),
        arbitrary in prop::collection::vec(any::<u8>(), 0..24), selected in any::<usize>(),
        null_data in any::<u32>(), names in any::<bool>(),
    ) {
        check(&entries, Some(&arbitrary), null_data, names);
        if !entries.is_empty() {
            let selected = selected % entries.len();
            check(&entries, Some(&entries[selected].0), null_data, names);
            let mut first_nul = entries[selected].0.clone();
            first_nul.extend_from_slice(b"\0ignored\xff");
            check(&entries, Some(&first_nul), null_data, names);
        }
    }
}
