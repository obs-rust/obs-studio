//! Tier 3: original extracted C vs the production Rust ABI, with real layouts.
//! No defined-input exclusions; null scene/destination are Rust-only safe extensions.
use core::ffi::c_void;
use obs_core::ffi::canvas_move::obs_canvas_move_scene;
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_canvas_move_check(
        groups: u32,
        length: u32,
        reverse: bool,
        same: bool,
        flags: u32,
        ported: unsafe extern "C" fn(*mut c_void, *mut c_void),
    ) -> bool;
}

fn check(groups: u32, length: u32, reverse: bool, same: bool, flags: u32) {
    // SAFETY: the C fixture owns real scenes/items/sources/canvases for both
    // synchronous calls and compares complete object state and ordered calls.
    assert!(unsafe {
        oracle_canvas_move_check(groups, length, reverse, same, flags, obs_canvas_move_scene)
    });
}

#[test]
fn characterized_edges() {
    for length in [0, 1, 3, 8] {
        for groups in [0, 1, 0b101, 0xff] {
            for reverse in [false, true] {
                for same in [false, true] {
                    for flags in 0..4 {
                        check(groups, length, reverse, same, flags);
                    }
                }
            }
        }
    }
}

#[test]
fn null_arguments_do_not_reach_field_accessors() {
    // SAFETY: the shim explicitly accepts null as a no-op before any accessor.
    // Dangling opposite arguments verify no field of either object is read.
    unsafe {
        obs_canvas_move_scene(core::ptr::null_mut(), core::ptr::dangling_mut());
        obs_canvas_move_scene(core::ptr::dangling_mut(), core::ptr::null_mut());
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(128), .. ProptestConfig::default() })]
    #[test]
    fn migration_matches_c(groups in 0u32..256, length in 0u32..=8,
                           reverse in any::<bool>(), same in any::<bool>(), flags in 0u32..4) {
        check(groups, length, reverse, same, flags);
    }
}
