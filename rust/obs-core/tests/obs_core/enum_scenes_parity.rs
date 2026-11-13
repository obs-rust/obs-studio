//! Original-C delegation and object-byte parity. The actual downstream canvas
//! traversal is covered by unchanged test_enum_scenes.c on both real libraries.
//! Excludes uninitialized/dead core/canvas, invalid callback/parameter and ref
//! overflow (C UB); there are no intentional behavior differences.
use core::ffi::c_long;
use core::ffi::c_void;
use core::mem::{align_of, offset_of, size_of};
use obs_core::ffi::enum_scenes::{EnumProc, Fields, obs_enum_scenes};
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_enum_scenes_layout(layout: *mut usize);
    fn oracle_enum_scenes_check(
        total: u32,
        stop: u32,
        second_canvas: bool,
        second_callback: bool,
        second_param: bool,
        acquire: bool,
        initial: c_long,
        wrapper: unsafe extern "C" fn(EnumProc, *mut c_void),
    ) -> bool;
}

#[test]
fn real_c_adapter_layout() {
    let mut layout = [0; 4];
    // SAFETY: oracle writes exactly four values into this initialized array.
    unsafe { oracle_enum_scenes_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<Fields>(),
            align_of::<Fields>(),
            offset_of!(Fields, canvas),
            offset_of!(Fields, enumerate)
        ]
    );
}

fn check(total: u32, stop: u32, choices: [bool; 4], initial: u16) {
    // SAFETY: oracle owns live, real-layout fixtures, valid callbacks/parameters
    // and a thread-local initialized core. Total <= 4 bounds every allocation.
    assert!(unsafe {
        oracle_enum_scenes_check(
            total,
            stop,
            choices[0],
            choices[1],
            choices[2],
            choices[3],
            c_long::from(initial),
            obs_enum_scenes,
        )
    });
}

#[test]
fn characterized_delegation_edges() {
    for total in 0..=4 {
        for stop in 0..=5 {
            for bits in 0..16 {
                let choices = [bits & 1 != 0, bits & 2 != 0, bits & 4 != 0, bits & 8 != 0];
                for initial in [0, 1, u16::MAX] {
                    check(total, stop, choices, initial);
                }
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(512), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn exact_forwarding_matches_original_c(total in 0u32..=4, stop in any::<u32>(), choices in any::<[bool;4]>(), initial in any::<u16>()) {
        check(total, stop, choices, initial);
    }
}
