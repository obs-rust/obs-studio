//! Sleep inhibition on Linux and FreeBSD: `libobs/util/platform-nix-inhibit.c`
//! (obs-rust/obs-studio#1500). macOS is in [`super::cocoa`] and Windows in the
//! Windows host module.
//!
//! The desktop inhibitors stay C, because they need GIO: the
//! xdg-desktop-portal Inhibit request (`util/platform-nix-portal.c`) and the
//! session D-Bus screensaver and power services (`util/platform-nix-dbus.c`).
//! This core picks one of them for each inhibitor, in the C's order, and runs
//! the `xdg-screensaver reset` thread when there is neither. It reaches the C
//! and the OS through [`Host`]: the C ABI shim's host calls the C functions,
//! and the tests count the calls.
//!
//! Whether GIO was found (`GIO_FOUND` in `obsconfig.h`) is [`Host::GIO`],
//! a constant from the `gio` Cargo feature that CMake sets (#453 dropped
//! these inhibitors once; it is never decided at run time).
//!
//! Intentional differences from the C:
//! - The inhibitor is a `Box` in the shim and the reason a `CString`; C
//!   uses `bzalloc` and `bstrdup`.
//! - A thread that cannot be started returns false and leaves the inhibitor
//!   inactive. C checks `pthread_create(...) < 0`, which never holds, so it
//!   returns true and later joins a thread that does not exist.
//! - The reset retries its wait for `xdg-screensaver` only when interrupted.
//!   C retries any `waitpid` failure, so with `SIGCHLD` ignored (`ECHILD`) it
//!   never stops trying.

use std::ffi::{CStr, CString, OsStr};
use std::path::Path;
use std::sync::Arc;
use std::thread::JoinHandle;

use super::event::Event;
use crate::base::{LOG_ERROR, LOG_WARNING};
use crate::platform::EventType;

/// How long the fallback thread waits between resets, in milliseconds.
pub const RESET_INTERVAL_MS: u64 = 30_000;
/// The program the fallback thread runs, found on `PATH`.
pub const SCREENSAVER_PROGRAM: &str = "xdg-screensaver";
/// Its arguments after `argv[0]`.
pub const SCREENSAVER_ARGS: [&str; 1] = ["reset"];
/// Present inside a Flatpak, where only the portal may inhibit.
pub const FLATPAK_INFO: &str = "/.flatpak-info";

/// What the core calls outside itself. The defaults are the real OS; the
/// GIO inhibitors have no default.
pub trait Host: Send + Sync + 'static {
    /// `GIO_FOUND`: false is the C compiled without the portal and D-Bus
    /// branches, which then never calls the methods below up to `log`.
    const GIO: bool;
    /// `struct portal_inhibit_info *`.
    type Portal;
    /// `struct dbus_sleep_info *`.
    type Dbus;

    /// `portal_inhibit_info_create`; `None` when there is no portal.
    fn portal_create(&self) -> Option<Self::Portal>;
    /// `dbus_sleep_info_create`; `None` when no session inhibitor is found.
    fn dbus_create(&self) -> Option<Self::Dbus>;
    /// `portal_inhibit`.
    fn portal_inhibit(&self, portal: &mut Self::Portal, reason: Option<&CStr>, active: bool);
    /// `dbus_inhibit_sleep`.
    fn dbus_inhibit(&self, dbus: &mut Self::Dbus, reason: Option<&CStr>, active: bool);
    /// `portal_inhibit_info_destroy`.
    fn portal_destroy(&self, portal: Self::Portal);
    /// `dbus_sleep_info_destroy`.
    fn dbus_destroy(&self, dbus: Self::Dbus);
    /// `blog(level, "%s", message)`.
    fn log(&self, level: i32, message: &str);

    /// `access("/.flatpak-info", F_OK) == 0`.
    fn in_flatpak(&self) -> bool {
        Path::new(FLATPAK_INFO).exists()
    }

    /// Runs `program` with `args` and waits for it; `Err(errno)` when it
    /// cannot start (the C's `posix_spawnp` result).
    fn spawn(&self, program: &str, args: &[&str]) -> Result<(), i32> {
        run_and_wait(OsStr::new(program), args)
    }

    /// The fallback thread's wait: true when `stop` was signalled, false
    /// after `milliseconds` (`os_event_timedwait(...) == ETIMEDOUT`).
    fn wait_for_stop(&self, stop: &Event, milliseconds: u64) -> bool {
        stop.timed_wait(milliseconds)
    }

    /// Starts the fallback thread (`pthread_create`).
    fn spawn_thread(&self, body: Box<dyn FnOnce() + Send>) -> std::io::Result<JoinHandle<()>> {
        std::thread::Builder::new().spawn(body)
    }
}

/// Runs `program` with `args`, inheriting stdio and the environment, and
/// waits for it to exit. `Err(errno)` when it cannot start.
///
/// std's `Command` starts the child with an empty signal mask and `SIGPIPE`
/// at its default, as the C's `posix_spawnattr_t` does, and `argv[0]` is
/// `program`. Its wait is retried when interrupted.
pub fn run_and_wait(program: &OsStr, args: &[&str]) -> Result<(), i32> {
    let mut child = std::process::Command::new(program)
        .args(args)
        .spawn()
        .map_err(|e| e.raw_os_error().unwrap_or(-1))?;
    // The exit status is ignored, as in C.
    let _ = child.wait();
    Ok(())
}

/// `reset_screensaver`: one `xdg-screensaver reset`, logged when it cannot
/// start.
fn reset_screensaver<H: Host>(host: &H) {
    if let Err(err) = host.spawn(SCREENSAVER_PROGRAM, &SCREENSAVER_ARGS) {
        host.log(
            LOG_WARNING,
            &format!("Failed to create xdg-screensaver: {err}"),
        );
    }
}

/// `screensaver_thread`: resets every [`RESET_INTERVAL_MS`] until stopped.
fn screensaver_thread<H: Host>(host: &H, stop: &Event) {
    while !host.wait_for_stop(stop, RESET_INTERVAL_MS) {
        reset_screensaver(host);
    }
}

/// The xdg-screensaver fallback: `stop_event` and `screensaver_thread`.
struct Fallback {
    stop: Arc<Event>,
    thread: Option<JoinHandle<()>>,
}

/// `struct os_inhibit_info`.
pub struct Inhibit<H: Host> {
    host: Arc<H>,
    portal: Option<H::Portal>,
    dbus: Option<H::Dbus>,
    /// `Some` exactly when the C has a `stop_event`.
    fallback: Option<Fallback>,
    reason: Option<CString>,
    active: bool,
}

impl<H: Host> Inhibit<H> {
    /// `os_inhibit_sleep_create`. `None` only inside a Flatpak with no
    /// portal, where nothing else may inhibit.
    pub fn create(host: Arc<H>, reason: Option<&CStr>) -> Option<Self> {
        let mut info = Self {
            host,
            portal: None,
            dbus: None,
            fallback: None,
            reason: None,
            active: false,
        };

        if H::GIO {
            info.portal = info.host.portal_create();
            if info.portal.is_none() {
                if info.host.in_flatpak() {
                    return None;
                }
                info.dbus = info.host.dbus_create();
            }
            if info.portal.is_some() || info.dbus.is_some() {
                info.reason = reason.map(CStr::to_owned);
                return Some(info);
            }
        }

        info.fallback = Some(Fallback {
            stop: Arc::new(Event::new(EventType::Auto)),
            thread: None,
        });
        info.reason = reason.map(CStr::to_owned);
        Some(info)
    }

    /// The reason given to [`Self::create`], copied.
    pub fn reason(&self) -> Option<&CStr> {
        self.reason.as_deref()
    }

    /// Whether the last successful [`Self::set_active`] activated it.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// `os_inhibit_sleep_set_active`: true when the state changed.
    pub fn set_active(&mut self, active: bool) -> bool {
        if self.active == active {
            return false;
        }

        if H::GIO {
            let reason = self.reason.as_deref();
            if let Some(portal) = self.portal.as_mut() {
                self.host.portal_inhibit(portal, reason, active);
            }
            if let Some(dbus) = self.dbus.as_mut() {
                self.host.dbus_inhibit(dbus, reason, active);
            }
            if self.portal.is_some() || self.dbus.is_some() {
                self.active = active;
                return true;
            }
        }

        // C: `if (!info->stop_event) return true;`, which no inhibitor
        // reaches: without a desktop inhibitor create always made one.
        let Some(fallback) = self.fallback.as_mut() else {
            return true;
        };

        if active {
            let host = Arc::clone(&self.host);
            let stop = Arc::clone(&fallback.stop);
            match self
                .host
                .spawn_thread(Box::new(move || screensaver_thread(&*host, &stop)))
            {
                Ok(thread) => fallback.thread = Some(thread),
                Err(_) => {
                    self.host
                        .log(LOG_ERROR, "Failed to create screensaver inhibitor thread");
                    return false;
                }
            }
        } else {
            fallback.stop.signal();
            if let Some(thread) = fallback.thread.take() {
                // A panic in a test host's hook is not the inhibitor's.
                let _ = thread.join();
            }
        }

        self.active = active;
        true
    }

    /// `os_inhibit_sleep_destroy`: deactivates, then releases the
    /// inhibitor it used.
    pub fn destroy(mut self) {
        self.set_active(false);
        if H::GIO {
            if let Some(portal) = self.portal.take() {
                self.host.portal_destroy(portal);
            } else if let Some(dbus) = self.dbus.take() {
                self.host.dbus_destroy(dbus);
            }
        }
    }
}

/// `os_inhibit_sleep_set_active` with the C's `NULL` check.
pub fn set_active<H: Host>(info: Option<&mut Inhibit<H>>, active: bool) -> bool {
    info.is_some_and(|info| info.set_active(active))
}

/// `os_inhibit_sleep_destroy`; `None` does nothing.
pub fn destroy<H: Host>(info: Option<Inhibit<H>>) {
    if let Some(info) = info {
        info.destroy();
    }
}
