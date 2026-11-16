//! Original-C conversions over complete source, scene and reference controls.
//! No private layouts cross into Rust and no intentional differences apply.
use core::ffi::{c_char, c_void};
use obs_scene::ffi::from_source::{obs_group_from_source, obs_scene_from_source};
use proptest::prelude::*;
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

fn check(id: &[u8], bits: u32) {
    let id = CString::new(id).unwrap();
    // SAFETY: C constructs complete live objects and calls these getters
    // synchronously, checking pointers, repeated reads and every object byte.
    assert_eq!(
        unsafe {
            oracle_scene_from_source_probe(
                id.as_ptr(),
                bits,
                Some(obs_scene_from_source),
                Some(obs_group_from_source),
            )
        },
        0,
        "id={id:?}, bits={bits}"
    );
}

#[test]
fn characterized_edges_match_c() {
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
        for bits in 0..8 {
            check(id, bits);
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(128), failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn identifiers_and_borrowed_context_match_c(
        id in prop_oneof![Just(b"scene".to_vec()), Just(b"group".to_vec()), prop::collection::vec(1_u8..=255, 0..65)],
        bits in any::<u32>(),
    ) {
        check(&id, bits);
    }
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
