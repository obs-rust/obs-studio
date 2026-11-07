//! Real-header layout and differential checks for #3860; no exclusions.
use core::ffi::{c_char, c_void};
use core::mem::{align_of, offset_of, size_of};
use obs_core::ffi::canvas_readonly_getters::*;
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_canvas_readonly_layout(layout: *mut usize);
    fn oracle_canvas_readonly_new(
        name: *mut c_char,
        uuid: *mut c_char,
        signals: *mut c_void,
        mixed: bool,
    ) -> *mut c_void;
    fn oracle_canvas_readonly_unchanged(canvas: *const c_void) -> bool;
    fn oracle_canvas_readonly_free(canvas: *mut c_void);
    fn oracle_obs_canvas_get_name(canvas: *const c_void) -> *const c_char;
    fn oracle_obs_canvas_get_uuid(canvas: *const c_void) -> *const c_char;
    fn oracle_obs_canvas_get_signal_handler(canvas: *mut c_void) -> *mut c_void;
    fn oracle_obs_canvas_has_video(canvas: *mut c_void) -> bool;
}

#[test]
fn real_header_field_view_layout() {
    let mut layout = [0; 6];
    // SAFETY: C writes exactly six layout values to this array.
    unsafe { oracle_canvas_readonly_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<Fields>(),
            align_of::<Fields>(),
            offset_of!(Fields, name),
            offset_of!(Fields, uuid),
            offset_of!(Fields, signals),
            offset_of!(Fields, mix)
        ]
    );
}

#[repr(align(16))]
struct SignalToken(u8);

fn check(mask: u8, mut name: Vec<u8>, mut uuid: Vec<u8>) {
    name.push(0);
    uuid.push(0);
    let name = if mask & 1 != 0 {
        name.as_mut_ptr().cast()
    } else {
        core::ptr::null_mut()
    };
    let uuid = if mask & 2 != 0 {
        uuid.as_mut_ptr().cast()
    } else {
        core::ptr::null_mut()
    };
    let mut signal = SignalToken(0);
    let signals = if mask & 4 != 0 {
        (&raw mut signal.0).cast()
    } else {
        core::ptr::null_mut()
    };
    // SAFETY: buffers and aligned token stay live, and C owns the real-layout
    // fixture until free. Both implementations only read borrowed pointer fields.
    unsafe {
        let canvas = oracle_canvas_readonly_new(name, uuid, signals, mask & 8 != 0);
        assert!(!canvas.is_null());
        assert_eq!(
            obs_canvas_get_name(canvas),
            oracle_obs_canvas_get_name(canvas)
        );
        assert_eq!(obs_canvas_get_name(canvas), name.cast_const());
        assert_eq!(
            obs_canvas_get_uuid(canvas),
            oracle_obs_canvas_get_uuid(canvas)
        );
        assert_eq!(obs_canvas_get_uuid(canvas), uuid.cast_const());
        assert_eq!(
            obs_canvas_get_signal_handler(canvas),
            oracle_obs_canvas_get_signal_handler(canvas)
        );
        assert_eq!(obs_canvas_get_signal_handler(canvas), signals);
        assert_eq!(
            obs_canvas_has_video(canvas),
            oracle_obs_canvas_has_video(canvas)
        );
        assert_eq!(obs_canvas_has_video(canvas), mask & 8 != 0);
        assert!(oracle_canvas_readonly_unchanged(canvas));
        oracle_canvas_readonly_free(canvas);
    }
}

#[test]
fn characterized_edges() {
    for mask in 0..16 {
        check(
            mask,
            b"canvas name".to_vec(),
            b"12345678-1234-1234-1234-123456789abc".to_vec(),
        );
        check(mask, vec![], vec![0xff, 0, 0x80]);
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(512), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn random_fields_match_c(mask in any::<u8>(), name in prop::collection::vec(any::<u8>(), 0..128), uuid in prop::collection::vec(any::<u8>(), 0..128)) {
        check(mask, name, uuid);
    }
}
