//! Original-C callback parity; unchanged test_get_output_source.c separately
//! replays real getter ownership/bounds on both actual libraries. UB exclusions:
//! uninitialized core, dead main canvas/source and overflowing refcounts.
//! No behavior differences. Full-width channels are safe for the oracle's
//! observer; real downstream debug bounds assertions remain Tier 2's contract.
use core::ffi::{c_long, c_void};
use core::mem::{align_of, offset_of, size_of};
use obs_core::ffi::get_output_source::{Fields, obs_get_output_source};
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_get_output_source_layout(layout: *mut usize);
    fn oracle_get_output_source_check(
        channel: u32,
        present: bool,
        second_canvas: bool,
        second_source: bool,
        initial: c_long,
        getter: unsafe extern "C" fn(u32) -> *mut c_void,
    ) -> bool;
}

#[test]
fn real_c_adapter_layout() {
    let mut layout = [0; 4];
    // SAFETY: the oracle writes exactly four layout values into this array.
    unsafe { oracle_get_output_source_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<Fields>(),
            align_of::<Fields>(),
            offset_of!(Fields, canvas),
            offset_of!(Fields, get_channel),
        ]
    );
}

fn check(channel: u32, present: bool, second_canvas: bool, second_source: bool, initial: u16) {
    // SAFETY: C owns live real-layout fixtures and a thread-local initialized
    // core. Both wrappers call the same observer and keep all refcounts bounded.
    assert!(unsafe {
        oracle_get_output_source_check(
            channel,
            present,
            second_canvas,
            second_source,
            c_long::from(initial),
            obs_get_output_source,
        )
    });
}

#[test]
fn characterized_delegation_edges() {
    for channel in [0, 2, 3, 63, 64, 65, u32::MAX] {
        for present in [false, true] {
            for second_canvas in [false, true] {
                for second_source in [false, true] {
                    for initial in [0, 1, 255, u16::MAX] {
                        check(channel, present, second_canvas, second_source, initial);
                    }
                }
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(512), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn exact_forwarding_and_return_match_original_c(channel in any::<u32>(), present in any::<bool>(), second_canvas in any::<bool>(), second_source in any::<bool>(), initial in any::<u16>()) {
        check(channel, present, second_canvas, second_source, initial);
    }
}
