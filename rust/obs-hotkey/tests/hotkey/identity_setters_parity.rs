//! Tier 3: original C, real C hash tables, real adapter and Rust exports.
//! No intentional differences. Absent cores and aliased replacement strings
//! are excluded because the C implementation dereferences/frees those inputs.
use obs_c_oracle as _;
use obs_hotkey::ffi::identity_setters as rs;
use proptest::prelude::*;
use std::ffi::{CString, c_char};

unsafe extern "C" {
    fn oracle_obs_hotkey_identity_slot(id: usize, description: bool) -> *mut *mut c_char;
    fn oracle_hotkey_identity_setter_check(
        id: usize,
        description: bool,
        input: *const c_char,
        setter: unsafe extern "C" fn(usize, *const c_char),
    ) -> bool;
}

/// Forward to the production adapter compiled over the real oracle-owned core.
/// # Safety
/// Called only inside the oracle callback with its live thread-local core.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkey_identity_slot(
    id: usize,
    description: bool,
) -> *mut *mut c_char {
    // SAFETY: the oracle installs its live core for the callback's duration.
    unsafe { oracle_obs_hotkey_identity_slot(id, description) }
}

fn check(id: usize, description: bool, input: Option<Vec<u8>>) {
    let input = input.map(|bytes| CString::new(bytes).unwrap());
    let ptr = input.as_ref().map_or(std::ptr::null(), |s| s.as_ptr());
    let setter = if description {
        rs::obs_hotkey_set_description
    } else {
        rs::obs_hotkey_set_name
    };
    // SAFETY: C owns both registries and all strings; the input lives through
    // the synchronous callback. The helper compares bytes and all other fields.
    assert!(unsafe { oracle_hotkey_identity_setter_check(id, description, ptr, setter) });
}

#[test]
fn characterized_edges() {
    for id in [0, 17, 1, 100, usize::MAX] {
        for description in [false, true] {
            for input in [
                None,
                Some(vec![]),
                Some(b"new.name".to_vec()),
                Some(vec![255, 128]),
            ] {
                check(id, description, input);
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(128)))]
    #[test]
    fn exact_string_and_registry_parity(
        id in prop_oneof![Just(0usize), Just(17usize), any::<usize>()],
        description in any::<bool>(),
        input in prop::option::of(prop::collection::vec(1u8..=255, 0..128)),
    ) {
        check(id, description, input);
    }
}
