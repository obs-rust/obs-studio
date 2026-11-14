//! The watchdog process (#4589).
//!
//! On Windows, a plugin DLL's static destructors and `atexit` handlers run
//! in its `DLL_PROCESS_DETACH`, inside `ExitProcess`. `ExitProcess` has
//! ended every other thread of the process by then, the watchdog thread
//! too, so a destructor that waits for a signal that never comes hangs OBS
//! forever. Only something outside the process can end it.
//!
//! So on Windows, OBS starts a small helper process at startup ([`start`]),
//! long before it quits: a first-start antivirus scan of the helper is over
//! by then. The helper gets the OBS process handle and an arm event through
//! an explicit handle list, and the timeout and the log path through its
//! environment ([`ENV_PARENT`], [`ENV_ARM`], [`ENV_TIMEOUT_MS`],
//! [`ENV_LOG`]). It waits on both handles:
//! - OBS ends first: the helper ends;
//! - [`crate::arm`] sets the event: the helper waits on OBS for the timeout,
//!   then appends the watchdog line to the log and ends OBS with
//!   [`crate::EXIT_CODE`].
//!
//! The helper notices the end of OBS through the process handle only,
//! never through a closed handle: the children that OBS starts later could
//! hold copies.
//!
//! When the helper did not start (another host, or its file is missing),
//! [`crate::arm`] starts the in-process thread as before.

use std::ffi::{OsStr, OsString};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use crate::platform;

/// The helper's file name, without the host's executable suffix. It ships
/// next to the OBS executable.
pub const HELPER_NAME: &str = "obs-exit-watchdog";

/// The helper's environment: the OBS process handle, as a decimal number.
pub const ENV_PARENT: &str = "OBS_EXIT_WATCHDOG_PARENT";
/// The helper's environment: the arm event handle, as a decimal number.
pub const ENV_ARM: &str = "OBS_EXIT_WATCHDOG_ARM";
/// The helper's environment: the timeout in milliseconds.
pub const ENV_TIMEOUT_MS: &str = "OBS_EXIT_WATCHDOG_TIMEOUT_MS";
/// The helper's environment: the log file to append the line to (absent:
/// no log file).
pub const ENV_LOG: &str = "OBS_EXIT_WATCHDOG_LOG";

/// The exit code of a helper whose environment is not complete.
pub const BAD_ARGS: i32 = 2;

/// The started helper of this process, until [`crate::arm`] arms it.
static HELPER: Mutex<Option<platform::Helper>> = Mutex::new(None);

/// Starts the helper next to the running executable, with `timeout` and
/// the log file `log`. Returns false when it did not start: another host,
/// no helper file, or a helper already started.
pub fn start(timeout: Duration, log: Option<&Path>) -> bool {
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    let Some(dir) = exe.parent() else {
        return false;
    };
    let program = dir.join(format!("{HELPER_NAME}{}", std::env::consts::EXE_SUFFIX));
    start_with(&program, &[], timeout, log)
}

/// [`start`] with a given program and arguments (tests).
pub fn start_with(program: &Path, args: &[&OsStr], timeout: Duration, log: Option<&Path>) -> bool {
    let _ = (program, args, timeout, log);
    false
}

/// Arms the started helper, if there is one. Returns false when there is
/// none, or when the event could not be set.
pub(crate) fn arm_started() -> bool {
    false
}

/// What the helper reads from its environment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HelperArgs {
    /// The OBS process handle.
    pub parent: u64,
    /// The arm event handle.
    pub arm: u64,
    /// How long OBS may take after it armed the helper.
    pub timeout: Duration,
    /// The log file to append the line to.
    pub log: Option<PathBuf>,
}

impl HelperArgs {
    /// Reads the arguments with `var` (the environment): `None` when a
    /// handle or the timeout is missing or not a decimal number, or a
    /// handle is 0.
    pub fn read(var: impl Fn(&str) -> Option<OsString>) -> Option<Self> {
        let _ = var;
        None
    }
}

/// The helper's `main`: reads its environment, waits, and returns its exit
/// code: 0 when OBS ended (by itself, or by the helper), 1 when a wait
/// failed or the host has no helper, [`BAD_ARGS`] when the environment is
/// not complete.
pub fn run_helper() -> i32 {
    BAD_ARGS
}

/// Appends `text` to `log`, if it opens.
#[allow(dead_code)] // used by run_helper (GREEN)
fn append(log: Option<&Path>, text: &[u8]) {
    if let Some(mut file) = log.and_then(|path| OpenOptions::new().append(true).open(path).ok()) {
        let _ = file.write_all(text);
    }
}
