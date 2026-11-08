//! Real-header layout, full-width masks, immutable state and exact diagnostics.
//! No intentional differences or input exclusions.
use core::ffi::c_void;
use core::mem::{align_of, offset_of, size_of};
use obs_output::ffi::output_mixer_getters::{
    Fields, obs_output_get_mixer as mixer, obs_output_get_mixers as mixers,
};
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

type Getter = unsafe extern "C" fn(*const c_void) -> usize;
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);
unsafe extern "C" {
    fn oracle_output_mixer_new(mask: usize, flags: u32) -> *mut c_void;
    fn oracle_output_mixer_free(output: *mut c_void);
    fn oracle_output_mixer_unchanged(output: *const c_void) -> bool;
    fn oracle_output_mixer_addresses(output: *const c_void) -> bool;
    fn oracle_output_mixer_layout(layout: *mut usize);
    fn oracle_obs_output_get_mixer(output: *const c_void) -> usize;
    fn oracle_obs_output_get_mixers(output: *const c_void) -> usize;
    fn oracle_output_mixer_getters_logs(
        mixer: Getter,
        mixers: Getter,
        get: GetLog,
        set: SetLog,
    ) -> bool;
}

struct Fixture(*mut c_void);
impl Drop for Fixture {
    fn drop(&mut self) {
        // SAFETY: the fixture owns exactly this C allocation.
        unsafe { oracle_output_mixer_free(self.0) }
    }
}

#[test]
fn real_header_layout() {
    let mut layout = [0; 5];
    // SAFETY: the oracle writes five layout entries.
    unsafe { oracle_output_mixer_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<Fields>(),
            align_of::<Fields>(),
            offset_of!(Fields, mask),
            size_of::<usize>(),
            align_of::<usize>()
        ]
    );
}

fn check(mask: usize, flags: u32) {
    // SAFETY: fixture constructor takes only plain values.
    let fixture = Fixture(unsafe { oracle_output_mixer_new(mask, flags) });
    assert!(!fixture.0.is_null());
    // SAFETY: real-layout fixture is live. Accessors never receive NULL.
    unsafe {
        assert!(oracle_output_mixer_addresses(fixture.0));
        for _ in 0..2 {
            assert_eq!(mixer(fixture.0), oracle_obs_output_get_mixer(fixture.0));
            assert_eq!(mixers(fixture.0), oracle_obs_output_get_mixers(fixture.0));
            assert_eq!(mixers(fixture.0), mask);
            assert!(oracle_output_mixer_unchanged(fixture.0));
        }
    }
}

#[test]
fn characterized_edges() {
    for mask in [0, 1, 2, 3, 32, 64, 65, 128, usize::MAX]
        .into_iter()
        .chain((0..usize::BITS).map(|bit| 1usize << bit))
    {
        for flags in [0, 7, u32::MAX] {
            check(mask, flags);
        }
    }
}

#[test]
fn null_diagnostics_match_c() {
    if !super::isolated(concat!(module_path!(), "::null_diagnostics_match_c")) {
        return;
    }
    // SAFETY: isolated child exclusively owns the log handler. Probe restores
    // it and invokes production symbols through the supplied callbacks only.
    unsafe {
        assert!(oracle_output_mixer_getters_logs(
            mixer,
            mixers,
            base_get_log_handler,
            base_set_log_handler
        ));
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn arbitrary_masks_match_c(mask in any::<usize>(), flags in any::<u32>()) { check(mask, flags); }
}
