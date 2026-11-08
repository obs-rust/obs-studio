//! Callback-only differential parity. NULL canvas is original C UB, excluded.
use core::ffi::c_void;
use core::mem::{align_of, offset_of, size_of};
use obs_core::ffi::canvas_channel_getter::{Fields, obs_canvas_get_channel};
use proptest::prelude::*;

type Getter = unsafe extern "C" fn(*mut c_void, u32) -> *mut c_void;
unsafe extern "C" {
    fn oracle_canvas_channel_layout(layout: *mut usize);
    fn oracle_canvas_channel_probe(
        channel: u32,
        present: bool,
        expired: bool,
        initial: u32,
        getter: Getter,
    ) -> bool;
}

#[test]
fn real_header_field_view_layout() {
    let mut layout = [0; 8];
    // SAFETY: oracle writes exactly eight size, alignment and offset values.
    unsafe { oracle_canvas_channel_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<Fields>(),
            align_of::<Fields>(),
            offset_of!(Fields, view),
            offset_of!(Fields, get_source),
            size_of::<*mut c_void>(),
            align_of::<*mut c_void>(),
            size_of::<Getter>(),
            align_of::<Getter>()
        ]
    );
}

fn check(channel: u32, present: bool, expired: bool, initial: u32) {
    // SAFETY: C owns live real-layout fixtures, a real mutex and bounded counters;
    // both getters acquire a reference through the same independent dependency.
    assert!(unsafe {
        oracle_canvas_channel_probe(channel, present, expired, initial, obs_canvas_get_channel)
    });
}

#[test]
fn characterized_edges() {
    for channel in [0, 1, 63, 64, u32::MAX] {
        for present in [false, true] {
            for expired in [false, true] {
                for initial in [0, 255] {
                    check(channel, present, expired, initial);
                }
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(512), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn retained_channel_matches_c(channel in prop_oneof![0u32..64, any::<u32>()], present in any::<bool>(), expired in any::<bool>(), initial in 0u32..1024) {
        check(channel, present, expired, initial);
    }
}
