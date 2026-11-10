//! Exact original-C registry-query parity, with no intentional differences.
//! Nonempty registries require non-NULL C strings, as the original strcmp does.
use core::ffi::{c_char, c_int};
use core::mem::{align_of, offset_of, size_of};
use obs_output::ffi::encoder_registry_queries::{self as rust, RegistryQueryFields};
use proptest::prelude::*;
use std::ffi::CString;

unsafe extern "C" {
    fn oracle_encoder_registry_layout(layout: *mut usize);
    fn oracle_encoder_registry_check(
        ids: *const *const c_char,
        kinds: *const c_int,
        caps: *const u32,
        count: usize,
        needle: *const c_char,
        get_type: unsafe extern "C" fn(*const c_char) -> c_int,
        get_caps: unsafe extern "C" fn(*const c_char) -> u32,
    ) -> bool;
}

#[test]
fn real_header_layout_and_constants() {
    let mut layout = [0; 9];
    // SAFETY: the oracle writes nine size_t entries to this live array.
    unsafe { oracle_encoder_registry_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<RegistryQueryFields>(),
            align_of::<RegistryQueryFields>(),
            offset_of!(RegistryQueryFields, kind),
            offset_of!(RegistryQueryFields, caps),
            size_of::<c_int>(),
            align_of::<c_int>(),
            0,
            1,
            1 << 2
        ]
    );
}

fn check(ids: &[CString], kinds: &[i32], caps: &[u32], needle: Option<&CString>) {
    assert_eq!(ids.len(), kinds.len());
    assert_eq!(ids.len(), caps.len());
    assert!(ids.len() <= 8);
    let pointers: Vec<_> = ids.iter().map(|id| id.as_ptr()).collect();
    let needle = needle.map_or(core::ptr::null(), |id| id.as_ptr());
    // SAFETY: matching bounded slices and all C strings stay live throughout
    // the call. The fixture owns a real, thread-local registry during callbacks.
    assert!(unsafe {
        oracle_encoder_registry_check(
            pointers.as_ptr(),
            kinds.as_ptr(),
            caps.as_ptr(),
            ids.len(),
            needle,
            rust::obs_get_encoder_type,
            rust::obs_get_encoder_caps,
        )
    });
}

#[test]
fn exact_edges_lookup_order_and_missing_defaults() {
    let ids = [
        c"audio".into(),
        c"video".into(),
        c"video".into(),
        c"".into(),
        c"\xff".into(),
    ];
    let kinds = [0, 1, 0, 1, 0];
    let missing = CString::new("missing").unwrap();
    for mask in [0, 1, 1 << 2, 0x8000_0000, 0xaaaa_5555, u32::MAX] {
        let caps = [mask, !mask, 0x1234_5678, mask, !mask];
        for needle in ids.iter().chain([&missing]) {
            check(&ids, &kinds, &caps, Some(needle));
        }
    }
    check(&[], &[], &[], Some(&missing));
    // Original find_encoder never dereferences id when there are no entries.
    check(&[], &[], &[], None);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(256)))]
    #[test]
    fn generated_registry_queries(
        entries in prop::collection::vec((prop::collection::vec(1u8..=255, 0..24), any::<bool>(), any::<u32>()), 0..9),
        query in prop::collection::vec(1u8..=255, 0..24),
        index in 0usize..9,
    ) {
        let ids: Vec<_> = entries.iter().map(|entry| CString::new(entry.0.clone()).unwrap()).collect();
        let kinds: Vec<_> = entries.iter().map(|entry| i32::from(entry.1)).collect();
        let caps: Vec<_> = entries.iter().map(|entry| entry.2).collect();
        let query = CString::new(query).unwrap();
        check(&ids, &kinds, &caps, Some(&query));
        if let Some(needle) = ids.get(index) {
            // A separate allocation checks byte comparison rather than identity.
            check(&ids, &kinds, &caps, Some(&needle.clone()));
        }
    }
}
