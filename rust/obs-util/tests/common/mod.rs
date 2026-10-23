//! Shared scratch directory for integration tests.
#![allow(dead_code)]

use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static SEQ: AtomicU64 = AtomicU64::new(0);

/// A fresh directory under the temp dir, named `{prefix}-{pid}-{seq}`.
/// It is removed recursively on drop, including when a test or proptest
/// case panics.
pub struct ScratchDir(PathBuf);

impl ScratchDir {
    pub fn new(prefix: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "{prefix}-{}-{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Deref for ScratchDir {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.0
    }
}

impl AsRef<Path> for ScratchDir {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Runs `test` (its full path in the test binary, e.g.
/// `threading_allocs::init_destroy_counts_one_bzalloc_per_live_handle`)
/// alone in a child process, for checks of process-wide state such as
/// `bnum_allocs()` that any other test running in parallel in the same
/// binary would disturb (#1133).
///
/// In the child (marked by an environment variable) this returns `true`:
/// the caller does the measurement there. In the parent it starts the
/// child with `--exact <test> --test-threads=1`, asserts that it ran that
/// one test and passed, and returns `false`.
pub fn test_name(test: &str) -> &str {
    test.strip_prefix(concat!(env!("CARGO_CRATE_NAME"), "::"))
        .unwrap_or(test)
}

pub fn isolated(test: &str) -> bool {
    isolated_env(test, &[])
}

/// [`isolated`], with `env` set in the child's environment from its start,
/// so the C library reads it at startup (no `setenv` in a running test
/// process).
pub fn isolated_env(test: &str, env: &[(&str, &str)]) -> bool {
    let test = test_name(test);
    const MARKER: &str = "OBS_UTIL_ISOLATED_TEST";
    if std::env::var(MARKER).as_deref() == Ok(test) {
        return true;
    }
    let exe = std::env::current_exe().expect("path of the test binary");
    let out = std::process::Command::new(exe)
        .args([test, "--exact", "--test-threads=1", "--nocapture"])
        .env(MARKER, test)
        .envs(env.iter().copied())
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

#[test]
fn module_paths_resolve_to_exact_libtest_names() {
    assert_eq!(
        test_name(concat!(env!("CARGO_CRATE_NAME"), "::tests::probe")),
        "tests::probe"
    );
    assert_eq!(test_name("tests::probe"), "tests::probe");
}

/// Held by every test that replaces the process-global log handler
/// (`base_set_log_handler`), so they do not swap it under each other.
pub static LOG_HANDLER_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
