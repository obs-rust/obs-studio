//! Exact parity on all counter values and NULL handles. No intended differences.
//! Fixtures use the real C encoder layout; Rust sees only borrowed field pointers.

use core::ffi::c_void;
use core::mem::{align_of, offset_of, size_of};
use obs_output::ffi::encoder_counters::{self as rust, CounterFields};
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_encoder_counter_layout(layout: *mut usize);
    fn oracle_encoder_counter_new(frames: u32, offset: u64) -> *mut c_void;
    fn oracle_encoder_counter_free(encoder: *mut c_void);
    fn oracle_encoder_counter_unchanged(encoder: *const c_void, frames: u32, offset: u64) -> bool;
    fn oracle_obs_encoder_get_encoded_frames(encoder: *const c_void) -> u32;
    fn oracle_obs_encoder_get_pause_offset(encoder: *const c_void) -> u64;
    fn oracle_encoder_counter_null_log(rust: bool) -> bool;
}

#[test]
fn real_header_view_layout() {
    let mut c = [0; 4];
    // SAFETY: the oracle writes exactly four size_t values.
    unsafe { oracle_encoder_counter_layout(c.as_mut_ptr()) };
    assert_eq!(
        c,
        [
            size_of::<CounterFields>(),
            align_of::<CounterFields>(),
            offset_of!(CounterFields, encoded_frames),
            offset_of!(CounterFields, pause_offset)
        ]
    );
}

struct Encoder(*mut c_void);

impl Drop for Encoder {
    fn drop(&mut self) {
        // SAFETY: this fixture is allocated by the oracle and freed exactly once.
        unsafe { oracle_encoder_counter_free(self.0) };
    }
}

fn check(frames: u32, offset: u64) {
    // SAFETY: the oracle creates a private encoder fixture with readable counters.
    let encoder = Encoder(unsafe { oracle_encoder_counter_new(frames, offset) });
    assert!(!encoder.0.is_null());
    // SAFETY: the C fixture remains live and exclusive until the end of this scope.
    unsafe {
        assert_eq!(
            rust::obs_encoder_get_encoded_frames(encoder.0),
            oracle_obs_encoder_get_encoded_frames(encoder.0)
        );
        assert_eq!(
            rust::obs_encoder_get_pause_offset(encoder.0),
            oracle_obs_encoder_get_pause_offset(encoder.0)
        );
        assert!(oracle_encoder_counter_unchanged(encoder.0, frames, offset));
    }
}

#[test]
fn characterized_edges() {
    for frames in [0, 1, 0x8000_0000, u32::MAX] {
        for offset in [0, 1, 0x8000_0000_0000_0000, u64::MAX] {
            check(frames, offset);
        }
    }
}

#[test]
fn null_defaults_and_historical_log() {
    if !super::isolated(concat!(
        module_path!(),
        "::null_defaults_and_historical_log"
    )) {
        return;
    }
    // SAFETY: each probe saves/restores the global logger, isolated in this child.
    unsafe {
        assert!(oracle_encoder_counter_null_log(false));
        assert!(oracle_encoder_counter_null_log(true));
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn random_counters_match_c(frames in any::<u32>(), offset in any::<u64>()) {
        check(frames, offset);
    }
}
