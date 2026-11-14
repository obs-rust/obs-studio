//! The exit watchdog (obs-rust/obs-studio#4065).
//!
//! OBS can finish its own shutdown and still not quit: a static destructor
//! or an `atexit` handler of a plugin blocks in `exit()`, after `main()` has
//! returned (#3739). The frontend arms this watchdog after its last log
//! line. When the process still runs after the timeout, the watchdog writes
//! one line to the log file and to stderr and ends the process with
//! [`EXIT_CODE`].
//!
//! The line names the library where the armed thread waits and the OBS
//! module that brought it in (#4525): the frontend notes the modules and the
//! image list after loading and after post-load ([`note_module`],
//! [`note_loaded`], [`note_post_load`]); [`arm`] builds the table
//! ([`images::Table`]) and notes the arming thread, which then runs
//! `exit()`. When it fires, the watchdog captures that thread's stack and
//! names the first library on it that OBS itself did not load.
//!
//! All other work is done when the watchdog is armed: when it fires, `main()`
//! has returned, the frontend's log file is closed and destructors run.
//! Firing captures the stack into a fixed buffer, writes the line from a
//! fixed buffer and ends the process without `exit()` (which would run the
//! same destructors again) and without `abort()` (which looks like a crash).
//! The host calls are in `platform`.

#![deny(unsafe_code)]

use std::fmt::Write as _;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

#[allow(unsafe_code)]
pub mod ffi;
pub mod helper;
pub mod images;
mod platform;
pub mod stack;

pub use helper::{HelperArgs, run_helper, start, start_with};
use images::{Entry, Image, Owner, Table};

/// How long `exit()` may take after the frontend arms the watchdog.
pub const TIMEOUT: Duration = Duration::from_secs(10);

/// The exit code of a process that the watchdog ends (as `timeout(1)`).
pub const EXIT_CODE: i32 = 124;

/// The start of the watchdog's line, for tools that read the log.
pub const LINE_PREFIX: &str = "exit-watchdog: ";

/// The line the watchdog writes when it fires and names no library, with
/// its line end.
pub fn line(timeout: Duration) -> String {
    line_naming(timeout, None)
}

/// The line with the library the armed thread waits in, if one was found.
pub fn line_naming(timeout: Duration, culprit: Option<&Entry>) -> String {
    let mut buf = [0u8; LINE_MAX];
    let n = write_line(&mut buf, timeout, culprit);
    String::from_utf8_lossy(&buf[..n]).into_owned()
}

/// The longest line the watchdog writes (a longer file or module name is cut).
pub const LINE_MAX: usize = 512;

/// Writes the line into `out` without allocating; returns its length.
fn write_line(out: &mut [u8], timeout: Duration, culprit: Option<&Entry>) -> usize {
    const END: &str = "; ending the process\n";
    let room = out.len().saturating_sub(END.len());
    let mut w = Cut {
        buf: &mut out[..room],
        len: 0,
    };
    let _ = write!(w, "{LINE_PREFIX}exit did not finish in ");
    if timeout.subsec_millis() == 0 {
        let _ = write!(w, "{} s", timeout.as_secs());
    } else {
        let _ = write!(w, "{} ms", timeout.as_millis());
    }
    let _ = write!(w, ": a static destructor or atexit handler is blocking");
    if let Some(e) = culprit {
        let _ = write!(w, " in {} (", e.file);
        let _ = match &e.owner {
            Owner::Module(m) if *m == e.file => write!(w, "an OBS module"),
            Owner::Module(m) => write!(w, "loaded by {m}"),
            Owner::PostLoad => write!(w, "loaded during the modules' post-load"),
            Owner::AfterStartup => write!(w, "loaded after startup"),
            Owner::Obs => write!(w, "OBS"),
        };
        let _ = write!(w, ")");
    }
    let len = w.len;
    let end = (len + END.len()).min(out.len());
    out[len..end].copy_from_slice(&END.as_bytes()[..end - len]);
    end
}

/// A `fmt::Write` into a fixed buffer that drops what does not fit, on a
/// UTF-8 character boundary, without allocating.
struct Cut<'a> {
    buf: &'a mut [u8],
    len: usize,
}

impl std::fmt::Write for Cut<'_> {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        let room = self.buf.len() - self.len;
        let mut take = s.len().min(room);
        while !s.is_char_boundary(take) {
            take -= 1;
        }
        self.buf[self.len..self.len + take].copy_from_slice(&s.as_bytes()[..take]);
        self.len += take;
        Ok(())
    }
}

/// The first frame of `frames` (innermost first) in an image that OBS itself
/// did not load: the library to name.
pub fn culprit<'a>(frames: &[usize], table: &'a Table) -> Option<&'a Entry> {
    frames
        .iter()
        .filter_map(|&f| table.find(f))
        .find(|e| e.owner != Owner::Obs)
}

/// What the frontend noted before arming.
struct Notes {
    modules: Vec<String>,
    after_load: Option<Vec<Image>>,
    after_post_load: Option<Vec<Image>>,
}

static NOTES: Mutex<Notes> = Mutex::new(Notes {
    modules: Vec::new(),
    after_load: None,
    after_post_load: None,
});

fn notes() -> std::sync::MutexGuard<'static, Notes> {
    NOTES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The frontend: the binary path of a loaded OBS module.
pub fn note_module(path: &str) {
    notes().modules.push(path.to_string());
}

/// The frontend: the modules are loaded (before their post-load).
pub fn note_loaded() {
    notes().after_load = Some(images::current());
}

/// The frontend: the modules' post-load is done.
pub fn note_post_load() {
    notes().after_post_load = Some(images::current());
}

static ARMED: AtomicBool = AtomicBool::new(false);

/// Arms the watchdog once per process: when the process still runs after
/// `timeout`, it writes [`line`] to stderr and appends it to `log` (if the
/// file opens), then ends the process with [`EXIT_CODE`]. Returns false when
/// the watchdog was already armed or its thread did not start.
pub fn arm(timeout: Duration, log: Option<&Path>) -> bool {
    let at_arm = images::current();
    let table = {
        let n = notes();
        // Without notes, every image is OBS's own and the line names none.
        let after_load = n.after_load.as_deref().unwrap_or(&at_arm);
        let after_post_load = n.after_post_load.as_deref().unwrap_or(after_load);
        Table::build(after_load, after_post_load, &at_arm, &n.modules)
    };
    arm_with(timeout, log, table)
}

/// [`arm`] with a given table (tests).
pub fn arm_with(timeout: Duration, log: Option<&Path>, table: Table) -> bool {
    if ARMED.swap(true, Ordering::SeqCst) {
        return false;
    }
    // A started helper (#4589) ends a hang that this thread cannot see;
    // only one of them is armed, so the line is written once.
    if helper::arm_started() {
        return true;
    }
    let thread = stack::current();
    stack::prepare();
    let file = log.and_then(|path| OpenOptions::new().append(true).open(path).ok());
    let started = std::thread::Builder::new()
        .name("obs-exit-watchdog".into())
        .spawn(move || {
            std::thread::sleep(timeout);
            fire(timeout, &table, &thread, file);
        });
    started.is_ok()
}

/// The deepest stack the watchdog looks at.
const MAX_FRAMES: usize = 64;

/// Captures the armed thread's stack, writes the line and ends the process.
/// Runs while destructors run: no allocation, no lock of the frontend, no
/// `exit()`.
fn fire(timeout: Duration, table: &Table, thread: &stack::Thread, file: Option<File>) -> ! {
    let mut frames = [0usize; MAX_FRAMES];
    let n = stack::capture(thread, &mut frames);
    let mut text = [0u8; LINE_MAX];
    let len = write_line(&mut text, timeout, culprit(&frames[..n], table));
    if let Some(mut file) = file {
        let _ = file.write_all(&text[..len]);
    }
    let _ = std::io::stderr().write_all(&text[..len]);
    platform::terminate(EXIT_CODE)
}
