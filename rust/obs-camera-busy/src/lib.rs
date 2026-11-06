//! The camera-busy warning (obs-rust/obs-studio#3724).
//!
//! A V4L2 camera can stream to one process at a time. When OBS's camera
//! source finds its device busy (EBUSY, #799), or another process opens the
//! device while OBS holds it, the frontend names the processes that hold it
//! and asks the user what to do: **Close it** (SIGTERM, then SIGKILL after
//! [`KILL_GRACE_MS`]), **Ignore**, or **Always allow** (never warn about that
//! app again).
//!
//! This crate is the decision part: who holds a device (a scan of a
//! `/proc`-like tree under a caller-supplied root, so tests use fixtures),
//! whether a warning shows a dialog (allowlist, one dialog per device and
//! holder, rate limit), what an answer does, the texts and the log lines.
//! The host calls (signals, the inotify watcher) are in `platform`; the
//! frontend draws the dialog (Qt) and calls the C ABI in [`ffi`].

#![deny(unsafe_code)]

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;

#[allow(unsafe_code)]
pub mod ffi;
mod platform;

pub use platform::{ProcessSignals, Watcher, current_uid};

/// After an answer, the same device and holder do not warn again for this
/// long.
pub const RATE_LIMIT_MS: u64 = 60_000;

/// How long Close it waits after SIGTERM before SIGKILL, and after SIGKILL
/// before it gives up.
pub const KILL_GRACE_MS: u64 = 2_000;

/// How often Close it looks whether the process is gone.
pub const POLL_MS: u64 = 50;

/// How long after an open of the device the watcher looks who holds it, so
/// a program that only probes the camera (opens, queries, closes) is not
/// reported.
pub const OPEN_SETTLE_MS: u64 = 500;

/// A process that holds the device open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Holder {
    pub pid: u32,
    /// The name the dialog shows and the allowlist matches.
    pub name: String,
    /// Owned by this user, so Close it may signal it.
    pub can_close: bool,
}

/// The name of a process: the file name of its `exe` (a Nix
/// `.foo-wrapped` is `foo`), else its `comm`, else the file name of the
/// first word of its `cmdline`, else `?`.
pub fn display_name(exe: Option<&Path>, comm: &str, cmdline: &[u8]) -> String {
    let _ = (exe, comm, cmdline);
    todo!()
}

/// The processes other than `self_pid` with a descriptor on `device`, by
/// the `fd` symlinks under `<proc_root>/<pid>/`, in PID order. `device` is
/// compared with the link targets as given, so the caller resolves it first
/// (`/dev/v4l/by-id/...` to `/dev/videoN`). A process whose descriptors
/// cannot be read is not found. `self_uid` decides [`Holder::can_close`].
pub fn find_holders(
    proc_root: &Path,
    device: &Path,
    self_pid: u32,
    self_uid: Option<u32>,
) -> Vec<Holder> {
    let _ = (proc_root, device, self_pid, self_uid);
    todo!()
}

/// Whether process `pid` under `proc_root` is still running: it has a
/// `stat` and is not a zombie (an exited child of OBS holds nothing).
pub fn is_alive(proc_root: &Path, pid: u32) -> bool {
    let _ = (proc_root, pid);
    todo!()
}

/// The app names that never warn, in OBS's user config as one string.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Allowlist(BTreeSet<String>);

impl Allowlist {
    /// The separator in the config string.
    pub const SEPARATOR: char = ';';

    /// The names in a config string; empty names are dropped.
    pub fn parse(config: &str) -> Self {
        let _ = config;
        todo!()
    }

    /// The config string, names sorted.
    pub fn to_config(&self) -> String {
        todo!()
    }

    pub fn contains(&self, name: &str) -> bool {
        let _ = name;
        todo!()
    }

    /// Adds `name`. An empty name, `?` or one with the separator is not
    /// added. Returns whether the list changed.
    pub fn allow(&mut self, name: &str) -> bool {
        let _ = name;
        todo!()
    }
}

/// What noticed the busy device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cause {
    /// OBS could not open or stream it (EBUSY).
    Busy,
    /// Another process opened it while OBS holds it.
    Opened,
}

/// Whether a warning shows a dialog, and if not, why.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Show {
    Dialog,
    /// Every holder is on the allowlist.
    Allowed,
    /// A dialog for this device and holder is open.
    Duplicate,
    /// This device and holder were answered less than [`RATE_LIMIT_MS`] ago.
    RateLimited,
    /// Nobody else holds the device (any more): nothing to warn about.
    NoHolder,
}

/// The user's answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    CloseIt,
    /// Also the window close and Escape.
    Ignore,
    AlwaysAllow,
}

/// The decision for one warning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Begin {
    pub show: Show,
    /// The holders the dialog names: without the allowlisted ones and those
    /// already warned about. Empty unless `show` is [`Show::Dialog`]; a
    /// dialog for an unknown holder (another user's process) is also empty.
    pub holders: Vec<Holder>,
}

/// What an answer does.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Action {
    /// The PIDs to close (only those this user owns).
    pub close: Vec<u32>,
    /// The allowlist changed: save it.
    pub allowlist_changed: bool,
}

/// The warnings of this OBS session.
#[derive(Debug, Default)]
pub struct Warnings {
    allowlist: Allowlist,
    open: HashSet<(String, String)>,
    answered: HashMap<(String, String), u64>,
}

impl Warnings {
    pub fn new(allowlist: Allowlist) -> Self {
        let _ = allowlist;
        todo!()
    }

    pub fn allowlist(&self) -> &Allowlist {
        todo!()
    }

    pub fn set_allowlist(&mut self, allowlist: Allowlist) {
        let _ = allowlist;
        todo!()
    }

    /// Whether a warning about `device` held by `holders` shows a dialog, at
    /// `now_ms`. A dialog counts as open for its device and holders until
    /// [`Warnings::end`].
    pub fn begin(&mut self, cause: Cause, device: &str, holders: &[Holder], now_ms: u64) -> Begin {
        let _ = (cause, device, holders, now_ms);
        todo!()
    }

    /// The dialog for `device` naming `holders` (as [`Begin::holders`])
    /// ended with `answer` at `now_ms`.
    pub fn end(&mut self, device: &str, holders: &[Holder], answer: Answer, now_ms: u64) -> Action {
        let _ = (device, holders, answer, now_ms);
        todo!()
    }
}

/// How Close it ended for one process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Closed {
    /// It exited after SIGTERM (or was already gone).
    Exited,
    /// It exited only after SIGKILL.
    Killed,
    /// It could not be signalled, or is still there after SIGKILL.
    Failed,
}

/// The host calls Close it makes, so tests can fake a process.
pub trait Signals {
    /// SIGTERM; whether it was sent.
    fn terminate(&mut self, pid: u32) -> bool;
    /// SIGKILL; whether it was sent.
    fn kill(&mut self, pid: u32) -> bool;
    fn alive(&mut self, pid: u32) -> bool;
    fn sleep_ms(&mut self, ms: u64);
}

/// Close it for one process: SIGTERM, then SIGKILL if it is still alive
/// after `grace_ms`. Blocks up to twice `grace_ms`: not on the UI thread.
pub fn close_with(signals: &mut impl Signals, pid: u32, grace_ms: u64) -> Closed {
    let _ = (signals, pid, grace_ms);
    todo!()
}

/// The holders as the dialog and the log name them:
/// `zoom (PID 12), cheese (PID 40)`.
pub fn holder_list(holders: &[Holder]) -> String {
    let _ = holders;
    todo!()
}

/// The log line for a warning and its decision.
pub fn log_line(cause: Cause, device: &str, holders: &[Holder], show: Show) -> String {
    let _ = (cause, device, holders, show);
    todo!()
}

/// The log line for an answer.
pub fn answer_log_line(device: &str, holders: &[Holder], answer: Answer) -> String {
    let _ = (device, holders, answer);
    todo!()
}

/// The log line for how Close it ended for `name` (`pid`).
pub fn close_log_line(name: &str, pid: u32, closed: Closed) -> String {
    let _ = (name, pid, closed);
    todo!()
}

/// Watches `device` for opens and calls `on_holders` on the watcher thread,
/// [`OPEN_SETTLE_MS`] (`settle_ms`) after an open, with the processes other
/// than `self_pid` that hold it then under `proc_root` (often none: OBS's
/// own reopen, a program that only probed it).
pub fn watch_holders(
    device: &Path,
    proc_root: &Path,
    self_pid: u32,
    self_uid: Option<u32>,
    settle_ms: u64,
    on_holders: Box<dyn FnMut(Vec<Holder>) + Send>,
) -> std::io::Result<Watcher> {
    let _ = (device, proc_root, self_pid, self_uid, settle_ms, on_holders);
    todo!()
}
