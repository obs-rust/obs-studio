//! Only the C adapter knows the private output layout.
use crate::output_mixer_getters;
use core::ffi::{CStr, c_char, c_int, c_void};

#[repr(C)]
pub struct Fields {
    pub mask: *const usize,
}

unsafe extern "C" {
    fn obs_output_mixer_getters_fields(output: *const c_void) -> Fields;
    fn blog(level: c_int, format: *const c_char, ...);
}

unsafe fn mask(output: *const c_void) -> Option<usize> {
    if output.is_null() {
        return None;
    }
    // SAFETY: null is rejected before taking a member address. The caller
    // keeps the output live and its mask readable for this call.
    unsafe { Some(*obs_output_mixer_getters_fields(output).mask) }
}

fn log(name: &'static CStr) {
    // SAFETY: literal format and static terminated function name.
    unsafe { blog(400, c"%s: Null 'output' parameter".as_ptr(), name.as_ptr()) }
}

/// # Safety
/// A non-null output must be live and readable under the original C contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_get_mixer(output: *const c_void) -> usize {
    // SAFETY: the caller supplies a live output or null, checked before access.
    output_mixer_getters::mixer(unsafe { mask(output) }, log)
}

/// # Safety
/// A non-null output must be live and readable under the original C contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_get_mixers(output: *const c_void) -> usize {
    // SAFETY: the caller supplies a live output or null, checked before access.
    output_mixer_getters::mixers(unsafe { mask(output) }, log)
}
