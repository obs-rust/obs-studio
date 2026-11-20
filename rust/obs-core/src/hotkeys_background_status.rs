//! Safe core of `libobs/obs-hotkeys-background-status-changed.c.h`:
//! `obs_hotkeys_background_status_changed()`, which tells the core's hotkey
//! signal handler that the background hotkey status changed.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable
)]

use core::ffi::CStr;

/// The notification's name, exactly as the C sends it.
pub const SIGNAL: &CStr = c"hotkey_background_status_changed";

/// The core's hotkey signal handler as the notification sees it: `obs`,
/// `obs->hotkeys.signals` and `signal_handler_signal()`.
pub trait Host {
    /// A `signal_handler_t *`, compared by identity only.
    type Handler: Copy;
    /// One read of the live core's hotkey signal handler: `None` when there
    /// is no core or the core has no handler.
    fn signals(&mut self) -> Option<Self::Handler>;
    /// `signal_handler_signal(handler, name, NULL)`, synchronously. A
    /// callback may notify again or remove the core before it returns.
    fn signal(&mut self, handler: Self::Handler, name: &CStr);
}

/// Notify the current hotkey signal handler, if the core has one.
///
/// The handler is read once, when the call starts, and nothing is read after
/// the signal: a callback that replaced the handler or removed the core
/// affects only the next call (or the one it makes itself).
pub fn status_changed<H: Host>(host: &mut H) {
    if let Some(handler) = host.signals() {
        host.signal(handler, SIGNAL);
    }
}
