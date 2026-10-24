//! The shared warning for fallbacks that lower security
//! (obs-rust/obs-studio#2218).
//!
//! Some features fall back to a less safe mode when the safe one cannot
//! work, so that users do not lose them: the browser source without the
//! Chromium sandbox (#1092), RTMPS to RTMP (#2205). As decided on #1092, OBS
//! then logs the error (also on stderr), and warns the user with a countdown
//! that accepts the fallback by itself after 3 seconds.
//!
//! This crate is the pure part: whether a warning shows a dialog, how a
//! dialog ends, the countdown progress and the log line. The frontend draws
//! the dialog (Qt) and calls the C ABI in [`ffi`].

#![deny(unsafe_code)]

use std::collections::HashSet;

#[allow(unsafe_code)]
pub mod ffi;

/// How long the warning counts down before it accepts by itself.
pub const COUNTDOWN_MS: u64 = 3_000;

/// The sentence every fallback warning ends with.
pub const FUTURE_ERROR: &str = "This may become an error in a future version of OBS.";

/// What the frontend does with a warning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Show {
    /// Show the dialog with the countdown.
    Dialog,
    /// No dialog: log only, and continue as with "Accept anyway".
    AcceptNow,
}

/// How a dialog ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum End {
    /// The countdown reached its end.
    CountdownDone,
    /// The user clicked "Accept anyway".
    AcceptClicked,
    /// The user clicked "Cancel".
    CancelClicked,
    /// The user closed the dialog (window close, Escape).
    Closed,
}

/// The warnings that showed a dialog in this session.
#[derive(Debug, Default)]
pub struct Session {
    shown: HashSet<String>,
}

impl Session {
    /// A session in which no warning has shown a dialog.
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether warning `id` shows a dialog. With no visible main window
    /// nobody sees a dialog, so it does not show one and is not remembered.
    /// An `id` that already showed one in this session does not show it
    /// again. An empty `id` is never remembered.
    pub fn begin(&mut self, id: &str, window_visible: bool) -> Show {
        if !window_visible {
            return Show::AcceptNow;
        }
        if id.is_empty() || self.shown.insert(id.to_owned()) {
            Show::Dialog
        } else {
            Show::AcceptNow
        }
    }
}

/// Whether the caller continues in the fallback mode after the dialog
/// ended with `end`. Cancel and Close stop only a caller that can stop
/// (`can_cancel`); for the others there is nothing to stop.
pub fn accepted(end: End, can_cancel: bool) -> bool {
    match end {
        End::CountdownDone | End::AcceptClicked => true,
        End::CancelClicked | End::Closed => !can_cancel,
    }
}

/// The countdown progress after `elapsed_ms`, from 0 to 100.
pub fn progress_percent(elapsed_ms: u64) -> u8 {
    let percent = elapsed_ms.min(COUNTDOWN_MS) * 100 / COUNTDOWN_MS;
    u8::try_from(percent).unwrap_or(100)
}

/// The log line for warning `id`: the message, then [`FUTURE_ERROR`].
/// The frontend logs it at error level, so it is on stderr once.
pub fn log_line(id: &str, message: &str) -> String {
    if message.is_empty() {
        format!("[fallback] {id}: {FUTURE_ERROR}")
    } else {
        format!("[fallback] {id}: {message} {FUTURE_ERROR}")
    }
}
