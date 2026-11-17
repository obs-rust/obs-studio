//! Private canvas helpers. All layout access remains in C or existing getters.
use crate::canvas_signals::{self as core, CAPACITY, Host, Route};
use obs_callback::ffi::calldata::calldata;
use obs_callback::ffi::signal::{signal_handler, signal_handler_signal};
use std::ffi::{CStr, c_char, c_void};

unsafe extern "C" {
    fn obs_canvas_signals_private(canvas: *const c_void) -> bool;
}

struct CanvasHost {
    canvas: *mut c_void,
    stack: [u8; CAPACITY],
    data: calldata,
}
impl CanvasHost {
    fn new(canvas: *mut c_void) -> Self {
        Self {
            canvas,
            stack: [0; CAPACITY],
            data: calldata {
                stack: std::ptr::null_mut(),
                size: 0,
                capacity: CAPACITY,
                fixed: true,
            },
        }
    }
}
impl Host for CanvasHost {
    fn init(&mut self, source: Option<usize>) {
        self.data.size = core::payload(&mut self.stack, self.canvas.expose_provenance(), source);
        self.data.stack = self.stack.as_mut_ptr();
    }
    fn private(&self) -> bool {
        // SAFETY: the entry point's caller retains a non-null live canvas.
        unsafe { obs_canvas_signals_private(self.canvas) }
    }
    fn signal(&mut self, route: Route, name: &CStr) {
        // SAFETY: the caller keeps the core and canvas alive. Resolve each
        // handler at its original dispatch point, after previous callbacks.
        unsafe {
            let handler = match route {
                Route::Global => super::global_getters::obs_get_signal_handler(),
                Route::Local => {
                    super::canvas_readonly_getters::obs_canvas_get_signal_handler(self.canvas)
                }
            };
            signal_handler_signal(
                handler.cast::<signal_handler>(),
                name.as_ptr(),
                &mut self.data,
            );
        }
    }
}

/// # Safety
/// `canvas` is non-null and live, the core is initialized, and non-null names
/// are NUL-terminated. Synchronous callbacks follow the original C lifetimes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn canvas_dosignal(
    canvas: *mut c_void,
    global: *const c_char,
    local: *const c_char,
) {
    // SAFETY: optional C strings are valid for this synchronous call.
    let global = if global.is_null() {
        None
    } else {
        Some(unsafe { CStr::from_ptr(global) })
    };
    // SAFETY: as above for the local name.
    let local = if local.is_null() {
        None
    } else {
        Some(unsafe { CStr::from_ptr(local) })
    };
    core::emit(&mut CanvasHost::new(canvas), global, local);
}

/// # Safety
/// Same canvas/core contract as `canvas_dosignal`; `name` is NUL-terminated.
/// `source` is borrowed for the synchronous callbacks and may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn canvas_dosignal_source(
    name: *const c_char,
    canvas: *mut c_void,
    source: *mut c_void,
) {
    // SAFETY: source helper callers supply a live NUL-terminated signal name.
    core::emit_source(
        &mut CanvasHost::new(canvas),
        unsafe { CStr::from_ptr(name) },
        source.expose_provenance(),
    );
}
