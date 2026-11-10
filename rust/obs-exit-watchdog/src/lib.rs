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

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

#[allow(unsafe_code)]
pub mod ffi;
pub mod images;
mod platform;
pub mod stack;

use images::{Entry, Table};

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
    let _ = (out, timeout, culprit);
    todo!()
}

/// The first frame of `frames` (innermost first) in an image that OBS itself
/// did not load: the library to name.
pub fn culprit<'a>(frames: &[usize], table: &'a Table) -> Option<&'a Entry> {
    let _ = (frames, table);
    todo!()
}

/// The frontend: the binary path of a loaded OBS module.
pub fn note_module(path: &str) {
    let _ = path;
    todo!()
}

/// The frontend: the modules are loaded (before their post-load).
pub fn note_loaded() {
    todo!()
}

/// The frontend: the modules' post-load is done.
pub fn note_post_load() {
    todo!()
}

static ARMED: AtomicBool = AtomicBool::new(false);

/// Arms the watchdog once per process: when the process still runs after
/// `timeout`, it writes [`line`] to stderr and appends it to `log` (if the
/// file opens), then ends the process with [`EXIT_CODE`]. Returns false when
/// the watchdog was already armed or its thread did not start.
pub fn arm(timeout: Duration, log: Option<&Path>) -> bool {
    let _ = (timeout, log);
    todo!()
}

/// [`arm`] with a given table (tests).
pub fn arm_with(timeout: Duration, log: Option<&Path>, table: Table) -> bool {
    let _ = table;
    if ARMED.swap(true, Ordering::SeqCst) {
        return false;
    }
    let text = line(timeout);
    let file = log.and_then(|path| OpenOptions::new().append(true).open(path).ok());
    let started = std::thread::Builder::new()
        .name("obs-exit-watchdog".into())
        .spawn(move || {
            std::thread::sleep(timeout);
            fire(&text, file);
        });
    started.is_ok()
}

/// Writes the prepared line and ends the process. Runs while destructors
/// run: no allocation, no lock of the frontend, no `exit()`.
fn fire(text: &str, file: Option<File>) -> ! {
    if let Some(mut file) = file {
        let _ = file.write_all(text.as_bytes());
    }
    let _ = std::io::stderr().write_all(text.as_bytes());
    platform::terminate(EXIT_CODE)
}
