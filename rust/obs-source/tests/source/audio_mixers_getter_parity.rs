//! Tier 3 against the original C getter/predicate; no excluded inputs.
use core::ffi::c_void;
use core::mem::{align_of, size_of};
use obs_source::ffi::source_audio_mixers_getter::obs_source_get_audio_mixers as getter;
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

type Getter = unsafe extern "C" fn(*const c_void) -> u32;
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);
unsafe extern "C" {
    fn oracle_source_audio_mixers_new(owned: bool, flags: u32, mixers: u32) -> *mut c_void;
    fn oracle_source_audio_mixers_free(source: *mut c_void);
    fn oracle_source_audio_mixers_unchanged(source: *const c_void) -> bool;
    fn oracle_source_audio_mixers_fields(source: *const c_void) -> bool;
    fn oracle_source_audio_mixers_layout(layout: *mut usize);
    fn oracle_obs_source_get_audio_mixers(source: *const c_void) -> u32;
    fn oracle_source_audio_mixers_logs(getter: Getter, get_log: GetLog, set_log: SetLog) -> bool;
}

struct Fixture(*mut c_void);
impl Drop for Fixture {
    fn drop(&mut self) {
        // SAFETY: owns this fixture's sole allocation, with no nested owned state.
        unsafe { oracle_source_audio_mixers_free(self.0) }
    }
}

fn check(owned: bool, flags: u32, mixers: u32) {
    // SAFETY: oracle allocates and initializes a real-layout source fixture.
    let fixture = Fixture(unsafe { oracle_source_audio_mixers_new(owned, flags, mixers) });
    assert!(!fixture.0.is_null());
    // SAFETY: source is live, non-NULL and readable until the fixture is dropped.
    unsafe {
        assert!(oracle_source_audio_mixers_fields(fixture.0));
        for _ in 0..2 {
            assert_eq!(
                getter(fixture.0),
                oracle_obs_source_get_audio_mixers(fixture.0)
            );
            assert!(oracle_source_audio_mixers_unchanged(fixture.0));
        }
    }
}

#[test]
fn scalar_abi_matches_real_c_header() {
    let mut layout = [0; 5];
    // SAFETY: oracle writes exactly five entries into the supplied buffer.
    unsafe { oracle_source_audio_mixers_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<bool>(),
            align_of::<bool>(),
            size_of::<u32>(),
            align_of::<u32>(),
            obs_source::monitoring::AUDIO as usize
        ]
    );
}

#[test]
fn characterized_masks_and_every_flag_bit_match_c() {
    for owned in [false, true] {
        for flags in [0, 1, 2, 3, 4, 5, 6, 1 << 6, u32::MAX] {
            for mask in [0, 1, 0xFF, 0x3F, 0x8000_0000, u32::MAX, 0xA5A5_5A5A] {
                check(owned, flags, mask);
            }
        }
        for bit in 0..32 {
            check(owned, 1 << bit, u32::MAX);
            check(owned, u32::MAX, 1 << bit);
        }
    }
}

#[test]
fn null_and_silent_gate_diagnostics_match_c() {
    if !crate::output::isolated(concat!(
        module_path!(),
        "::null_and_silent_gate_diagnostics_match_c"
    )) {
        return;
    }
    // SAFETY: the isolated child owns logging and the oracle restores the handler.
    // NULL reaches only the public getters, never a private field accessor.
    unsafe {
        assert!(oracle_source_audio_mixers_logs(
            getter,
            base_get_log_handler,
            base_set_log_handler
        ));
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn arbitrary_source_fields_match_c(owned in any::<bool>(), flags in any::<u32>(), mixers in any::<u32>()) {
        check(owned, flags, mixers);
    }
}
