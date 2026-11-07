//! Real-header layout and differential borrowed pointer parity. No exclusions.
use core::ffi::c_void;
use core::mem::{align_of, offset_of, size_of};
use obs_core::ffi::canvas_video_getter::{Fields, obs_canvas_get_video};
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_canvas_video_layout(layout: *mut usize);
    fn oracle_canvas_video_new(mixed: bool, present: bool, value: u8) -> *mut c_void;
    fn oracle_canvas_video_token(canvas: *const c_void) -> *mut c_void;
    fn oracle_canvas_video_unchanged(canvas: *const c_void) -> bool;
    fn oracle_canvas_video_free(canvas: *mut c_void);
    fn oracle_obs_canvas_get_video(canvas: *const c_void) -> *mut c_void;
}

#[test]
fn real_header_field_view_layout() {
    let mut layout = [0; 5];
    // SAFETY: the oracle writes exactly five size/offset values.
    unsafe { oracle_canvas_video_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<Fields>(),
            align_of::<Fields>(),
            offset_of!(Fields, mix),
            size_of::<*mut c_void>(),
            align_of::<*mut c_void>()
        ]
    );
}

fn check(mixed: bool, present: bool, value: u8) {
    // SAFETY: C owns a real-layout canvas and mix until free. Neither getter
    // dereferences video, acquires a lock/reference, or mutates the fixture.
    unsafe {
        let canvas = oracle_canvas_video_new(mixed, present, value);
        assert!(!canvas.is_null());
        let video = oracle_canvas_video_token(canvas);
        assert_eq!(!video.is_null(), present);
        assert_eq!(
            obs_canvas_get_video(canvas),
            oracle_obs_canvas_get_video(canvas)
        );
        assert_eq!(
            obs_canvas_get_video(canvas),
            if mixed { video } else { core::ptr::null_mut() }
        );
        assert!(oracle_canvas_video_unchanged(canvas));
        oracle_canvas_video_free(canvas);
    }
}

#[test]
fn characterized_edges() {
    for mixed in [false, true] {
        for present in [false, true] {
            for value in [0, u8::MAX] {
                check(mixed, present, value);
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(512), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn borrowed_pointer_matches_c(mixed in any::<bool>(), present in any::<bool>(), value in any::<u8>()) {
        check(mixed, present, value);
    }
}
