//! Hosts without the watchdog process (#4589): only Windows needs it. The
//! in-process thread covers `exit()` on the others.

use std::ffi::OsStr;
use std::path::Path;
use std::time::Duration;

/// No helper.
pub struct Helper;

/// What the helper's wait ended with.
// The shared helper code matches on all three; this host only returns
// Failed.
#[allow(dead_code)]
pub enum Wait {
    /// OBS ended by itself.
    ParentEnded,
    /// OBS was armed and still ran after the timeout.
    TimedOut,
    /// A wait failed, or the host has no helper.
    Failed,
}

/// Never starts.
pub fn spawn_helper(_program: &Path, _args: &[&OsStr], _env: &[(&str, &OsStr)]) -> Option<Helper> {
    None
}

/// Nothing to arm.
pub fn arm_helper(_helper: &Helper) -> bool {
    false
}

/// Nothing to wait on.
pub fn helper_wait(_parent: u64, _arm: u64, _timeout: Duration) -> Wait {
    Wait::Failed
}

/// Nothing to end.
pub fn terminate_parent(_parent: u64, _code: i32) -> bool {
    false
}
