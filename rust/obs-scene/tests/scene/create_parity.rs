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

fn check(name: Option<&std::ffi::CStr>, private: bool, null_canvas: bool, null_data: bool) {
    use obs_scene::ffi::create::{obs_scene_create, obs_scene_create_private};
    let constructor = if private {
        obs_scene_create_private
    } else {
        obs_scene_create
    };
    // SAFETY: C owns full objects for both calls; the name is borrowed for this call.
    assert!(unsafe {
        oracle_scene_create_check(
            name.map_or(core::ptr::null(), |n| n.as_ptr()),
            private,
            null_canvas,
            null_data,
            Some(constructor),
        )
    });
}
#[test]
fn constructors_match_original_c() {
    for name in [
        None,
        Some(c""),
        Some(c"constructor name"),
        Some(c"\xff\xfe"),
    ] {
        for private in [false, true] {
            for null_canvas in [false, true] {
                for null_data in [false, true] {
                    check(name, private, null_canvas, null_data);
                }
            }
        }
    }
}
use proptest::prelude::*;
proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(128), failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn arbitrary_byte_names_match_c(bytes in prop::collection::vec(1u8..=255, 0..128), private in any::<bool>(), absent in any::<bool>(), null_canvas in any::<bool>(), null_data in any::<bool>()) {
        let name = std::ffi::CString::new(bytes).unwrap();
        check(if absent { None } else { Some(&name) }, private, null_canvas, null_data);
    }
}
