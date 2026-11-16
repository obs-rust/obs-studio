//! obs-exit-watchdog (#4065). The watchdog ends its process, so each case
//! that arms it re-runs this test binary with an exact selector and checks
//! the child from outside.

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use obs_exit_watchdog::{EXIT_CODE, LINE_PREFIX, arm, line};

#[path = "helper.rs"]
mod helper;
#[path = "images.rs"]
mod images;
#[path = "naming.rs"]
mod naming;

const MARKER: &str = "OBS_EXIT_WATCHDOG_CHILD";
/// The log file the child gets, in the parent's temporary directory.
pub(crate) const LOG: &str = "OBS_EXIT_WATCHDOG_LOG";
/// A child that the watchdog does not end is killed after this long.
const CHILD_LIMIT: Duration = Duration::from_secs(5);

/// The test's name for an exact libtest selector, from `module_path!()` so
/// a regrouping of the modules cannot run zero child tests.
pub(crate) fn selector(full: &str) -> &str {
    full.split_once("::").unwrap().1
}

/// True in the child of `test`.
pub(crate) fn in_child(test: &str) -> bool {
    std::env::var(MARKER).as_deref() == Ok(test)
}

/// Runs `test` alone in a child, with `log` as its log file. Kills the child
/// and fails when it runs longer than [`CHILD_LIMIT`].
pub(crate) fn run_child(test: &str, log: &Path) -> (Output, Duration) {
    let start = Instant::now();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([test, "--exact", "--test-threads=1", "--nocapture"])
        .env(MARKER, test)
        .env(LOG, log)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    while child.try_wait().unwrap().is_none() {
        if start.elapsed() > CHILD_LIMIT {
            child.kill().unwrap();
            let out = child.wait_with_output().unwrap();
            panic!(
                "the child still ran after {CHILD_LIMIT:?}: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let elapsed = start.elapsed();
    (child.wait_with_output().unwrap(), elapsed)
}

/// A log path of its own for each case, in a new temporary directory.
pub(crate) fn log_path(case: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("obs-exit-watchdog-{}-{case}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("log.txt")
}

pub(crate) fn count_lines(text: &str) -> usize {
    text.lines().filter(|l| l.starts_with(LINE_PREFIX)).count()
}

/// Blocks the child's thread for longer than the parent waits.
pub(crate) fn block() -> ! {
    loop {
        std::thread::sleep(Duration::from_secs(60));
    }
}

#[test]
fn the_line_names_the_watchdog_and_the_timeout() {
    assert_eq!(
        line(Duration::from_secs(10)),
        "exit-watchdog: exit did not finish in 10 s: a static destructor or \
         atexit handler is blocking; ending the process\n"
    );
    assert!(line(Duration::from_millis(200)).contains(" in 200 ms: "));
}

#[test]
fn a_blocked_process_ends_with_one_line() {
    let test = selector(concat!(
        module_path!(),
        "::a_blocked_process_ends_with_one_line"
    ));
    if in_child(test) {
        let log = PathBuf::from(std::env::var_os(LOG).unwrap());
        assert!(arm(Duration::from_millis(200), Some(&log)));
        block();
    }
    let log = log_path("blocked");
    std::fs::write(&log, "earlier line\n").unwrap();
    let (out, elapsed) = run_child(test, &log);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(EXIT_CODE), "{stderr}");
    assert!(elapsed >= Duration::from_millis(200), "{elapsed:?}");
    assert_eq!(count_lines(&stderr), 1, "{stderr}");
    let text = std::fs::read_to_string(&log).unwrap();
    assert_eq!(
        text,
        format!("earlier line\n{}", line(Duration::from_millis(200)))
    );
}

#[test]
fn a_blocked_process_without_its_log_file_still_ends() {
    let test = selector(concat!(
        module_path!(),
        "::a_blocked_process_without_its_log_file_still_ends"
    ));
    if in_child(test) {
        let log = PathBuf::from(std::env::var_os(LOG).unwrap());
        assert!(arm(Duration::from_millis(200), Some(&log)));
        block();
    }
    let log = log_path("no-log").join("missing-dir").join("log.txt");
    let (out, _) = run_child(test, &log);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(EXIT_CODE), "{stderr}");
    assert_eq!(count_lines(&stderr), 1, "{stderr}");
    assert!(!log.exists());
}

#[test]
fn a_normal_exit_is_not_delayed_and_writes_nothing() {
    let test = selector(concat!(
        module_path!(),
        "::a_normal_exit_is_not_delayed_and_writes_nothing"
    ));
    if in_child(test) {
        let log = PathBuf::from(std::env::var_os(LOG).unwrap());
        assert!(arm(Duration::from_secs(4), Some(&log)));
        assert!(!arm(Duration::from_secs(4), Some(&log)), "armed twice");
        return;
    }
    let log = log_path("normal");
    let (out, elapsed) = run_child(test, &log);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{stdout}\n{stderr}");
    assert!(
        stdout.contains("1 passed"),
        "child did not run one test: {stdout}"
    );
    assert!(elapsed < Duration::from_secs(3), "{elapsed:?}");
    assert_eq!(count_lines(&stderr), 0, "{stderr}");
    assert_eq!(std::fs::read_to_string(&log).unwrap_or_default(), "");
}
