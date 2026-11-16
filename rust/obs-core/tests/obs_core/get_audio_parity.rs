//! Original-C getter parity, including current-field changes and no writes.
//! Excludes uninitialized/dead core and concurrent reset, matching C's contract.
//! No intentional differences. Actual audio lifecycles remain unchanged Tier 2.
use core::ffi::c_void;
use core::mem::{align_of, offset_of, size_of};
use obs_core::ffi::get_audio::{Fields, obs_get_audio};
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_get_audio_layout(layout: *mut usize);
    fn oracle_get_audio_check(
        present: bool,
        second: bool,
        fill: u8,
        getter: unsafe extern "C" fn() -> *mut c_void,
    ) -> bool;
}

#[test]
fn real_c_adapter_layout() {
    let mut layout = [0; 3];
    // SAFETY: C writes exactly three layout values into this array.
    unsafe { oracle_get_audio_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<Fields>(),
            align_of::<Fields>(),
            offset_of!(Fields, audio)
        ]
    );
}

fn check(present: bool, second: bool, fill: u8) {
    // SAFETY: C owns live real-layout core fixtures and aligned opaque tokens,
    // installs a thread-local initialized core and never dereferences the audio.
    assert!(unsafe { oracle_get_audio_check(present, second, fill, obs_get_audio) });
}

#[test]
fn characterized_pointer_and_state_edges() {
    for present in [false, true] {
        for second in [false, true] {
            for fill in [0, 1, 127, 128, 254, 255] {
                check(present, second, fill);
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn current_borrowed_audio_matches_original_c(present in any::<bool>(), second in any::<bool>(), fill in any::<u8>()) {
        check(present, second, fill);
    }
}
