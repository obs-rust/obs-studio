//! Original-C constructors and real-header private adapter differential.
//! No mirrored structs, intentional differences or excluded names.
use core::ffi::{c_char, c_void};
type Constructor = unsafe extern "C" fn(*const c_char) -> *mut c_void;
unsafe extern "C" {
    fn oracle_scene_create_check(
        name: *const c_char,
        private: bool,
        null_canvas: bool,
        null_data: bool,
        create: Option<Constructor>,
    ) -> bool;
}
#[test]
fn original_c_characterized_edges() {
    for name in [
        None,
        Some(c""),
        Some(c"constructor name"),
        Some(c"\xff\xfe"),
    ] {
        for private in [false, true] {
            for null_canvas in [false, true] {
                for null_data in [false, true] {
                    // SAFETY: the oracle owns complete real-header objects for the call.
                    assert!(unsafe {
                        oracle_scene_create_check(
                            name.map_or(core::ptr::null(), |n| n.as_ptr()),
                            private,
                            null_canvas,
                            null_data,
                            None,
                        )
                    });
                }
            }
        }
    }
}
