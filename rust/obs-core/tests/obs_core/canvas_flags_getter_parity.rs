//! Real-header layout and exact flag/removal parity for #3940 and #4079.
//! No exclusions or intentional differences from C.
use core::ffi::c_void;
use core::mem::{align_of, offset_of, size_of};
use obs_core::ffi::canvas_flags_getter::{Fields, obs_canvas_get_flags, obs_canvas_removed};
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_canvas_flags_layout(layout: *mut usize);
    fn oracle_canvas_flags_new(flags: u32) -> *mut c_void;
    fn oracle_canvas_flags_unchanged(canvas: *const c_void) -> bool;
    fn oracle_canvas_flags_free(canvas: *mut c_void);
    fn oracle_obs_canvas_get_flags(canvas: *const c_void) -> u32;
    fn oracle_obs_canvas_removed(canvas: *mut c_void) -> bool;
}

#[test]
fn real_header_field_view_layout() {
    let mut layout = [0; 5];
    // SAFETY: the oracle writes exactly five size/offset values.
    unsafe { oracle_canvas_flags_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<Fields>(),
            align_of::<Fields>(),
            offset_of!(Fields, flags),
            size_of::<u32>(),
            align_of::<u32>()
        ]
    );
}

fn check(flags: u32) {
    // SAFETY: C owns a real-layout canvas until free; both getters only read it.
    unsafe {
        let canvas = oracle_canvas_flags_new(flags);
        assert!(!canvas.is_null());
        assert_eq!(
            obs_canvas_get_flags(canvas),
            oracle_obs_canvas_get_flags(canvas)
        );
        assert_eq!(obs_canvas_get_flags(canvas), flags);
        assert_eq!(
            obs_canvas_removed(canvas),
            oracle_obs_canvas_removed(canvas)
        );
        assert_eq!(obs_canvas_removed(canvas), flags & 0x8000_0000 != 0);
        assert!(oracle_canvas_flags_unchanged(canvas));
        oracle_canvas_flags_free(canvas);
    }
}

#[test]
fn characterized_edges_and_every_bit() {
    for flags in [
        0,
        1,
        2,
        4,
        8,
        16,
        32,
        64,
        128,
        0x8000_0000,
        0xa5a5_a5a5,
        u32::MAX,
    ] {
        check(flags);
    }
    for bit in 0..32 {
        check(1u32 << bit);
        check(!(1u32 << bit));
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(512), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn every_uint32_matches_c(flags in any::<u32>()) {
        check(flags);
    }
}
