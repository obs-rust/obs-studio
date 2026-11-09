//! Tier 3: unchanged C wrappers and real private layouts, no defined-input exclusions.
use core::ffi::{c_char, c_void};
use obs_core::ffi::canvas_scene::{obs_canvas_scene_create, obs_canvas_scene_remove};
use proptest::prelude::*;
use std::ffi::CString;

unsafe extern "C" {
    fn oracle_canvas_scene_check(
        name: *const c_char,
        null_canvas: bool,
        null_data: bool,
        alternate: bool,
        repeats: u32,
        create: unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_void,
        remove: unsafe extern "C" fn(*mut c_void),
    ) -> bool;
}

fn check(
    name: Option<&CString>,
    null_canvas: bool,
    null_data: bool,
    alternate: bool,
    repeats: u32,
) {
    // SAFETY: the C fixture owns all real-layout objects until both calls finish;
    // name is null or a live C string. Neither callback retains it beyond the call.
    assert!(unsafe {
        oracle_canvas_scene_check(
            name.map_or(core::ptr::null(), |n| n.as_ptr()),
            null_canvas,
            null_data,
            alternate,
            repeats,
            obs_canvas_scene_create,
            obs_canvas_scene_remove,
        )
    });
}

#[test]
fn characterized_edges() {
    for name in [
        None,
        Some(CString::new("").unwrap()),
        Some(CString::new("char moved scene").unwrap()),
        Some(CString::new("char removed scene").unwrap()),
        Some(CString::new("\u{1f3a5}").unwrap()),
    ] {
        for null_canvas in [false, true] {
            for null_data in [false, true] {
                for alternate in [false, true] {
                    for repeats in [0, 1, 2, 8] {
                        check(name.as_ref(), null_canvas, null_data, alternate, repeats);
                    }
                }
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(128), .. ProptestConfig::default() })]
    #[test]
    fn lifecycle_matches_c(bytes in proptest::collection::vec(1u8..=255, 0..128),
                           null_canvas in any::<bool>(), null_data in any::<bool>(),
                           alternate in any::<bool>(), repeats in 0u32..16) {
        check(Some(&CString::new(bytes).unwrap()), null_canvas, null_data, alternate, repeats);
    }
}
