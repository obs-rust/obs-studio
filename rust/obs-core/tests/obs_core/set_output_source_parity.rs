//! Original-C callback parity; real setter effects are replayed by unchanged
//! test_set_output_source.c against both actual libraries. UB exclusions: no
//! uninitialized core, dead main canvas or dead source. No behavior differences.
use core::ffi::{c_long, c_void};
use core::mem::{align_of, offset_of, size_of};
use obs_core::ffi::set_output_source::{Fields, obs_set_output_source};
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_set_output_source_layout(layout: *mut usize);
    fn oracle_set_output_source_check(
        channel: u32,
        present: bool,
        second_canvas: bool,
        initial: c_long,
        setter: unsafe extern "C" fn(u32, *mut c_void),
    ) -> bool;
}

#[test]
fn real_c_adapter_layout() {
    let mut layout = [0; 4];
    // SAFETY: oracle writes exactly four layout values into this array.
    unsafe { oracle_set_output_source_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<Fields>(),
            align_of::<Fields>(),
            offset_of!(Fields, canvas),
            offset_of!(Fields, set_channel)
        ]
    );
}

fn check(channel: u32, present: bool, second_canvas: bool, initial: u16) {
    // SAFETY: C owns live real-layout fixtures and a thread-local initialized
    // core. Both wrappers invoke the same observer, never libobs-global stubs.
    assert!(unsafe {
        oracle_set_output_source_check(
            channel,
            present,
            second_canvas,
            c_long::from(initial),
            obs_set_output_source,
        )
    });
}

#[test]
fn characterized_delegation_edges() {
    for channel in [0, 2, 3, 63, 64, 65, u32::MAX] {
        for present in [false, true] {
            for second_canvas in [false, true] {
                for initial in [0, 1, 255, u16::MAX] {
                    check(channel, present, second_canvas, initial);
                }
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(512), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn exact_forwarding_matches_original_c(channel in any::<u32>(), present in any::<bool>(), second_canvas in any::<bool>(), initial in any::<u16>()) {
        check(channel, present, second_canvas, initial);
    }
}
