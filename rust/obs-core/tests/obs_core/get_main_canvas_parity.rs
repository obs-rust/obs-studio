//! Original-C wrapper parity; the real reference operation is covered by
//! unchanged actual-library Tier 2. Excludes uninitialized core, dead canvases
//! and overflowing reference counts. No intentional behavior differences.
use core::ffi::{c_long, c_void};
use core::mem::{align_of, offset_of, size_of};
use obs_core::ffi::get_main_canvas::{Fields, obs_get_main_canvas};
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_get_main_canvas_layout(layout: *mut usize);
    fn oracle_get_main_canvas_check(
        present: bool,
        second: bool,
        alternate: bool,
        null_result: bool,
        initial: c_long,
        getter: unsafe extern "C" fn() -> *mut c_void,
    ) -> bool;
}

#[test]
fn real_c_adapter_layout() {
    let mut layout = [0; 4];
    // SAFETY: the C oracle writes exactly four layout measurements.
    unsafe { oracle_get_main_canvas_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<Fields>(),
            align_of::<Fields>(),
            offset_of!(Fields, canvas),
            offset_of!(Fields, get_ref),
        ]
    );
}

fn check(present: bool, second: bool, alternate: bool, null_result: bool, initial: u16) {
    // SAFETY: C owns the real-layout live fixtures and thread-local initialized
    // core. Both wrappers call the same observer; all allocations are bounded.
    assert!(unsafe {
        oracle_get_main_canvas_check(
            present,
            second,
            alternate,
            null_result,
            c_long::from(initial),
            obs_get_main_canvas,
        )
    });
}

#[test]
fn characterized_reference_and_return_edges() {
    for present in [false, true] {
        for second in [false, true] {
            for alternate in [false, true] {
                for null_result in [false, true] {
                    for initial in [0, 1, 255, u16::MAX] {
                        check(present, second, alternate, null_result, initial);
                    }
                }
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(512), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn exact_delegation_matches_original_c(present in any::<bool>(), second in any::<bool>(), alternate in any::<bool>(), null_result in any::<bool>(), initial in any::<u16>()) {
        check(present, second, alternate, null_result, initial);
    }
}
