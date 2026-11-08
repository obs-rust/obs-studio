//! Exact current/original metadata parity and pointer identity. No exclusions.
use core::ffi::{c_char, c_int, c_void};
use core::mem::{align_of, offset_of, size_of};
use obs_output::ffi::encoder_metadata::{self as rust, MetadataFields};
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_encoder_metadata_layout(layout: *mut usize);
    fn oracle_encoder_metadata_new(
        codec: *const c_char,
        kind: c_int,
        id: *const c_char,
        data: *mut c_void,
        caps: u32,
    ) -> *mut c_void;
    fn oracle_encoder_metadata_free(encoder: *mut c_void);
    fn oracle_encoder_metadata_unchanged(encoder: *mut c_void) -> bool;
    fn oracle_obs_encoder_get_codec(encoder: *const c_void) -> *const c_char;
    fn oracle_obs_encoder_get_type(encoder: *const c_void) -> c_int;
    fn oracle_obs_encoder_get_id(encoder: *const c_void) -> *const c_char;
    fn oracle_obs_encoder_get_type_data(encoder: *mut c_void) -> *mut c_void;
    fn oracle_obs_encoder_get_caps(encoder: *const c_void) -> u32;
    fn oracle_encoder_metadata_null_log(rust: bool) -> bool;
}

#[test]
fn real_header_view_and_enum_layout() {
    let mut c = [0; 11];
    // SAFETY: the oracle writes exactly eleven size_t values.
    unsafe { oracle_encoder_metadata_layout(c.as_mut_ptr()) };
    assert_eq!(
        c,
        [
            size_of::<MetadataFields>(),
            align_of::<MetadataFields>(),
            offset_of!(MetadataFields, codec),
            offset_of!(MetadataFields, kind),
            offset_of!(MetadataFields, id),
            offset_of!(MetadataFields, type_data),
            offset_of!(MetadataFields, caps),
            size_of::<c_int>(),
            align_of::<c_int>(),
            0,
            1,
        ]
    );
}

struct Encoder(*mut c_void);
impl Drop for Encoder {
    fn drop(&mut self) {
        // SAFETY: this fixture owns the C allocation, freed exactly once.
        unsafe { oracle_encoder_metadata_free(self.0) };
    }
}

fn check(codec_index: usize, id_index: usize, has_data: bool, kind: i32, caps: u32) {
    let strings = [
        core::ptr::null(),
        c"".as_ptr(),
        c"codec".as_ptr(),
        c"original-id".as_ptr(),
        c"utf8-\u{e9}".as_ptr(),
    ];
    let codec = strings[codec_index];
    let id = strings[id_index];
    let mut marker = 29u32;
    let data = if has_data {
        core::ptr::from_mut(&mut marker).cast()
    } else {
        core::ptr::null_mut()
    };
    // SAFETY: valid enum values and borrowed pointers remain live through the call.
    let encoder = Encoder(unsafe { oracle_encoder_metadata_new(codec, kind, id, data, caps) });
    assert!(!encoder.0.is_null());
    // SAFETY: the real-layout fixture is exclusive and remains live in this scope.
    unsafe {
        assert_eq!(
            rust::obs_encoder_get_codec(encoder.0),
            oracle_obs_encoder_get_codec(encoder.0)
        );
        assert_eq!(rust::obs_encoder_get_codec(encoder.0), codec);
        assert_eq!(
            rust::obs_encoder_get_type(encoder.0),
            oracle_obs_encoder_get_type(encoder.0)
        );
        assert_eq!(rust::obs_encoder_get_type(encoder.0), kind);
        assert_eq!(
            rust::obs_encoder_get_id(encoder.0),
            oracle_obs_encoder_get_id(encoder.0)
        );
        assert_eq!(rust::obs_encoder_get_id(encoder.0), id);
        assert_eq!(
            rust::obs_encoder_get_type_data(encoder.0),
            oracle_obs_encoder_get_type_data(encoder.0)
        );
        assert_eq!(rust::obs_encoder_get_type_data(encoder.0), data);
        assert_eq!(
            rust::obs_encoder_get_caps(encoder.0),
            oracle_obs_encoder_get_caps(encoder.0)
        );
        assert_eq!(rust::obs_encoder_get_caps(encoder.0), caps);
        assert!(oracle_encoder_metadata_unchanged(encoder.0));
    }
    assert_eq!(marker, 29);
}

#[test]
fn characterized_edges() {
    for codec in 0..5 {
        for id in 0..5 {
            for has_data in [false, true] {
                for kind in [0, 1] {
                    for caps in [0, 1, 0x8000_0000, u32::MAX] {
                        check(codec, id, has_data, kind, caps);
                    }
                }
            }
        }
    }
}

#[test]
fn null_defaults_and_logs() {
    if !super::isolated(concat!(module_path!(), "::null_defaults_and_logs")) {
        return;
    }
    // SAFETY: the probe saves/restores the global logger, isolated in this child.
    unsafe {
        assert!(oracle_encoder_metadata_null_log(false));
        assert!(oracle_encoder_metadata_null_log(true));
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn random_metadata_match_c(codec in 0usize..5, id in 0usize..5, data in any::<bool>(), kind in 0i32..2, caps in any::<u32>()) {
        check(codec, id, data, kind, caps);
    }
}
