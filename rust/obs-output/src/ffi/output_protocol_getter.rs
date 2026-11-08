//! Private field conversion; the output layout remains in C.
use crate::output_protocols::{self, Protocols};
use core::ffi::{CStr, c_char, c_int, c_void};

#[repr(C)]
pub struct Fields {
    pub flags: *const u32,
    pub protocols: *const *const c_char,
}

unsafe extern "C" {
    fn obs_output_protocol_getter_fields(output: *const c_void) -> Fields;
    fn blog(level: c_int, format: *const c_char, ...);
}

unsafe fn fields(output: *const c_void) -> Option<Protocols<*const c_char>> {
    if output.is_null() {
        return None;
    }
    // SAFETY: null is rejected before C member addresses are taken; the caller
    // keeps the live output and its borrowed metadata readable during this call.
    unsafe {
        let fields = obs_output_protocol_getter_fields(output);
        Some(Protocols {
            flags: *fields.flags,
            protocols: *fields.protocols,
        })
    }
}

fn log(name: &'static CStr) {
    // SAFETY: both the format and getter name are static terminated strings.
    unsafe { blog(400, c"%s: Null 'output' parameter".as_ptr(), name.as_ptr()) }
}

/// # Safety
/// A non-null output must remain live and readable under the original C contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_get_protocols(output: *const c_void) -> *const c_char {
    // SAFETY: the caller supplies a live output or null; fields rejects null first.
    output_protocols::protocols(unsafe { fields(output) }, log)
}
