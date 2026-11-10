//! The exit watchdog (obs-rust/obs-studio#4065).
//!
//! OBS can finish its own shutdown and still not quit: a static destructor
//! or an `atexit` handler of a plugin blocks in `exit()`, after `main()` has
//! returned (#3739). The frontend arms this watchdog after its last log
//! line. When the process still runs after the timeout, the watchdog writes
//! one line to the log file and to stderr and ends the process with
//! [`EXIT_CODE`].
//!
//! All work is done when the watchdog is armed: when it fires, `main()` has
//! returned, the frontend's log file is closed and destructors run. Firing
//! only writes the prepared line and ends the process without `exit()`
//! (which would run the same destructors again) and without `abort()`
//! (which looks like a crash). The host calls are in `platform`.

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

/// How long `exit()` may take after the frontend arms the watchdog.
pub const TIMEOUT: Duration = Duration::from_secs(10);

/// The exit code of a process that the watchdog ends (as `timeout(1)`).
pub const EXIT_CODE: i32 = 124;

/// The start of the watchdog's line, for tools that read the log.
pub const LINE_PREFIX: &str = "exit-watchdog: ";

/// The line the watchdog writes when it fires, with its line end.
pub fn line(timeout: Duration) -> String {
    let after = if timeout.subsec_millis() == 0 {
        format!("{} s", timeout.as_secs())
    } else {
        format!("{} ms", timeout.as_millis())
    };
    format!(
        "{LINE_PREFIX}exit did not finish in {after}: a static destructor or \
         atexit handler is blocking; ending the process\n"
    )
}

static ARMED: AtomicBool = AtomicBool::new(false);

/// Arms the watchdog once per process: when the process still runs after
/// `timeout`, it writes [`line`] to stderr and appends it to `log` (if the
/// file opens), then ends the process with [`EXIT_CODE`]. Returns false when
/// the watchdog was already armed or its thread did not start.
pub fn arm(timeout: Duration, log: Option<&Path>) -> bool {
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
