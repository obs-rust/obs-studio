//! Tier 3: actual C hotkeys, original C getters and Rust shims.
//! Exact pointer/value parity and unchanged object bytes. No intentional
//! differences. Null hotkey inputs are excluded because C dereferences them.
use obs_c_oracle as _;
use obs_hotkey::ffi::{object_getters as rs, pair::obs_hotkey_t};
use proptest::prelude::*;
use std::ffi::{c_char, c_uint, c_void};
use std::mem::{align_of, offset_of, size_of};

unsafe extern "C" {
    fn oracle_hotkey_object_layout(values: *mut usize);
    fn oracle_hotkey_object_new(
        description: *mut c_char,
        kind: c_uint,
        registerer: *mut c_void,
        partner: usize,
    ) -> *mut obs_hotkey_t;
    fn oracle_hotkey_object_unchanged(key: *const obs_hotkey_t) -> bool;
    fn oracle_hotkey_object_free(key: *mut obs_hotkey_t);
    fn oracle_obs_hotkey_get_description(key: *const obs_hotkey_t) -> *const c_char;
    fn oracle_obs_hotkey_get_registerer_type(key: *const obs_hotkey_t) -> c_uint;
    fn oracle_obs_hotkey_get_registerer(key: *const obs_hotkey_t) -> *mut c_void;
    fn oracle_obs_hotkey_get_pair_partner_id(key: *const obs_hotkey_t) -> usize;
}

#[test]
fn real_header_layout() {
    type Fields = rs::obs_hotkey_object_fields;
    let rust = [
        size_of::<Fields>(),
        align_of::<Fields>(),
        offset_of!(Fields, description),
        offset_of!(Fields, registerer_type),
        offset_of!(Fields, registerer),
        offset_of!(Fields, pair_partner_id),
        size_of::<c_uint>(),
        align_of::<c_uint>(),
        0,
        1,
        2,
        3,
        4,
    ];
    let mut c = [0; 13];
    // SAFETY: the helper fills exactly 13 size_t values from the real headers.
    unsafe { oracle_hotkey_object_layout(c.as_mut_ptr()) };
    assert_eq!(rust, c);
}

struct Fixture(*mut obs_hotkey_t);
impl Drop for Fixture {
    fn drop(&mut self) {
        // SAFETY: this fixture owns the allocation created by the C helper.
        unsafe { oracle_hotkey_object_free(self.0) };
    }
}

fn check(
    mut bytes: Vec<u8>,
    kind: u32,
    partner: usize,
    has_description: bool,
    has_registerer: bool,
) {
    bytes.push(0);
    let before = bytes.clone();
    let description = if has_description {
        bytes.as_mut_ptr().cast()
    } else {
        std::ptr::null_mut()
    };
    let mut token = 17u8;
    let registerer = if has_registerer {
        (&raw mut token).cast()
    } else {
        std::ptr::null_mut()
    };
    // SAFETY: C stores these borrowed pointers without taking ownership. The
    // description and token stay live until the C fixture is freed.
    let key = Fixture(unsafe { oracle_hotkey_object_new(description, kind, registerer, partner) });
    assert!(!key.0.is_null());
    // SAFETY: key is a real initialized C hotkey. Both sets of getters read
    // only stored fields, and the oracle snapshot covers every object byte.
    unsafe {
        assert_eq!(
            rs::obs_hotkey_get_description(key.0),
            oracle_obs_hotkey_get_description(key.0)
        );
        assert_eq!(
            rs::obs_hotkey_get_description(key.0),
            description.cast_const()
        );
        assert_eq!(
            rs::obs_hotkey_get_registerer_type(key.0),
            oracle_obs_hotkey_get_registerer_type(key.0)
        );
        assert_eq!(rs::obs_hotkey_get_registerer_type(key.0), kind);
        assert_eq!(
            rs::obs_hotkey_get_registerer(key.0),
            oracle_obs_hotkey_get_registerer(key.0)
        );
        assert_eq!(rs::obs_hotkey_get_registerer(key.0), registerer);
        assert_eq!(
            rs::obs_hotkey_get_pair_partner_id(key.0),
            oracle_obs_hotkey_get_pair_partner_id(key.0)
        );
        assert_eq!(rs::obs_hotkey_get_pair_partner_id(key.0), partner);
        assert!(oracle_hotkey_object_unchanged(key.0));
    }
    assert_eq!(bytes, before);
    assert_eq!(token, 17);
}

#[test]
fn characterized_edges() {
    for kind in 0..5 {
        for partner in [0, 17, usize::MAX] {
            for has_description in [false, true] {
                for has_registerer in [false, true] {
                    check(
                        vec![b'x', 0xff],
                        kind,
                        partner,
                        has_description,
                        has_registerer,
                    );
                    check(vec![], kind, partner, has_description, has_registerer);
                }
            }
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
    fn values_match_c(bytes in prop::collection::vec(any::<u8>(), 0..65), kind in 0u32..5,
        partner in any::<usize>(), has_description in any::<bool>(), has_registerer in any::<bool>()) {
        check(bytes, kind, partner, has_description, has_registerer);
    }
}
