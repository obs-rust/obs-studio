//! Exact extracted-C setter parity. No differences or excluded defined inputs.
use core::ffi::c_void;
use obs_source::ffi::source_audio_active_setter::obs_source_set_audio_active;
use proptest::prelude::*;

type Setter = unsafe extern "C" fn(*mut c_void, bool);
unsafe extern "C" {
    fn oracle_obs_source_set_audio_active(source: *mut c_void, active: bool);
    fn oracle_source_audio_active_setter_new(bits: u32) -> *mut c_void;
    fn oracle_source_audio_active_setter_free(source: *mut c_void);
    fn oracle_source_audio_active_setter_step(
        source: *mut c_void,
        active: bool,
        setter: Setter,
    ) -> u32;
    fn oracle_source_audio_active_setter_null_log(setter: Setter) -> bool;
}
struct Source(*mut c_void);
impl Source {
    fn new(bits: u32) -> Self {
        // SAFETY: allocates a complete real-header C source fixture.
        let source = Self(unsafe { oracle_source_audio_active_setter_new(bits) });
        assert!(!source.0.is_null());
        source
    }
}
impl Drop for Source {
    fn drop(&mut self) {
        // SAFETY: exclusively owned C allocation, freed once by its allocator.
        unsafe { oracle_source_audio_active_setter_free(self.0) };
    }
}
fn check(bits: u32, sequence: &[bool]) {
    let c = Source::new(bits);
    let rust = Source::new(bits);
    for &active in sequence {
        // SAFETY: both setters receive distinct, live initialized sources; the
        // recording seam reads the real field during synchronous notification.
        unsafe {
            assert_eq!(
                oracle_source_audio_active_setter_step(
                    c.0,
                    active,
                    oracle_obs_source_set_audio_active
                ),
                oracle_source_audio_active_setter_step(rust.0, active, obs_source_set_audio_active),
                "bits={bits}, active={active}"
            );
        }
    }
}
#[test]
fn characterized_edges_and_repeats() {
    for bits in 0..4 {
        for active in [false, true] {
            check(bits, &[active, active, !active, !active]);
        }
    }
}
#[test]
fn null_diagnostic_matches_c() {
    if std::env::var_os("OBS_SET_AUDIO_ACTIVE_CHILD").is_none() {
        let selector = concat!(module_path!(), "::null_diagnostic_matches_c")
            .split_once("::")
            .unwrap()
            .1;
        assert!(
            std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", selector, "--nocapture"])
                .env("OBS_SET_AUDIO_ACTIVE_CHILD", "1")
                .status()
                .unwrap()
                .success()
        );
        return;
    }
    // SAFETY: isolated process changes and restores its logger; both callbacks accept NULL.
    unsafe {
        assert!(oracle_source_audio_active_setter_null_log(
            oracle_obs_source_set_audio_active
        ));
        assert!(oracle_source_audio_active_setter_null_log(
            obs_source_set_audio_active
        ));
    }
}
proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(64),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn random_sequences_match_c(bits in 0u32..4, sequence in prop::collection::vec(any::<bool>(), 0..65)) {
        check(bits, &sequence);
    }
}
