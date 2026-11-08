//! Exact parity, including raw mode bits and historical NULL logs. No exclusions.
use core::ffi::c_void;
use core::mem::{align_of, offset_of, size_of};
use obs_source::ffi::source_deinterlace_getters::{self as rust, DeinterlaceFields};
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_source_deinterlace_layout(layout: *mut usize);
    fn oracle_source_deinterlace_new(mode: u32, top: bool) -> *mut c_void;
    fn oracle_source_deinterlace_free(source: *mut c_void);
    fn oracle_source_deinterlace_unchanged(source: *const c_void) -> bool;
    fn oracle_obs_source_get_deinterlace_mode(source: *const c_void) -> u32;
    fn oracle_obs_source_get_deinterlace_field_order(source: *const c_void) -> u32;
    fn oracle_source_deinterlace_null_log(
        mode: unsafe extern "C" fn(*const c_void) -> u32,
        order: unsafe extern "C" fn(*const c_void) -> u32,
    ) -> bool;
}

#[test]
fn real_header_view_layout() {
    let mut c = [0; 8];
    // SAFETY: the oracle writes eight size_t values into the array.
    unsafe { oracle_source_deinterlace_layout(c.as_mut_ptr()) };
    assert_eq!(
        c,
        [
            size_of::<DeinterlaceFields>(),
            align_of::<DeinterlaceFields>(),
            offset_of!(DeinterlaceFields, mode),
            offset_of!(DeinterlaceFields, top_first),
            size_of::<u32>(),
            align_of::<u32>(),
            size_of::<u32>(),
            align_of::<u32>(),
        ]
    );
}

struct Source(*mut c_void);
impl Drop for Source {
    fn drop(&mut self) {
        // SAFETY: the C fixture is allocated once and freed exactly once.
        unsafe { oracle_source_deinterlace_free(self.0) };
    }
}

fn check(mode: u32, top: bool) {
    // SAFETY: the oracle allocates the complete real C source layout.
    let source = Source(unsafe { oracle_source_deinterlace_new(mode, top) });
    assert!(!source.0.is_null());
    // SAFETY: the fixture remains live and exclusively owned during both reads.
    unsafe {
        assert_eq!(
            rust::obs_source_get_deinterlace_mode(source.0),
            oracle_obs_source_get_deinterlace_mode(source.0)
        );
        assert!(oracle_source_deinterlace_unchanged(source.0));
        assert_eq!(
            rust::obs_source_get_deinterlace_field_order(source.0),
            oracle_obs_source_get_deinterlace_field_order(source.0)
        );
        assert!(oracle_source_deinterlace_unchanged(source.0));
    }
}

#[test]
fn characterized_edges() {
    for mode in (0..=9).chain([0x7fff_ffff, 0x8000_0000, u32::MAX]) {
        for top in [false, true] {
            check(mode, top);
        }
    }
}

#[test]
fn null_defaults_and_historical_logs() {
    // The logger is global: isolate in the existing Core state executable.
    if std::env::var_os("OBS_DEINTERLACE_GETTERS_CHILD").is_none() {
        let selector = concat!(module_path!(), "::null_defaults_and_historical_logs")
            .split_once("::")
            .unwrap()
            .1;
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", selector, "--nocapture"])
            .env("OBS_DEINTERLACE_GETTERS_CHILD", "1")
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    // SAFETY: the isolated probes save and restore the process-global logger.
    unsafe {
        assert!(oracle_source_deinterlace_null_log(
            oracle_obs_source_get_deinterlace_mode,
            oracle_obs_source_get_deinterlace_field_order
        ));
        assert!(oracle_source_deinterlace_null_log(
            rust::obs_source_get_deinterlace_mode,
            rust::obs_source_get_deinterlace_field_order
        ));
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn random_fields_match_c(mode in any::<u32>(), top in any::<bool>()) {
        check(mode, top);
    }
}
