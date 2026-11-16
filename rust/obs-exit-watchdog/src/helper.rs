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
//! by then. The helper gets the OBS process handle, an arm event and a
//! section through an explicit handle list, and the timeout and the log
//! path through its environment ([`ENV_PARENT`], [`ENV_ARM`], [`ENV_TABLE`],
//! [`ENV_TIMEOUT_MS`], [`ENV_LOG`]). It waits on the process and the event:
//! - OBS ends first: the helper ends;
//! - [`crate::arm`] writes the table and the armed thread into the section
//!   ([`encode_arm`]) and sets the event: the helper waits on OBS for the
//!   timeout, then walks the armed thread's stack from outside, appends the
//!   watchdog line, naming the library it waits in, to the log and ends
//!   OBS with [`crate::EXIT_CODE`].
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

use crate::images::{Entry, Owner, Table};
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

/// The helper's environment: the section that [`crate::arm`] writes the
/// table and the armed thread into, as a decimal handle (absent: the line
/// names no library).
pub const ENV_TABLE: &str = "OBS_EXIT_WATCHDOG_TABLE";

/// The exit code of a helper whose environment is not complete.
pub const BAD_ARGS: i32 = 2;

/// The start of what [`encode_arm`] writes (a section nobody wrote is all
/// zeros).
const ARM_MAGIC: &[u8; 8] = b"OBSWDT01";

/// What OBS writes into the section when it arms the helper: the id of the
/// armed thread (the one that then runs `exit()`) and the table.
///
/// Layout, little-endian: the magic, the thread id (u64), the entry count
/// (u32), then per entry its range (two u64), its owner tag (u8: 0 OBS,
/// 1 a module, 2 post-load, 3 after startup), its file name (u16 length and
/// UTF-8) and, for a module, the module's name (the same way).
pub fn encode_arm(thread: u64, table: &Table) -> Vec<u8> {
    fn text(out: &mut Vec<u8>, s: &str) {
        // A name longer than u16 is cut; names are file names.
        let bytes = &s.as_bytes()[..s.len().min(usize::from(u16::MAX))];
        let bytes = &bytes[..s.floor_char_boundary(bytes.len())];
        out.extend(u16::try_from(bytes.len()).unwrap_or(0).to_le_bytes());
        out.extend(bytes);
    }
    let entries = table.entries();
    let mut out = Vec::with_capacity(24 + entries.len() * 64);
    out.extend(ARM_MAGIC);
    out.extend(thread.to_le_bytes());
    out.extend(u32::try_from(entries.len()).unwrap_or(0).to_le_bytes());
    for e in entries {
        out.extend((e.start as u64).to_le_bytes());
        out.extend((e.end as u64).to_le_bytes());
        let (tag, module) = match &e.owner {
            Owner::Obs => (0u8, None),
            Owner::Module(m) => (1, Some(m.as_str())),
            Owner::PostLoad => (2, None),
            Owner::AfterStartup => (3, None),
        };
        out.push(tag);
        text(&mut out, &e.file);
        if let Some(m) = module {
            text(&mut out, m);
        }
    }
    out
}

/// Reads what [`encode_arm`] wrote; `None` when it is not complete. Bytes
/// after it (the rest of the section) are ignored.
pub fn decode_arm(bytes: &[u8]) -> Option<(u64, Table)> {
    struct Reader<'a>(&'a [u8]);
    impl<'a> Reader<'a> {
        fn take(&mut self, n: usize) -> Option<&'a [u8]> {
            let (head, rest) = self.0.split_at_checked(n)?;
            self.0 = rest;
            Some(head)
        }
        fn u64(&mut self) -> Option<u64> {
            Some(u64::from_le_bytes(self.take(8)?.try_into().ok()?))
        }
        fn text(&mut self) -> Option<String> {
            let len = u16::from_le_bytes(self.take(2)?.try_into().ok()?);
            String::from_utf8(self.take(usize::from(len))?.to_vec()).ok()
        }
    }
    let mut r = Reader(bytes);
    if r.take(ARM_MAGIC.len())? != ARM_MAGIC {
        return None;
    }
    let thread = r.u64()?;
    let count = u32::from_le_bytes(r.take(4)?.try_into().ok()?);
    let mut entries = Vec::new();
    for _ in 0..count {
        let start = usize::try_from(r.u64()?).ok()?;
        let end = usize::try_from(r.u64()?).ok()?;
        let tag = r.take(1)?[0];
        let file = r.text()?;
        let owner = match tag {
            0 => Owner::Obs,
            1 => Owner::Module(r.text()?),
            2 => Owner::PostLoad,
            3 => Owner::AfterStartup,
            _ => return None,
        };
        entries.push(Entry {
            start,
            end,
            file,
            owner,
        });
    }
    Some((thread, Table::from_entries(entries)))
}

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
    let mut started = HELPER
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if started.is_some() {
        return false;
    }
    let timeout_ms = timeout.as_millis().to_string();
    let mut env: Vec<(&str, &OsStr)> = vec![(ENV_TIMEOUT_MS, OsStr::new(&timeout_ms))];
    if let Some(log) = log {
        env.push((ENV_LOG, log.as_os_str()));
    }
    *started = platform::spawn_helper(program, args, &env);
    started.is_some()
}

/// Arms the started helper, if there is one, and hands it `table` and the
/// calling thread (the one that then runs `exit()`). Returns false when
/// there is none, or when the event could not be set.
pub(crate) fn arm_started(table: &Table) -> bool {
    let started = HELPER
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    started
        .as_ref()
        .is_some_and(|helper| platform::arm_helper(helper, table))
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
    /// The section with the armed thread and the table.
    pub table: Option<u64>,
}

impl HelperArgs {
    /// Reads the arguments with `var` (the environment): `None` when a
    /// handle or the timeout is missing or not a decimal number, or a
    /// handle is 0.
    pub fn read(var: impl Fn(&str) -> Option<OsString>) -> Option<Self> {
        let number = |key: &str| -> Option<u64> {
            let text = var(key)?.into_string().ok()?;
            if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            text.parse().ok()
        };
        let parent = number(ENV_PARENT).filter(|&h| h != 0)?;
        let arm = number(ENV_ARM).filter(|&h| h != 0)?;
        let timeout = Duration::from_millis(number(ENV_TIMEOUT_MS)?);
        let log = var(ENV_LOG).filter(|v| !v.is_empty()).map(PathBuf::from);
        let table = number(ENV_TABLE).filter(|&h| h != 0);
        Some(Self {
            parent,
            arm,
            timeout,
            log,
            table,
        })
    }
}

/// The helper's `main`: reads its environment, waits, and returns its exit
/// code: 0 when OBS ended (by itself, or by the helper), 1 when a wait
/// failed or the host has no helper, [`BAD_ARGS`] when the environment is
/// not complete.
pub fn run_helper() -> i32 {
    let Some(args) = HelperArgs::read(|key| std::env::var_os(key)) else {
        return BAD_ARGS;
    };
    match platform::helper_wait(args.parent, args.arm, args.timeout) {
        platform::Wait::ParentEnded => 0,
        platform::Wait::TimedOut => {
            // OBS still runs: walk the armed thread's stack before ending it.
            let named = args
                .table
                .and_then(platform::read_section)
                .and_then(|bytes| decode_arm(&bytes));
            let text = match named {
                Some((thread, table)) => {
                    let mut frames = [0usize; 64];
                    let n = platform::capture_remote(args.parent, thread, &mut frames);
                    crate::line_naming(args.timeout, crate::culprit(&frames[..n], &table))
                }
                None => crate::line(args.timeout),
            };
            append(args.log.as_deref(), text.as_bytes());
            if platform::terminate_parent(args.parent, crate::EXIT_CODE) {
                0
            } else {
                1
            }
        }
        platform::Wait::Failed => 1,
    }
}

/// Appends `text` to `log`, if it opens.
fn append(log: Option<&Path>, text: &[u8]) {
    if let Some(mut file) = log.and_then(|path| OpenOptions::new().append(true).open(path).ok()) {
        let _ = file.write_all(text);
    }
}
