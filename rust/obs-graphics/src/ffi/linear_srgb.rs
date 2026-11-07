//! Opaque graphics context boundary; C owns its layout and validation log.

use std::ffi::c_char;

unsafe extern "C" {
    fn obs_graphics_linear_srgb_field(function: *const c_char) -> *const bool;
}

/// Read the current context's flag, or false without a context.
///
/// # Safety
/// The current graphics context, if any, must remain live for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_get_linear_srgb() -> bool {
    // SAFETY: C returns null or the current thread's live context field. The
    // borrow ends after copying the flag; no private layout crosses into Rust.
    let value = unsafe { obs_graphics_linear_srgb_field(c"gs_get_linear_srgb".as_ptr()).as_ref() };
    crate::linear_srgb::linear_srgb(value.copied())
}
