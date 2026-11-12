//! Original C setters vs public Rust shims, with real signals, fields and locks.
use core::ffi::c_void;
use core::mem::{align_of, offset_of, size_of};
use obs_source::ffi::source_push_to_enable::{self as rust, Fields};
use proptest::prelude::*;
type Setter = unsafe extern "C" fn(*mut c_void, bool);
unsafe extern "C" {
    fn oracle_push_enable_layout(layout: *mut usize);
    fn oracle_push_enable_compare(
        mute: Setter,
        talk: Setter,
        ops: *const u8,
        count: usize,
        flags: u32,
        initial_mute: bool,
        initial_talk: bool,
    ) -> bool;
    fn oracle_push_enable_null(mute: Setter, talk: Setter) -> bool;
    fn oracle_obs_source_enable_push_to_mute(source: *mut c_void, enabled: bool);
    fn oracle_obs_source_enable_push_to_talk(source: *mut c_void, enabled: bool);
}
#[test]
fn real_header_layout() {
    let mut c = [0; 8];
    // SAFETY: the oracle writes eight size_t measurements into this array.
    unsafe { oracle_push_enable_layout(c.as_mut_ptr()) };
    assert_eq!(
        c,
        [
            size_of::<Fields>(),
            align_of::<Fields>(),
            offset_of!(Fields, audio_mutex),
            offset_of!(Fields, mute_enabled),
            offset_of!(Fields, talk_enabled),
            offset_of!(Fields, output_flags),
            offset_of!(Fields, name),
            offset_of!(Fields, signal_changed),
        ]
    );
}
fn check(ops: &[u8], flags: u32, mute: bool, talk: bool) {
    // SAFETY: the runner owns initialized real sources and restores its temporary log handler.
    // This is called only in the isolated child test process.
    assert!(unsafe {
        oracle_push_enable_compare(
            rust::obs_source_enable_push_to_mute,
            rust::obs_source_enable_push_to_talk,
            ops.as_ptr(),
            ops.len(),
            flags,
            mute,
            talk,
        )
    });
}
#[test]
fn original_c_differential() {
    if std::env::var_os("OBS_PUSH_ENABLE_PARITY_CHILD").is_none() {
        let selector = concat!(module_path!(), "::original_c_differential")
            .split_once("::")
            .unwrap()
            .1;
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", selector, "--nocapture"])
            .env("OBS_PUSH_ENABLE_PARITY_CHILD", "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
        return;
    }
    // C transitions() sequence, including independent mute and talk observations.
    let ops = [0, 1, 2, 3, 2, 3, 0, 1, 0, 1, 2, 3];
    for flags in [0, 1, 2, 3, u32::MAX] {
        for mute in [false, true] {
            for talk in [false, true] {
                check(&ops, flags, mute, talk);
            }
        }
    }
    // SAFETY: the child exclusively owns the process logger; each runner restores it.
    unsafe {
        assert!(oracle_push_enable_null(
            oracle_obs_source_enable_push_to_mute,
            oracle_obs_source_enable_push_to_talk
        ));
        assert!(oracle_push_enable_null(
            rust::obs_source_enable_push_to_mute,
            rust::obs_source_enable_push_to_talk
        ));
    }
    let mut runner = proptest::test_runner::TestRunner::new(ProptestConfig {
        cases: obs_c_oracle::parity::cases(128),
        failure_persistence: None,
        ..ProptestConfig::default()
    });
    let strategy = (
        prop::collection::vec(0u8..4, 0..65),
        any::<u32>(),
        any::<bool>(),
        any::<bool>(),
    );
    runner
        .run(&strategy, |(ops, flags, mute, talk)| {
            check(&ops, flags, mute, talk);
            Ok(())
        })
        .unwrap();
}
