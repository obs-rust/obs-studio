//! Real-header layout and differential borrowed pointer parity. No exclusions.
use core::ffi::c_void;
use core::mem::{align_of, offset_of, size_of};
use obs_core::ffi::canvas_video_getter::{Fields, obs_canvas_get_video};
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_canvas_video_layout(layout: *mut usize);
    fn oracle_canvas_video_new(mixed: bool, video: *mut c_void) -> *mut c_void;
    fn oracle_canvas_video_unchanged(canvas: *const c_void) -> bool;
    fn oracle_canvas_video_free(canvas: *mut c_void);
    fn oracle_obs_canvas_get_video(canvas: *const c_void) -> *mut c_void;
}

#[test]
fn real_header_field_view_layout() {
    let mut layout = [0; 5];
    // SAFETY: the oracle writes exactly five size/offset values.
    unsafe { oracle_canvas_video_layout(layout.as_mut_ptr()) };
    assert_eq!(layout, [size_of::<Fields>(), align_of::<Fields>(), offset_of!(Fields, mix), size_of::<*mut c_void>(), align_of::<*mut c_void>()]);
}

fn check(mixed: bool, video: *mut c_void) {
    // SAFETY: C owns a real-layout canvas and mix until free. Neither getter
    // dereferences video, acquires a lock/reference, or mutates the fixture.
    unsafe {
        let canvas = oracle_canvas_video_new(mixed, video);
        assert!(!canvas.is_null());
        assert_eq!(obs_canvas_get_video(canvas), oracle_obs_canvas_get_video(canvas));
        assert_eq!(obs_canvas_get_video(canvas), if mixed { video } else { core::ptr::null_mut() });
        assert!(oracle_canvas_video_unchanged(canvas));
        oracle_canvas_video_free(canvas);
    }
}

#[test]
fn characterized_edges() {
    let mut tokens = [0u8; 2];
    for mixed in [false, true] {
        check(mixed, core::ptr::null_mut());
        check(mixed, (&mut tokens[0] as *mut u8).cast());
        check(mixed, (&mut tokens[1] as *mut u8).cast());
    }
    assert_eq!(tokens, [0, 0]);
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(512), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn borrowed_pointer_matches_c(mixed in any::<bool>(), present in any::<bool>(), offset in 0usize..64) {
        let mut tokens = [0u8; 64];
        let video = if present { (&mut tokens[offset] as *mut u8).cast() } else { core::ptr::null_mut() };
        check(mixed, video);
        prop_assert_eq!(tokens, [0; 64]);
    }
}
