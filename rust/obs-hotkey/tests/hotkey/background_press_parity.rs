//! Original C and the real adapter: exact state, signal payload and lock order.
//! No intentional differences or excluded inputs.
use obs_hotkey::ffi::background_press::obs_hotkey_enable_background_press;
use obs_util::ffi::bmem::bnum_allocs;
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_hotkey_background_press_init(present: bool, disabled: bool, listener: bool);
    fn oracle_hotkey_background_press_step(present: bool, enabled: bool);
    fn oracle_hotkey_background_press_snapshot(out: *mut usize);
    fn oracle_hotkey_background_press_free();
    fn oracle_obs_hotkey_enable_background_press(enabled: bool);
}

fn compare(disabled: bool, listener: bool, steps: &[(bool, bool)]) {
    let run = |original: bool| {
        // SAFETY: setup, adapter and original share only this thread's live C
        // fixture. Each snapshot writes exactly ten size_t values. All signals
        // return synchronously before the fixture is destroyed.
        unsafe {
            let before = bnum_allocs();
            oracle_hotkey_background_press_init(false, disabled, listener);
            let snapshots = steps
                .iter()
                .map(|&(present, enabled)| {
                    oracle_hotkey_background_press_step(present, enabled);
                    if original {
                        oracle_obs_hotkey_enable_background_press(enabled);
                    } else {
                        obs_hotkey_enable_background_press(enabled);
                    }
                    let mut snapshot = [0; 10];
                    oracle_hotkey_background_press_snapshot(snapshot.as_mut_ptr());
                    snapshot
                })
                .collect::<Vec<_>>();
            oracle_hotkey_background_press_free();
            assert_eq!(
                bnum_allocs(),
                before,
                "signal storage must be fully released"
            );
            snapshots
        }
    };
    assert_eq!(run(false), run(true));
}

#[test]
fn differential() {
    // Allocation accounting and signal internals are process-wide. Re-exec this
    // exact case in the existing category binary, without adding a test binary.
    let full = concat!(module_path!(), "::differential");
    let test = full.split_once("::").unwrap().1;
    const MARKER: &str = "OBS_HOTKEY_BACKGROUND_PRESS_ISOLATED";
    if std::env::var(MARKER).as_deref() != Ok(test) {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([test, "--exact", "--test-threads=1", "--nocapture"])
            .env(MARKER, test)
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success(),
            "{stdout}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            stdout.contains("1 passed"),
            "child did not run one test: {stdout}"
        );
        return;
    }
    for disabled in [false, true] {
        for listener in [false, true] {
            compare(
                disabled,
                listener,
                &[
                    (false, false),
                    (false, true),
                    (true, true),
                    (true, false),
                    (true, false),
                    (true, true),
                    (true, true),
                    (true, false),
                    (true, true),
                    (false, false),
                ],
            );
        }
    }
    let strategy = (
        any::<bool>(),
        any::<bool>(),
        prop::collection::vec((any::<bool>(), any::<bool>()), 1..64),
    );
    proptest::test_runner::TestRunner::new(proptest::test_runner::Config {
        cases: obs_c_oracle::parity::cases(128),
        failure_persistence: None,
        ..Default::default()
    })
    .run(&strategy, |(disabled, listener, steps)| {
        compare(disabled, listener, &steps);
        Ok(())
    })
    .unwrap();
}
