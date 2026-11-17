//! Field-address conversion only; the private service layout stays in C.
use crate::service_handler_getters::{self, Handlers};
use core::ffi::{CStr, c_char, c_int, c_void};

#[repr(C)]
pub struct Fields {
    pub signal: *const *mut c_void,
    pub proc: *const *mut c_void,
}

unsafe extern "C" {
    fn obs_service_handler_getters_fields(service: *const c_void) -> Fields;
    fn blog(level: c_int, format: *const c_char, ...);
}

unsafe fn fields(service: *const c_void) -> Option<Handlers<*mut c_void>> {
    if service.is_null() {
        return None;
    }
    // SAFETY: null is rejected before taking C member addresses; the caller
    // keeps this service and its handler fields readable throughout the call.
    unsafe {
        let fields = obs_service_handler_getters_fields(service);
        Some(Handlers {
            signal: *fields.signal,
            proc: *fields.proc,
        })
    }
}

fn log(name: &'static CStr) {
    // SAFETY: the format is a literal and name is a static, terminated string.
    unsafe {
        blog(
            400,
            c"%s: Null '%s' parameter".as_ptr(),
            name.as_ptr(),
            c"service".as_ptr(),
        )
    }
}

/// # Safety
/// A non-null service must remain live and readable under the original C contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_service_get_signal_handler(service: *const c_void) -> *mut c_void {
    // SAFETY: the caller supplies a live service or null; fields rejects null first.
    service_handler_getters::signal(unsafe { fields(service) }, log)
}

/// # Safety
/// A non-null service must remain live and readable under the original C contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_service_get_proc_handler(service: *const c_void) -> *mut c_void {
    // SAFETY: the caller supplies a live service or null; fields rejects null first.
    service_handler_getters::proc(unsafe { fields(service) }, log)
}
