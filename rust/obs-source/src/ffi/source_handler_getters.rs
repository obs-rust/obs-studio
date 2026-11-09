//! Field-address conversion only; the private source layout stays in C.
use crate::handler_getters;
use core::ffi::{CStr, c_char, c_int, c_void};

#[repr(C)]
pub struct Fields {
    pub proc: *const *mut c_void,
}

unsafe extern "C" {
    fn obs_source_handler_getters_fields(source: *const c_void) -> Fields;
    fn blog(level: c_int, format: *const c_char, ...);
}

unsafe fn fields(source: *const c_void) -> Option<*mut c_void> {
    if source.is_null() {
        return None;
    }
    // SAFETY: null is rejected before taking C member addresses; the caller
    // keeps this source and its handler fields readable throughout the call.
    unsafe {
        let fields = obs_source_handler_getters_fields(source);
        Some(*fields.proc)
    }
}

fn log(name: &'static CStr) {
    // SAFETY: the format is a literal and name is a static, terminated string.
    unsafe { blog(400, c"%s: Null 'source' parameter".as_ptr(), name.as_ptr()) }
}

/// # Safety
/// A non-null source must remain live and readable under the original C contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_get_proc_handler(source: *const c_void) -> *mut c_void {
    // SAFETY: the caller supplies a live source or null; fields rejects null first.
    handler_getters::proc(unsafe { fields(source) }, log)
}
