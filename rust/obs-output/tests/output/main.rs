//! obs-output integration tests: a module of the core category binary
//! (`obs-tests-core`, rust/obs-data/tests/core.rs), not a binary of its own.

mod caption_text;
mod caption_text_parity;
mod delay;
mod delay_parity;
mod encoder_audio_start;
mod encoder_audio_start_parity;
mod encoder_roi;
mod encoder_roi_parity;
mod encoder_size;
mod encoder_size_parity;
mod interleave;
mod interleave_parity;
mod protocols;
mod protocols_parity;
mod service;
mod service_lifecycle;
mod service_lifecycle_parity;
mod service_parity;
mod service_settings;
mod service_settings_parity;
mod video_checks;
mod video_checks_parity;

use std::process::Command;
use std::sync::Mutex;

/// The oracle's process-wide service state: the `find_service()` table,
/// the module lists and the lifecycle trace hook, shared by the service
/// parity tests on both sides.
pub(crate) static SERVICE_GLOBALS: Mutex<()> = Mutex::new(());

const MARKER_ENV: &str = "OBS_OUTPUT_ISOLATED_TEST";

/// Runs `test` (its `module_path!()`-based name) alone in a child process
/// of this executable, for tests that swap the process-wide log handler.
/// Returns true in the child, where the test body runs; the parent checks
/// that the child ran exactly that test and passed.
pub(crate) fn isolated(test: &str) -> bool {
    let test = test
        .strip_prefix(concat!(env!("CARGO_CRATE_NAME"), "::"))
        .unwrap_or(test);
    if std::env::var(MARKER_ENV).as_deref() == Ok(test) {
        return true;
    }
    let exe = std::env::current_exe().expect("path of the test binary");
    let out = Command::new(exe)
        .args([test, "--exact", "--test-threads=1", "--nocapture"])
        .env(MARKER_ENV, test)
        .output()
        .expect("start the isolated test process");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "isolated {test} failed:\n{stdout}\n{stderr}"
    );
    assert!(
        stdout.contains("1 passed"),
        "isolated {test} did not run exactly one test:\n{stdout}"
    );
    false
}
