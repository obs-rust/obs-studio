//! C ABI for the `signal_handler_*` functions in `libobs/callback/signal.h`.
//!
//! `signal_handler_t` is opaque in the header. The handle is one `bmalloc`
//! block that holds an `Arc<SignalHandler>` from `Arc::into_raw`. That
//! block is the allocation `bnum_allocs` counts, so `test_signal.c` sees
//! the handler live until its last reference goes, as in C. The C
//! reference count is [`SignalHandler::refs`]: the `Arc` is dropped and the
//! block freed when that count reaches zero. `signal_handler_signal` holds
//! its own `Arc` clone while callbacks run, so a callback that destroys the
//! handler does not free it under the running pass (C would). When the
//! pass gives back the last `keep_ref` reference, the handler is freed
//! after the pass, as in C. Signals, decl names and callback lists are
//! Rust values and are not counted.
//!
//! The log texts are the C ones. Intentional differences, all where C
//! dereferences null: a null handler in `signal_handler_add` returns false;
//! a null signal name is not found (the warning prints `(null)`, as glibc
//! does); a null callback is stored but not called.

use std::ffi::{CStr, c_char, c_int, c_void};
use std::sync::Arc;

use obs_util::ffi::bmem::{bfree, bmalloc};

use crate::ffi::calldata::calldata;
use crate::signal::{AddResult, CallbackKey, SignalHandler, remove_current};

const LOG_WARNING: c_int = 200;
const LOG_ERROR: c_int = 100;

unsafe extern "C" {
    fn blog(level: c_int, fmt: *const c_char, ...);
}

/// `struct signal_handler`: opaque to C.
#[repr(C)]
pub struct signal_handler {
    _opaque: [u8; 0],
}

/// `signal_callback_t`.
#[allow(non_camel_case_types)]
pub type signal_callback_t = Option<unsafe extern "C" fn(*mut c_void, *mut calldata)>;
/// `global_signal_callback_t`.
#[allow(non_camel_case_types)]
pub type global_signal_callback_t =
    Option<unsafe extern "C" fn(*mut c_void, *const c_char, *mut calldata)>;

/// The `Arc::into_raw` pointer that the handle block holds.
///
/// # Safety
///
/// `handler` is a live, non-null pointer from `signal_handler_create`.
unsafe fn inner(handler: *mut signal_handler) -> *const SignalHandler {
    // SAFETY: the block holds one pointer, written by create.
    unsafe { handler.cast::<*const SignalHandler>().read() }
}

/// # Safety
///
/// `handler` is null or a live pointer from `signal_handler_create`.
unsafe fn handler_ref<'a>(handler: *mut signal_handler) -> Option<&'a SignalHandler> {
    if handler.is_null() {
        return None;
    }
    // SAFETY: the caller guarantees a live handle; its `Arc` lives as long.
    unsafe { inner(handler).as_ref() }
}

/// # Safety
///
/// `handler` came from `signal_handler_create`, and this was its last C
/// reference.
unsafe fn free_handler(handler: *mut signal_handler) {
    // SAFETY: the block holds the `Arc::into_raw` of create; the C count is
    // zero, so this drops C's share. The block is `bmalloc` from create.
    unsafe {
        drop(Arc::from_raw(inner(handler)));
        bfree(handler.cast());
    }
}

/// # Safety
///
/// `s` is null or NUL-terminated.
unsafe fn bytes<'a>(s: *const c_char) -> Option<&'a [u8]> {
    // SAFETY: the caller guarantees a NUL-terminated string or null.
    (!s.is_null()).then(|| unsafe { CStr::from_ptr(s) }.to_bytes())
}

/// `blog(level, "%s", text)` for bytes without a NUL.
fn log(level: c_int, text: &[u8]) {
    let mut text = text.to_vec();
    text.push(0);
    // SAFETY: "%s" takes one NUL-terminated string, and `text` is one.
    unsafe { blog(level, c"%s".as_ptr(), text.as_ptr()) };
}

fn quoted(s: Option<&[u8]>) -> &[u8] {
    s.unwrap_or(b"(null)")
}

fn signal_key(callback: signal_callback_t, data: *mut c_void) -> CallbackKey {
    CallbackKey {
        func: callback.map_or(0, |f| f as usize),
        data: data as usize,
    }
}

fn global_key(callback: global_signal_callback_t, data: *mut c_void) -> CallbackKey {
    CallbackKey {
        func: callback.map_or(0, |f| f as usize),
        data: data as usize,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn signal_handler_create() -> *mut signal_handler {
    let ptr = Arc::into_raw(Arc::new(SignalHandler::new()));
    // SAFETY: one pointer-sized block. `bmalloc` aborts instead of
    // returning null, as C's `bzalloc` in create does.
    unsafe {
        let block = bmalloc(size_of::<*const SignalHandler>()).cast::<*const SignalHandler>();
        block.write(ptr);
        block.cast()
    }
}

/// # Safety
///
/// `handler` is null or a live pointer from `signal_handler_create`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn signal_handler_destroy(handler: *mut signal_handler) {
    // SAFETY: forwarded from the caller.
    let Some(h) = (unsafe { handler_ref(handler) }) else {
        return;
    };
    if h.release() {
        // SAFETY: that was the last C reference.
        unsafe { free_handler(handler) };
    }
}

/// # Safety
///
/// `handler` is null or live. `signal_decl` is null or NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn signal_handler_add(
    handler: *mut signal_handler,
    signal_decl: *const c_char,
) -> bool {
    // SAFETY: forwarded from the caller.
    let Some(h) = (unsafe { handler_ref(handler) }) else {
        return false;
    };
    // SAFETY: forwarded from the caller.
    let decl = unsafe { bytes(signal_decl) };
    match h.add(decl.unwrap_or_default()) {
        AddResult::Added => true,
        AddResult::Invalid(parse) => {
            if let Some(warning) = parse.warning(decl.unwrap_or_default()) {
                log(LOG_WARNING, &warning);
            }
            let mut text = b"Signal declaration invalid: ".to_vec();
            text.extend_from_slice(quoted(decl));
            log(LOG_ERROR, &text);
            false
        }
        AddResult::Exists(name) => {
            let mut text = b"Signal declaration '".to_vec();
            text.extend_from_slice(&name);
            text.extend_from_slice(b"' exists");
            log(LOG_WARNING, &text);
            false
        }
    }
}

/// # Safety
///
/// `handler` is null or live. `signal` is null or NUL-terminated.
unsafe fn connect(
    handler: *mut signal_handler,
    signal: *const c_char,
    key: CallbackKey,
    keep_ref: bool,
) {
    // SAFETY: forwarded from the caller.
    let Some(h) = (unsafe { handler_ref(handler) }) else {
        return;
    };
    // SAFETY: forwarded from the caller.
    let name = unsafe { bytes(signal) };
    let found = name.is_some_and(|n| h.connect(n, key, keep_ref));
    if !found {
        let mut text = b"signal_handler_connect: signal '".to_vec();
        text.extend_from_slice(quoted(name));
        text.extend_from_slice(b"' not found");
        log(LOG_WARNING, &text);
    }
}

/// # Safety
///
/// `handler` is null or live. `signal` is null or NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn signal_handler_connect(
    handler: *mut signal_handler,
    signal: *const c_char,
    callback: signal_callback_t,
    data: *mut c_void,
) {
    // SAFETY: forwarded from the caller.
    unsafe { connect(handler, signal, signal_key(callback, data), false) };
}

/// # Safety
///
/// `handler` is null or live. `signal` is null or NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn signal_handler_connect_ref(
    handler: *mut signal_handler,
    signal: *const c_char,
    callback: signal_callback_t,
    data: *mut c_void,
) {
    // SAFETY: forwarded from the caller.
    unsafe { connect(handler, signal, signal_key(callback, data), true) };
}

/// # Safety
///
/// `handler` is null or live. `signal` is null or NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn signal_handler_disconnect(
    handler: *mut signal_handler,
    signal: *const c_char,
    callback: signal_callback_t,
    data: *mut c_void,
) {
    // SAFETY: forwarded from the caller.
    let Some(h) = (unsafe { handler_ref(handler) }) else {
        return;
    };
    // SAFETY: forwarded from the caller.
    let Some(name) = (unsafe { bytes(signal) }) else {
        return;
    };
    if h.disconnect(name, signal_key(callback, data)) {
        // SAFETY: that was the last C reference.
        unsafe { free_handler(handler) };
    }
}

/// # Safety
///
/// `handler` is null or live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn signal_handler_connect_global(
    handler: *mut signal_handler,
    callback: global_signal_callback_t,
    data: *mut c_void,
) {
    // SAFETY: forwarded from the caller.
    let Some(h) = (unsafe { handler_ref(handler) }) else {
        return;
    };
    if callback.is_some() {
        h.connect_global(global_key(callback, data));
    }
}

/// # Safety
///
/// `handler` is null or live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn signal_handler_disconnect_global(
    handler: *mut signal_handler,
    callback: global_signal_callback_t,
    data: *mut c_void,
) {
    // SAFETY: forwarded from the caller.
    let Some(h) = (unsafe { handler_ref(handler) }) else {
        return;
    };
    if callback.is_some() {
        h.disconnect_global(global_key(callback, data));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn signal_handler_remove_current() {
    remove_current();
}

/// # Safety
///
/// `handler` is null or live. `signal` is null or NUL-terminated and lives
/// for the call. The stored callbacks are valid `signal_callback_t` /
/// `global_signal_callback_t` functions for their data.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn signal_handler_signal(
    handler: *mut signal_handler,
    signal: *const c_char,
    params: *mut calldata,
) {
    if handler.is_null() {
        return;
    }
    // SAFETY: forwarded from the caller.
    let Some(name) = (unsafe { bytes(signal) }) else {
        return;
    };
    // SAFETY: `handler` is a live handle that holds an `Arc::into_raw`
    // pointer. The extra strong reference keeps it alive while callbacks
    // run, even if one of them drops the last C reference.
    let h = unsafe {
        let ptr = inner(handler);
        Arc::increment_strong_count(ptr);
        Arc::from_raw(ptr)
    };
    let last = h.signal(
        name,
        |key| {
            if key.func == 0 {
                return;
            }
            // SAFETY: `func` is a non-null `signal_callback_t` that C passed
            // to connect; the caller vouches for it and its data.
            unsafe {
                let f: unsafe extern "C" fn(*mut c_void, *mut calldata) =
                    std::mem::transmute(key.func);
                f(key.data as *mut c_void, params);
            }
        },
        |key| {
            // SAFETY: as above, for a non-null `global_signal_callback_t`.
            unsafe {
                let f: unsafe extern "C" fn(*mut c_void, *const c_char, *mut calldata) =
                    std::mem::transmute(key.func);
                f(key.data as *mut c_void, signal, params);
            }
        },
    );
    if last {
        // SAFETY: that was the last C reference. `h` keeps the handler
        // alive until this function returns.
        unsafe { free_handler(handler) };
    }
}
