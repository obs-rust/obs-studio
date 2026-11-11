//! Original-C conversions over complete source, scene and reference controls.
//! No private layouts cross into Rust and no intentional differences apply.
use core::ffi::{c_char, c_void};
use std::ffi::CString;

type Getter = unsafe extern "C" fn(*const c_void) -> *mut c_void;
unsafe extern "C" {
    fn oracle_scene_from_source_probe(
        id: *const c_char,
        bits: u32,
        scene: Option<Getter>,
        group: Option<Getter>,
    ) -> u32;
}

#[test]
fn original_c_characterization() {
    for id in [
        b"scene".as_slice(),
        b"group",
        b"scene-extra",
        b"group-extra",
        b"Scene",
        b"Group",
        b"",
        b"\xff",
    ] {
        let id = CString::new(id).unwrap();
        for bits in 0..8 {
            // SAFETY: C constructs live complete fixtures, uses the original
            // getters for absent callbacks and retains no pointers afterwards.
            assert_eq!(
                unsafe { oracle_scene_from_source_probe(id.as_ptr(), bits, None, None) },
                0
            );
        }
    }
}
