//! Field-address conversion only; the private output layout stays in C.
use crate::output_handler_getters::{self, Handlers};
use core::ffi::{CStr, c_char, c_int, c_void};

#[repr(C)]
pub struct Fields {
    pub signal: *const *mut c_void,
    pub proc: *const *mut c_void,
}

unsafe extern "C" {
    fn obs_output_handler_getters_fields(output: *const c_void) -> Fields;
    fn blog(level: c_int, format: *const c_char, ...);
}

unsafe fn fields(output: *const c_void) -> Option<Handlers<*mut c_void>> {
    if output.is_null() {
        return None;
    }
    // SAFETY: null is rejected before taking C member addresses; the caller
    // keeps this output and its handler fields readable throughout the call.
    unsafe {
        let fields = obs_output_handler_getters_fields(output);
        Some(Handlers {
            signal: *fields.signal,
            proc: *fields.proc,
        })
    }
}

fn log(name: &'static CStr) {
    // SAFETY: the format is a literal and name is a static, terminated string.
    unsafe { blog(400, c"%s: Null 'output' parameter".as_ptr(), name.as_ptr()) }
}

/// # Safety
/// A non-null output must remain live and readable under the original C contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_get_signal_handler(output: *const c_void) -> *mut c_void {
    // SAFETY: the caller supplies a live output or null; fields rejects null first.
    output_handler_getters::signal(unsafe { fields(output) }, log)
}

/// # Safety
/// A non-null output must remain live and readable under the original C contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_get_proc_handler(output: *const c_void) -> *mut c_void {
    // SAFETY: the caller supplies a live output or null; fields rejects null first.
    output_handler_getters::proc(unsafe { fields(output) }, log)
}
