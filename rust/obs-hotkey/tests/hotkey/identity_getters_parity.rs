//! Tier 3: real C hotkeys, original C getters and Rust shims (#3924).
//! Exact stored values, pointer identity and unchanged bytes. No intentional
//! differences. Null hotkeys are excluded because the original C dereferences them.
use obs_c_oracle as _;
use obs_hotkey::ffi::{identity_getters as rs, pair::obs_hotkey_t};
use proptest::prelude::*;
use std::ffi::{c_char, c_void};
use std::mem::{align_of, offset_of, size_of};

unsafe extern "C" {
    fn oracle_hotkey_identity_layout(values: *mut usize);
    fn oracle_hotkey_identity_new(id: usize, name: *mut c_char) -> *mut obs_hotkey_t;
    fn oracle_hotkey_identity_free(handle: *mut c_void);
    fn oracle_hotkey_identity_probe(
        handle: *const c_void,
        get_id: unsafe extern "C" fn(*const obs_hotkey_t) -> usize,
        get_name: unsafe extern "C" fn(*const obs_hotkey_t) -> *const c_char,
    ) -> bool;
    fn oracle_obs_hotkey_identity_fields(
        key: *const obs_hotkey_t,
    ) -> rs::obs_hotkey_identity_fields;
}

/// The shared binary also exercises save/load on its own fake registry. Route
/// only those known pointers to that fixture; real hotkeys use the C adapter.
/// # Safety
/// `key` is a live real C hotkey or one of the current thread's save/load fixtures.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkey_identity_fields(
    key: *const obs_hotkey_t,
) -> rs::obs_hotkey_identity_fields {
    super::save_load_parity::identity_fields(key).unwrap_or_else(|| {
        // SAFETY: a pointer outside the fake registry is a real C hotkey.
        unsafe { oracle_obs_hotkey_identity_fields(key) }
    })
}

#[test]
fn real_header_layout() {
    type Fields = rs::obs_hotkey_identity_fields;
    let rust = [
        size_of::<Fields>(),
        align_of::<Fields>(),
        offset_of!(Fields, id),
        offset_of!(Fields, name),
        size_of::<usize>(),
        align_of::<usize>(),
    ];
    let mut c = [0; 6];
    // SAFETY: the helper fills exactly six size_t values from the real headers.
    unsafe { oracle_hotkey_identity_layout(c.as_mut_ptr()) };
    assert_eq!(rust, c);
}

struct Fixture(*mut obs_hotkey_t);
impl Drop for Fixture {
    fn drop(&mut self) {
        // SAFETY: this fixture owns the allocation created by the C helper.
        unsafe { oracle_hotkey_identity_free(self.0.cast()) };
    }
}

fn check(id: usize, mut bytes: Vec<u8>, has_name: bool) {
    bytes.push(0);
    let before = bytes.clone();
    let name = if has_name {
        bytes.as_mut_ptr().cast()
    } else {
        std::ptr::null_mut()
    };
    // SAFETY: C borrows the name, whose allocation stays live until the fixture drops.
    let key = Fixture(unsafe { oracle_hotkey_identity_new(id, name) });
    assert!(!key.0.is_null());
    // SAFETY: this is an initialized real C hotkey. Callbacks read stored fields
    // only; the C probe compares original getters and every hotkey byte.
    unsafe {
        assert!(oracle_hotkey_identity_probe(
            key.0.cast(),
            rs::obs_hotkey_get_id,
            rs::obs_hotkey_get_name,
        ));
        assert_eq!(rs::obs_hotkey_get_id(key.0), id);
        assert_eq!(rs::obs_hotkey_get_name(key.0), name.cast_const());
    }
    assert_eq!(bytes, before);
}

#[test]
fn characterized_edges() {
    for id in [0, 1, 17, usize::MAX - 1, usize::MAX] {
        for has_name in [false, true] {
            check(id, vec![b'x', 0xff], has_name);
            check(id, vec![], has_name);
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(128),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn values_match_c(id in any::<usize>(), bytes in prop::collection::vec(any::<u8>(), 0..65),
        has_name in any::<bool>()) {
        check(id, bytes, has_name);
    }
}
