//! Tier 3: original C query, actual private layout, no behavior exclusions (#3895).
use obs_output::ffi::output_can_pause::obs_output_can_pause;
use obs_output::output_can_pause::OBS_OUTPUT_CAN_PAUSE;
use proptest::prelude::*;
use std::ffi::c_void;
use std::mem::{align_of, size_of};

unsafe extern "C" {
    fn oracle_output_can_pause(output: *const c_void) -> bool;
    fn oracle_output_pause_new(flags: u32) -> *mut c_void;
    fn oracle_output_pause_unchanged(output: *const c_void) -> bool;
    fn oracle_output_pause_free(output: *mut c_void);
    fn oracle_output_pause_layout(values: *mut usize);
    fn oracle_output_pause_field_address(output: *const c_void) -> bool;
    fn oracle_output_pause_logs(rust: bool) -> bool;
}

struct Fixture(*mut c_void);
impl Drop for Fixture {
    fn drop(&mut self) {
        // SAFETY: this owns the C fixture allocated by oracle_output_pause_new.
        unsafe { oracle_output_pause_free(self.0) }
    }
}

fn compare(flags: u32) {
    // SAFETY: the fixture allocator accepts arbitrary flag words.
    let fixture = Fixture(unsafe { oracle_output_pause_new(flags) });
    assert!(!fixture.0.is_null());
    // SAFETY: the fixture has the actual output layout and remains live, exclusively used.
    unsafe {
        assert!(oracle_output_pause_field_address(fixture.0));
        let c = oracle_output_can_pause(fixture.0);
        assert!(oracle_output_pause_unchanged(fixture.0));
        assert_eq!(obs_output_can_pause(fixture.0), c, "flags={flags:#x}");
        assert!(oracle_output_pause_unchanged(fixture.0));
    }
}

#[test]
fn real_header_flag_layout() {
    let mut values = [0; 3];
    // SAFETY: the oracle writes exactly three size_t values.
    unsafe { oracle_output_pause_layout(values.as_mut_ptr()) }
    assert_eq!(
        values,
        [
            size_of::<u32>(),
            align_of::<u32>(),
            OBS_OUTPUT_CAN_PAUSE as usize
        ]
    );
}

#[test]
fn characterized_edges() {
    for flags in [
        0,
        OBS_OUTPUT_CAN_PAUSE,
        u32::MAX,
        u32::MAX ^ OBS_OUTPUT_CAN_PAUSE,
        7,
    ] {
        compare(flags);
    }
    for bit in 0..32 {
        compare(1 << bit);
        compare((1 << bit) | OBS_OUTPUT_CAN_PAUSE);
    }
}

#[test]
fn null_logging_matches_c() {
    if !super::isolated(concat!(module_path!(), "::null_logging_matches_c")) {
        return;
    }
    // SAFETY: this isolated child owns the global log handler; each probe restores it.
    unsafe {
        assert!(oracle_output_pause_logs(false));
        assert!(oracle_output_pause_logs(true));
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn random_flags_match_c(flags in any::<u32>()) {
        compare(flags);
    }
}
