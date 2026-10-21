//! Safe signal handler: the broadcast table of `libobs/callback/signal.c.h`.
//!
//! A handler holds named signals, each with an ordered list of callbacks,
//! and a list of global callbacks that see every signal. Callbacks are
//! opaque [`CallbackKey`] values (function and data addresses); the caller
//! invokes them, so this core runs no foreign code itself.
//!
//! Locking is the C locking. Each signal, and the global list, has a
//! re-entrant lock that is held while its callbacks run. A callback may
//! connect, disconnect, signal or call [`remove_current`] on the same
//! handler from its own thread. Another thread that disconnects waits
//! until the running pass is done, so after `disconnect` returns, that
//! callback is not running.
//!
//! The running callback is a thread-local (signal, index) pair, not a
//! pointer into the callback array. In C, `current_signal_cb` points into
//! the array, and a connect from inside the callback can reallocate it.
//! Here a connect only pushes, and the index stays valid.
//!
//! As in C (pinned by `test/cmocka/test_signal.c`):
//! - A callback connected while signalling runs in the same pass.
//! - The running callback is the innermost one. A signal callback clears
//!   the running global callback and a global callback clears the running
//!   signal callback; both are restored when the callback returns, so a
//!   [`remove_current`] after a nested signal marks the outer callback.
//! - Passes of one signal nest by count. Removed callbacks are erased only
//!   when the outermost pass ends.
//! - Global callbacks are found by callback and data, so one that
//!   disconnects itself while it runs is marked and erased after the pass.
//! - A `keep_ref` reference given back after a pass destroys the handler
//!   when it was the last one.
//!
//! Characterized, not endorsed: an unknown signal returns before the global
//! callbacks.

use crate::decl::Decl;
use crate::decl::DeclParse;

/// A callback as the C API names it: function and data addresses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CallbackKey {
    pub func: usize,
    pub data: usize,
}

/// Result of [`SignalHandler::add`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AddResult {
    Added,
    /// The declaration did not parse. `parse.warning` is what decl logs.
    Invalid(DeclParse),
    /// A signal with this name exists.
    Exists(Vec<u8>),
}

/// `signal_handler_remove_current`.
pub fn remove_current() {
    todo!()
}

/// `struct signal_handler`, without its storage: the shim owns that.
pub struct SignalHandler {}

impl Default for SignalHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl SignalHandler {
    pub fn new() -> Self {
        todo!()
    }
    pub fn refs(&self) -> i64 {
        todo!()
    }
    pub fn release(&self) -> bool {
        todo!()
    }
    pub fn add(&self, _decl_string: &[u8]) -> AddResult {
        todo!()
    }
    pub fn decl(&self, _name: &[u8]) -> Option<Decl> {
        todo!()
    }
    pub fn connect(&self, _signal: &[u8], _key: CallbackKey, _keep_ref: bool) -> bool {
        todo!()
    }
    pub fn disconnect(&self, _signal: &[u8], _key: CallbackKey) -> bool {
        todo!()
    }
    pub fn connect_global(&self, _key: CallbackKey) {
        todo!()
    }
    pub fn disconnect_global(&self, _key: CallbackKey) {
        todo!()
    }
    pub fn signal(
        &self,
        _signal: &[u8],
        _call: impl FnMut(CallbackKey),
        _call_global: impl FnMut(CallbackKey),
    ) -> bool {
        todo!()
    }
}
