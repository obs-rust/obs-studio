//! Tier 3: original C, production adapters and shim over real-header fields.
//! Re-exec isolates the existing initialized-core adapter's process-global slot.
use obs_core::ffi::monitoring_device_getter::obs_get_audio_monitoring_device;
use proptest::prelude::*;
use proptest::test_runner::{Config, TestRunner};
use std::ffi::{c_char, c_void};
use std::ptr::{null, null_mut, without_provenance_mut};

unsafe extern "C" {
    fn oracle_core_monitoring_device_set(name: *mut c_char, id: *mut c_char) -> *mut c_void;
    fn oracle_core_initialized_set(core: *mut c_void);
    fn oracle_obs_get_audio_monitoring_device(name: *mut *const c_char, id: *mut *const c_char);
}

fn compare(fields: [usize; 2]) {
    // SAFETY: this exact-test child owns both fixtures. Pointer tokens are only
    // copied, never dereferenced. All output slots are live and writable.
    unsafe {
        let core = oracle_core_monitoring_device_set(
            without_provenance_mut(fields[0]),
            without_provenance_mut(fields[1]),
        );
        oracle_core_initialized_set(core);
        for mask in 0..4 {
            let mut expected = [without_provenance_mut::<c_char>(17).cast_const(); 2];
            let mut actual = expected;
            oracle_obs_get_audio_monitoring_device(
                if mask & 1 != 0 {
                    &mut expected[0]
                } else {
                    null_mut()
                },
                if mask & 2 != 0 {
                    &mut expected[1]
                } else {
                    null_mut()
                },
            );
            obs_get_audio_monitoring_device(
                if mask & 1 != 0 {
                    &mut actual[0]
                } else {
                    null_mut()
                },
                if mask & 2 != 0 {
                    &mut actual[1]
                } else {
                    null_mut()
                },
            );
            assert_eq!(actual, expected);
            assert_eq!(
                actual[0] as usize,
                if mask & 1 != 0 { fields[0] } else { 17 }
            );
            assert_eq!(
                actual[1] as usize,
                if mask & 2 != 0 { fields[1] } else { 17 }
            );
        }
        let mut expected = null();
        let mut actual = null();
        oracle_obs_get_audio_monitoring_device(&mut expected, &mut expected);
        obs_get_audio_monitoring_device(&mut actual, &mut actual);
        assert_eq!(actual, expected);
        assert_eq!(actual as usize, fields[1]);
    }
}

#[test]
fn monitoring_device_matches_c() {
    const CHILD: &str = "OBS_MONITORING_DEVICE_GETTER_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let name = format!(
            "{}::monitoring_device_matches_c",
            module_path!().split_once("::").unwrap().1
        );
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &name, "--nocapture"])
            .env(CHILD, "1")
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
    // SAFETY: both absent outputs are accepted without an initialized core.
    unsafe {
        oracle_core_initialized_set(null_mut());
        obs_get_audio_monitoring_device(null_mut(), null_mut());
    }
    for fields in [[0, 0], [1, 0], [0, 2], [3, 3], [usize::MAX, 1], [0, 0]] {
        compare(fields);
        compare(fields); // getters must not alter the stored pointers
    }
    let mut runner = TestRunner::new(Config {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..Config::default()
    });
    runner
        .run(
            &proptest::collection::vec(any::<[usize; 2]>(), 0..32),
            |sequence| {
                for fields in sequence {
                    compare(fields);
                }
                Ok(())
            },
        )
        .unwrap();
}
