//! Tier 2 layout and Tier 3 borrowed-pointer, state and diagnostic parity.
//! No input exclusions or diagnostic normalization.
use core::ffi::{c_char, c_void};
use core::mem::{align_of, offset_of, size_of};
use obs_output::ffi::output_protocol_getter::{Fields, obs_output_get_protocols as getter};
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

type Getter = unsafe extern "C" fn(*const c_void) -> *const c_char;
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);
unsafe extern "C" {
    fn oracle_output_protocol_getter_new(protocols: *const c_char, flags: u32) -> *mut c_void;
    fn oracle_output_protocol_getter_free(output: *mut c_void);
    fn oracle_output_protocol_getter_unchanged(output: *const c_void) -> bool;
    fn oracle_output_protocol_getter_addresses(output: *const c_void) -> bool;
    fn oracle_output_protocol_getter_layout(layout: *mut usize);
    fn oracle_obs_output_get_protocols(output: *const c_void) -> *const c_char;
    fn oracle_output_protocol_getter_logs(getter: Getter, get: GetLog, set: SetLog) -> bool;
}

struct Fixture(*mut c_void);
impl Drop for Fixture {
    fn drop(&mut self) {
        // SAFETY: owns the C allocation; borrowed metadata is still live here.
        unsafe { oracle_output_protocol_getter_free(self.0) }
    }
}

#[test]
fn real_header_layout() {
    let mut layout = [0; 6];
    // SAFETY: the oracle writes exactly six size/offset values.
    unsafe { oracle_output_protocol_getter_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<Fields>(),
            align_of::<Fields>(),
            offset_of!(Fields, flags),
            offset_of!(Fields, protocols),
            size_of::<u32>(),
            obs_output::video_checks::OBS_OUTPUT_SERVICE as usize
        ]
    );
}

fn check(value: Option<&[u8]>, flags: u32) {
    let ptr = value.map_or(core::ptr::null(), |s| s.as_ptr().cast());
    // SAFETY: the C fixture borrows the buffer and never decodes it.
    let fixture = Fixture(unsafe { oracle_output_protocol_getter_new(ptr, flags) });
    assert!(!fixture.0.is_null());
    // SAFETY: the real-layout fixture and its borrowed bytes remain live.
    // Member-address accessors never receive null.
    unsafe {
        assert!(oracle_output_protocol_getter_addresses(fixture.0));
        for _ in 0..2 {
            assert_eq!(
                getter(fixture.0),
                oracle_obs_output_get_protocols(fixture.0)
            );
            let expected = if flags & obs_output::video_checks::OBS_OUTPUT_SERVICE != 0 {
                ptr
            } else {
                core::ptr::null()
            };
            assert_eq!(getter(fixture.0), expected);
            assert!(oracle_output_protocol_getter_unchanged(fixture.0));
        }
    }
}

#[test]
fn characterized_edges() {
    let values: [Option<&[u8]>; 4] = [
        None,
        Some(b"\0"),
        Some(b"rtmp;rtmps\0"),
        Some(b"rtmp;\xff\0"),
    ];
    for value in values {
        for flags in [0, 8, 1, u32::MAX, !8] {
            check(value, flags);
        }
    }
}

#[test]
fn null_diagnostics_match_c() {
    if !super::isolated(concat!(module_path!(), "::null_diagnostics_match_c")) {
        return;
    }
    // SAFETY: the isolated child exclusively owns the log handler; callbacks
    // use the base shim's opaque handler-pointer ABI, and the probe restores it.
    unsafe {
        assert!(oracle_output_protocol_getter_logs(
            getter,
            base_get_log_handler,
            base_set_log_handler
        ));
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn arbitrary_metadata_matches_c(mut bytes in prop::collection::vec(any::<u8>(), 0..128), present in any::<bool>(), flags in any::<u32>()) {
        bytes.push(0);
        let before = bytes.clone();
        check(present.then_some(bytes.as_slice()), flags);
        prop_assert_eq!(bytes, before);
    }
}
