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
    fn file_name(path: &str) -> Option<String> {
        let name = Path::new(path).file_name()?.to_string_lossy();
        // Nix wraps a program as `.foo-wrapped` next to it.
        let name = name
            .strip_prefix('.')
            .and_then(|n| n.strip_suffix("-wrapped"))
            .unwrap_or(&name);
        (!name.is_empty()).then(|| name.to_owned())
    }

    let comm = comm.trim_end_matches('\n');
    let comm = file_name(comm);
    if let Some(exe) = exe {
        let exe = exe.to_string_lossy();
        // A binary replaced while it runs reads `... (deleted)`.
        if let Some(name) = file_name(exe.strip_suffix(" (deleted)").unwrap_or(&exe)) {
            // comm is the exe's name cut to 15 bytes, unless the binary is a
            // multicall one (coreutils' sleep) or renamed itself: then comm
            // is the name `ps` shows.
            match &comm {
                Some(c) if !name.starts_with(c.as_str()) => return c.clone(),
                _ => return name,
            }
        }
    }
    if let Some(comm) = comm {
        return comm;
    }
    let first = cmdline.split(|b| *b == 0).next().unwrap_or_default();
    file_name(&String::from_utf8_lossy(first)).unwrap_or_else(|| "?".to_owned())
}

/// Whether `<dir>/fd` has a descriptor on `device`.
fn holds(dir: &Path, device: &Path) -> bool {
    let Ok(fds) = std::fs::read_dir(dir.join("fd")) else {
        return false;
    };
    fds.flatten()
        .any(|fd| std::fs::read_link(fd.path()).is_ok_and(|target| target == device))
}

/// A process directory name as a PID (1 and up).
fn pid_of(name: &std::ffi::OsStr) -> Option<u32> {
    let name = name.to_str()?;
    if name.is_empty() || !name.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    name.parse().ok().filter(|pid| *pid > 0)
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
    let Ok(entries) = std::fs::read_dir(proc_root) else {
        return Vec::new();
    };
    let mut holders: Vec<Holder> = entries
        .flatten()
        .filter_map(|e| Some((pid_of(&e.file_name())?, e.path())))
        .filter(|(pid, dir)| *pid != self_pid && holds(dir, device))
        .map(|(pid, dir)| {
            let exe = std::fs::read_link(dir.join("exe")).ok();
            let comm = std::fs::read_to_string(dir.join("comm")).unwrap_or_default();
            let cmdline = std::fs::read(dir.join("cmdline")).unwrap_or_default();
            let uid = platform::owner_uid(&dir);
            Holder {
                pid,
                name: display_name(exe.as_deref(), &comm, &cmdline),
                can_close: uid.is_some() && uid == self_uid,
            }
        })
        .collect();
    holders.sort_by_key(|h| h.pid);
    holders
}

/// Whether process `pid` under `proc_root` is still running: it has a
/// `stat` and is not a zombie (an exited child of OBS holds nothing).
pub fn is_alive(proc_root: &Path, pid: u32) -> bool {
    let Ok(stat) = std::fs::read_to_string(proc_root.join(pid.to_string()).join("stat")) else {
        return false;
    };
    // `pid (comm) state ...`; comm may hold spaces and parentheses.
    let state = stat
        .rfind(')')
        .and_then(|end| stat[end + 1..].split_whitespace().next());
    !matches!(state, Some("Z" | "X" | "x"))
}

/// The app names that never warn, in OBS's user config as one string.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Allowlist(BTreeSet<String>);

impl Allowlist {
    /// The separator in the config string.
    pub const SEPARATOR: char = ';';

    /// The names in a config string; empty names are dropped.
    pub fn parse(config: &str) -> Self {
        Self(
            config
                .split(Self::SEPARATOR)
                .map(str::trim)
                .filter(|n| !n.is_empty())
                .map(str::to_owned)
                .collect(),
        )
    }

    /// The config string, names sorted.
    pub fn to_config(&self) -> String {
        let names: Vec<&str> = self.0.iter().map(String::as_str).collect();
        names.join(&Self::SEPARATOR.to_string())
    }

    pub fn contains(&self, name: &str) -> bool {
        self.0.contains(name)
    }

    /// Adds `name`. An empty name, `?` or one with the separator is not
    /// added. Returns whether the list changed.
    pub fn allow(&mut self, name: &str) -> bool {
        let name = name.trim();
        if name.is_empty() || name == "?" || name.contains(Self::SEPARATOR) {
            return false;
        }
        self.0.insert(name.to_owned())
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
        Self {
            allowlist,
            ..Self::default()
        }
    }

    pub fn allowlist(&self) -> &Allowlist {
        &self.allowlist
    }

    pub fn set_allowlist(&mut self, allowlist: Allowlist) {
        self.allowlist = allowlist;
    }

    /// The key of a holder of `device`; an unknown holder has the empty name.
    fn key(device: &str, name: &str) -> (String, String) {
        (device.to_owned(), name.to_owned())
    }

    fn recent(&self, key: &(String, String), now_ms: u64) -> bool {
        // A clock that went back counts as recent.
        self.answered
            .get(key)
            .is_some_and(|&at| now_ms < at.saturating_add(RATE_LIMIT_MS))
    }

    /// Whether a warning about `device` held by `holders` shows a dialog, at
    /// `now_ms`. A dialog counts as open for its device and holders until
    /// [`Warnings::end`].
    pub fn begin(&mut self, cause: Cause, device: &str, holders: &[Holder], now_ms: u64) -> Begin {
        let not_shown = |show| Begin {
            show,
            holders: Vec::new(),
        };
        if holders.is_empty() {
            // An open by OBS itself, or by a program that only probed it.
            if cause == Cause::Opened {
                return not_shown(Show::NoHolder);
            }
            // Busy, but the holder cannot be seen (another user's process).
            let key = Self::key(device, "");
            if self.open.contains(&key) {
                return not_shown(Show::Duplicate);
            }
            if self.recent(&key, now_ms) {
                return not_shown(Show::RateLimited);
            }
            self.open.insert(key);
            return Begin {
                show: Show::Dialog,
                holders: Vec::new(),
            };
        }

        let warned: Vec<&Holder> = holders
            .iter()
            .filter(|h| !self.allowlist.contains(&h.name))
            .collect();
        if warned.is_empty() {
            return not_shown(Show::Allowed);
        }
        let mut fresh: Vec<Holder> = Vec::new();
        let mut any_open = false;
        for h in warned {
            let key = Self::key(device, &h.name);
            if self.open.contains(&key) {
                any_open = true;
            } else if !self.recent(&key, now_ms) && !fresh.iter().any(|f| f.name == h.name) {
                fresh.push(h.clone());
            }
        }
        if fresh.is_empty() {
            return not_shown(if any_open {
                Show::Duplicate
            } else {
                Show::RateLimited
            });
        }
        for h in &fresh {
            self.open.insert(Self::key(device, &h.name));
        }
        Begin {
            show: Show::Dialog,
            holders: fresh,
        }
    }

    /// The dialog for `device` naming `holders` (as [`Begin::holders`])
    /// ended with `answer` at `now_ms`.
    pub fn end(&mut self, device: &str, holders: &[Holder], answer: Answer, now_ms: u64) -> Action {
        let names: Vec<&str> = if holders.is_empty() {
            vec![""]
        } else {
            holders.iter().map(|h| h.name.as_str()).collect()
        };
        for name in names {
            let key = Self::key(device, name);
            self.open.remove(&key);
            self.answered.insert(key, now_ms);
        }
        match answer {
            Answer::Ignore => Action::default(),
            Answer::CloseIt => Action {
                close: holders.iter().filter(|h| h.can_close).map(|h| h.pid).collect(),
                allowlist_changed: false,
            },
            Answer::AlwaysAllow => {
                let mut changed = false;
                for h in holders {
                    changed |= self.allowlist.allow(&h.name);
                }
                Action {
                    close: Vec::new(),
                    allowlist_changed: changed,
                }
            }
        }
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
    /// Waits up to `grace_ms` for `pid` to go; whether it went.
    fn gone(signals: &mut impl Signals, pid: u32, grace_ms: u64) -> bool {
        let mut waited = 0;
        loop {
            if !signals.alive(pid) {
                return true;
            }
            if waited >= grace_ms {
                return false;
            }
            signals.sleep_ms(POLL_MS);
            waited += POLL_MS;
        }
    }

    if !signals.alive(pid) {
        return Closed::Exited;
    }
    if !signals.terminate(pid) {
        return if signals.alive(pid) {
            Closed::Failed
        } else {
            Closed::Exited
        };
    }
    if gone(signals, pid, grace_ms) {
        return Closed::Exited;
    }
    if signals.kill(pid) && gone(signals, pid, grace_ms) {
        return Closed::Killed;
    }
    if signals.alive(pid) {
        Closed::Failed
    } else {
        Closed::Killed
    }
}

/// The holders as the dialog and the log name them:
/// `zoom (PID 12), cheese (PID 40)`.
pub fn holder_list(holders: &[Holder]) -> String {
    let names: Vec<String> = holders
        .iter()
        .map(|h| format!("{} (PID {})", h.name, h.pid))
        .collect();
    names.join(", ")
}

/// The log line for a warning and its decision.
pub fn log_line(cause: Cause, device: &str, holders: &[Holder], show: Show) -> String {
    let who = if holders.is_empty() {
        "another program".to_owned()
    } else {
        holder_list(holders)
    };
    let what = match cause {
        Cause::Busy => format!("{device} is busy, in use by {who}"),
        Cause::Opened => format!("{who} opened {device} while OBS uses it"),
    };
    let decision = match show {
        Show::Dialog => "warning shown",
        Show::Allowed => "allowed, not warned",
        Show::Duplicate => "a warning for it is already open",
        Show::RateLimited => "warned recently, not warned again",
        Show::NoHolder => "nobody else holds it",
    };
    format!("[camera-busy] {what}: {decision}")
}

/// The log line for an answer.
pub fn answer_log_line(device: &str, holders: &[Holder], answer: Answer) -> String {
    match answer {
        Answer::CloseIt => {
            let closable: Vec<Holder> = holders.iter().filter(|h| h.can_close).cloned().collect();
            if closable.is_empty() {
                format!("[camera-busy] {device}: Close it, but no holder can be closed")
            } else {
                format!(
                    "[camera-busy] {device}: Close it, closing {}",
                    holder_list(&closable)
                )
            }
        }
        Answer::Ignore => format!("[camera-busy] {device}: Ignore"),
        Answer::AlwaysAllow => format!(
            "[camera-busy] {device}: Always allow {}",
            holder_list(holders)
        ),
    }
}

/// The log line for how Close it ended for `name` (`pid`).
pub fn close_log_line(name: &str, pid: u32, closed: Closed) -> String {
    let how = match closed {
        Closed::Exited => "exited on SIGTERM",
        Closed::Killed => "did not exit on SIGTERM and was killed (SIGKILL)",
        Closed::Failed => "could not be closed",
    };
    format!("[camera-busy] {name} (PID {pid}) {how}")
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
    let mut on_holders = on_holders;
    let (dev, root) = (device.to_path_buf(), proc_root.to_path_buf());
    Watcher::start(
        device,
        settle_ms,
        Box::new(move || on_holders(find_holders(&root, &dev, self_pid, self_uid))),
    )
}
