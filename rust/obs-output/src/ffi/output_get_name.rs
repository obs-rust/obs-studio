//! The public output name query over the real private context-name field.
//! The NULL guard lives in the safe core; the field read sits behind a
//! private C adapter, so no output layout crosses the ABI.
use crate::output_get_name::get_name;
use core::ffi::{c_char, c_int, c_void};

unsafe extern "C" {
    fn obs_output_get_name_context_name(output: *const c_void) -> *const c_char;
    fn blog(level: c_int, format: *const c_char, ...);
}

/// Borrow the output's current context name, or NULL for a NULL output.
///
/// # Safety
/// A non-NULL output must be live and readable during the call. The result
/// borrows its C-owned name, as in C.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_get_name(output: *const c_void) -> *const c_char {
    get_name(
        (!output.is_null()).then_some(|| {
            // SAFETY: get_name projects only a non-NULL, live output.
            unsafe { obs_output_get_name_context_name(output) }
        }),
        || {
            // SAFETY: obs_output_valid()'s static format and arguments.
            unsafe {
                blog(
                    400,
                    c"%s: Null '%s' parameter".as_ptr(),
                    c"obs_output_get_name".as_ptr(),
                    c"output".as_ptr(),
                )
            }
        },
    )
    .unwrap_or(core::ptr::null())
}
