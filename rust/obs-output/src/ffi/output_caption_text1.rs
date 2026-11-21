//! The public legacy caption text call. The NULL guard and the duration
//! live in the safe core; the queueing stays in the C
//! `obs_output_output_caption_text2()`, called with the caller's pointers.
use crate::output_caption_text1::caption_text1;
use core::ffi::{c_char, c_int, c_void};
use core::ptr::NonNull;

unsafe extern "C" {
    // EXPORT, in obs-output.c.h.
    fn obs_output_output_caption_text2(
        output: *mut c_void,
        text: *const c_char,
        display_duration: f64,
    );
    fn blog(level: c_int, format: *const c_char, ...);
}

/// Queue `text` on every caption track of `output`, two seconds a page.
///
/// # Safety
/// As `obs_output_output_caption_text2()`: a non-NULL output must be live,
/// and `text` must be NULL or a NUL-terminated string whenever that function
/// reads it (an active output).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_output_caption_text1(output: *mut c_void, text: *const c_char) {
    caption_text1(
        NonNull::new(output),
        || {
            // SAFETY: obs_output_valid()'s static format and arguments.
            unsafe {
                blog(
                    400,
                    c"%s: Null '%s' parameter".as_ptr(),
                    c"obs_output_output_caption_text1".as_ptr(),
                    c"output".as_ptr(),
                )
            }
        },
        |output, duration| {
            // SAFETY: the caller's live output and text, passed through
            // unchanged under the caller's contract.
            unsafe { obs_output_output_caption_text2(output.as_ptr(), text, duration) }
        },
    );
}
