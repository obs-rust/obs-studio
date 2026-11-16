//! No intentional differences: original extracted C and Rust on real C sources.
use core::ffi::c_void;
use obs_source::ffi::source_push_delay as rust;
use proptest::prelude::*;
type Setter = unsafe extern "C" fn(*mut c_void, u64);
unsafe extern "C" {
    fn oracle_obs_source_set_push_to_mute_delay(source: *mut c_void, delay: u64);
    fn oracle_obs_source_set_push_to_talk_delay(source: *mut c_void, delay: u64);
    fn oracle_source_delay_check(
        mute: Setter,
        talk: Setter,
        values: *const u64,
        count: usize,
    ) -> bool;
    fn oracle_source_delay_null(mute: Setter, talk: Setter) -> bool;
}
fn check(values: &[u64]) {
    // SAFETY: C fixture owns live sources, initialized locks and handlers, runs
    // synchronous callbacks, and frees only the resources it creates.
    unsafe {
        assert!(oracle_source_delay_check(
            oracle_obs_source_set_push_to_mute_delay,
            oracle_obs_source_set_push_to_talk_delay,
            values.as_ptr(),
            values.len()
        ));
        assert!(oracle_source_delay_check(
            rust::obs_source_set_push_to_mute_delay,
            rust::obs_source_set_push_to_talk_delay,
            values.as_ptr(),
            values.len()
        ));
    }
}
#[test]
fn characterized_edges() {
    check(&[
        0,
        0,
        1,
        u32::MAX as u64,
        u32::MAX as u64 + 1,
        i64::MAX as u64,
        i64::MAX as u64 + 1,
        u64::MAX,
        u64::MAX,
        0,
    ]);
}
#[test]
fn null_diagnostics() {
    if std::env::var_os("OBS_SOURCE_DELAY_CHILD").is_none() {
        let selector = concat!(module_path!(), "::null_diagnostics")
            .split_once("::")
            .unwrap()
            .1;
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", selector, "--nocapture"])
            .env("OBS_SOURCE_DELAY_CHILD", "1")
            .output()
            .unwrap();
        assert!(output.status.success(), "{:?}", output);
        assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
        return;
    }
    // SAFETY: isolated child exclusively changes the global logger; C restores it.
    unsafe {
        assert!(oracle_source_delay_null(
            oracle_obs_source_set_push_to_mute_delay,
            oracle_obs_source_set_push_to_talk_delay
        ));
        assert!(oracle_source_delay_null(
            rust::obs_source_set_push_to_mute_delay,
            rust::obs_source_set_push_to_talk_delay
        ));
    }
}
proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(32)))]
    #[test]
    fn arbitrary_full_width_delays(values in prop::collection::vec(any::<u64>(), 1..12)) {
        check(&values);
    }
}
