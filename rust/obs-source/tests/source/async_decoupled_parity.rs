//! Tier 3: real C fields, all buffers and lock-boundary state. No exclusions.
use core::ffi::c_void;
use obs_source_seam::ffi::source_async_decoupled::obs_source_set_async_decoupled;
use proptest::prelude::*;

type Setter = unsafe extern "C" fn(*mut c_void, bool);
unsafe extern "C" {
    fn oracle_obs_source_set_async_decoupled(source: *mut c_void, value: bool);
    fn oracle_source_async_decoupled_compare(
        setter: Setter,
        previous: bool,
        value: bool,
        seed: u64,
    ) -> bool;
    fn oracle_source_async_decoupled_null_log(setter: Setter) -> bool;
}

fn check(previous: bool, value: bool, seed: u64) {
    // SAFETY: the oracle owns real sources, mutexes and buffers for both calls,
    // restores only changed fields, checks state and frees all allocations.
    assert!(unsafe {
        oracle_source_async_decoupled_compare(obs_source_set_async_decoupled, previous, value, seed)
    });
}

#[test]
fn characterized_edges() {
    for previous in [false, true] {
        for value in [false, true] {
            for seed in [0, 1, 123456, u64::MAX] {
                check(previous, value, seed);
            }
        }
    }
}

#[test]
fn null_diagnostics_match_c() {
    if std::env::var_os("OBS_ASYNC_DECOUPLED_CHILD").is_none() {
        let selector = concat!(module_path!(), "::null_diagnostics_match_c")
            .split_once("::")
            .unwrap()
            .1;
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", selector, "--nocapture"])
            .env("OBS_ASYNC_DECOUPLED_CHILD", "1")
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    // SAFETY: this isolated child saves and restores the global log handler.
    unsafe {
        assert!(oracle_source_async_decoupled_null_log(
            oracle_obs_source_set_async_decoupled
        ));
        assert!(oracle_source_async_decoupled_null_log(
            obs_source_set_async_decoupled
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
    fn random_state_matches_c(previous in any::<bool>(), value in any::<bool>(), seed in any::<u64>()) {
        check(previous, value, seed);
    }
}
