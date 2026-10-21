//! `oracle_*` build of `libobs/callback/signal.c.h` (oracle/signal.c), on
//! the decl oracle. `signal_handler_t` is opaque; the pointers here are
//! only passed back.

use core::ffi::{c_char, c_void};

/// `signal_callback_t`.
pub type SignalCallback = Option<unsafe extern "C" fn(*mut c_void, *mut c_void)>;
/// `global_signal_callback_t`.
pub type GlobalSignalCallback =
    Option<unsafe extern "C" fn(*mut c_void, *const c_char, *mut c_void)>;

unsafe extern "C" {
    pub fn oracle_signal_handler_create() -> *mut c_void;
    pub fn oracle_signal_handler_destroy(handler: *mut c_void);
    pub fn oracle_signal_handler_add(handler: *mut c_void, signal_decl: *const c_char) -> bool;
    pub fn oracle_signal_handler_connect(
        handler: *mut c_void,
        signal: *const c_char,
        callback: SignalCallback,
        data: *mut c_void,
    );
    pub fn oracle_signal_handler_connect_ref(
        handler: *mut c_void,
        signal: *const c_char,
        callback: SignalCallback,
        data: *mut c_void,
    );
    pub fn oracle_signal_handler_disconnect(
        handler: *mut c_void,
        signal: *const c_char,
        callback: SignalCallback,
        data: *mut c_void,
    );
    pub fn oracle_signal_handler_connect_global(
        handler: *mut c_void,
        callback: GlobalSignalCallback,
        data: *mut c_void,
    );
    pub fn oracle_signal_handler_disconnect_global(
        handler: *mut c_void,
        callback: GlobalSignalCallback,
        data: *mut c_void,
    );
    pub fn oracle_signal_handler_remove_current();
    pub fn oracle_signal_handler_signal(
        handler: *mut c_void,
        signal: *const c_char,
        params: *mut c_void,
    );
}
