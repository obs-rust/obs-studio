//! `obs_hotkeys_background_status_changed()` over C-owned state: the core,
//! its hotkey signal handler and `signal_handler_signal()`, behind the
//! private adapters of `libobs/obs-hotkeys-background-status-changed-fields.c.h`.
//!
//! Declared in `obs-internal.h` without `EXPORT`, so both entries stay hidden
//! (`rust-hidden.d/obs-core-hotkeys_background_status.txt`). The decision and
//! the signal name are the safe core's
//! ([`crate::hotkeys_background_status::status_changed`]); the handler is read
//! again on every call and nothing is cached, because the synchronous
//! callbacks may replace it, notify again or remove the core.
use core::ffi::{CStr, c_char, c_void};
use core::ptr::NonNull;

use crate::hotkeys_background_status::{Host, status_changed};

unsafe extern "C" {
    fn obs_hotkeys_background_status_signals() -> *mut c_void;
    fn obs_hotkeys_background_status_signal(handler: *mut c_void, name: *const c_char);
}

struct Core;

impl Host for Core {
    type Handler = NonNull<c_void>;
    fn signals(&mut self) -> Option<NonNull<c_void>> {
        // SAFETY: the adapter checks the core for NULL before reading its
        // handler, as the original's condition does.
        NonNull::new(unsafe { obs_hotkeys_background_status_signals() })
    }
    fn signal(&mut self, handler: NonNull<c_void>, name: &CStr) {
        // SAFETY: the handler was read from the live core just now; the name
        // is NUL-terminated and outlives this synchronous dispatch, and C
        // sends no calldata (NULL), as the original does.
        unsafe { obs_hotkeys_background_status_signal(handler.as_ptr(), name.as_ptr()) }
    }
}

/// Tell the core's hotkey signal handler that the background hotkey status
/// changed; silent without a core or a handler.
#[unsafe(no_mangle)]
pub extern "C" fn obs_hotkeys_background_status_changed() {
    obs_hotkeys_background_status_request();
}

/// Explicit static entry for fixtures with their own private core namespace.
#[unsafe(no_mangle)]
pub extern "C" fn obs_hotkeys_background_status_request() {
    status_changed(&mut Core);
}
