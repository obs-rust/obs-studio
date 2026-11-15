//! Thin ABI shim for the legacy preferred-size wrapper. Private fields come
//! through the output size getters' opaque adapters; the indexed setter stays
//! the C `obs_output_set_preferred_size2`.
use crate::output_preferred_size::{DEFAULT_SLOT, Decision, decide};
use core::ffi::{c_char, c_int, c_void};

unsafe extern "C" {
    fn obs_output_size_getters_flags(output: *const c_void) -> u32;
    fn obs_output_size_getters_name(output: *const c_void) -> *const c_char;
    fn obs_output_set_preferred_size2(output: *mut c_void, width: u32, height: u32, idx: usize);
    fn blog(level: c_int, format: *const c_char, ...);
}

/// The indexed setter the wrapper delegates to.
pub trait Calls {
    /// # Safety
    /// `output` is the live, non-NULL output the wrapper validated.
    unsafe fn set_preferred_size2(&self, output: *mut c_void, width: u32, height: u32, idx: usize);
}

/// The production delegate: libobs's own `obs_output_set_preferred_size2`.
pub struct Libobs;
impl Calls for Libobs {
    unsafe fn set_preferred_size2(&self, output: *mut c_void, width: u32, height: u32, idx: usize) {
        // SAFETY: the caller passes the validated live output.
        unsafe { obs_output_set_preferred_size2(output, width, height, idx) }
    }
}

/// # Safety
/// A non-NULL `output` must be a live output for the whole call.
pub unsafe fn set_preferred_size(calls: &impl Calls, output: *mut c_void, width: u32, height: u32) {
    let decision = decide(!output.is_null(), || {
        // SAFETY: only requested for a non-NULL live output.
        unsafe { obs_output_size_getters_flags(output) }
    });
    // SAFETY: static formats with matching arguments; a non-NULL output is
    // live, so its name stays readable for the diagnostic.
    unsafe {
        match decision {
            Decision::Null => blog(
                400,
                c"%s: Null 'output' parameter".as_ptr(),
                c"obs_output_set_preferred_size".as_ptr(),
            ),
            Decision::NotVideo => blog(
                200,
                c"Output '%s': Tried to use %s on a non-video output".as_ptr(),
                obs_output_size_getters_name(output),
                c"obs_output_set_preferred_size".as_ptr(),
            ),
            Decision::Delegate => calls.set_preferred_size2(output, width, height, DEFAULT_SLOT),
        }
    }
}

/// # Safety
/// A non-NULL `output` must be a live output for the whole call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_set_preferred_size(
    output: *mut c_void,
    width: u32,
    height: u32,
) {
    // SAFETY: the caller supplies the original setter contract.
    unsafe { set_preferred_size(&Libobs, output, width, height) }
}
