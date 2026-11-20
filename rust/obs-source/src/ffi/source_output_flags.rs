//! The source layout stays behind the existing output-flags field adapter
//! (`obs-source-audio-mixers-fields.c.h`), which every Rust-static consumer
//! already links.
use crate::output_flags::output_flags;
use core::ffi::{CStr, c_char, c_int, c_void};

unsafe extern "C" {
    fn obs_source_audio_mixers_flags_field(source: *const c_void) -> u32;
    fn blog(level: c_int, format: *const c_char, ...);
}

/// # Safety
/// A non-null source must be live under the original libobs ownership contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_get_output_flags(source: *const c_void) -> u32 {
    let read = (!source.is_null()).then_some(|| {
        // SAFETY: NULL was rejected before the private C layout accessor.
        unsafe { obs_source_audio_mixers_flags_field(source) }
    });
    output_flags(read, |name: &'static CStr| {
        // SAFETY: static strings match the two %s arguments of the format.
        unsafe {
            blog(
                400,
                c"%s: Null '%s' parameter".as_ptr(),
                name.as_ptr(),
                c"source".as_ptr(),
            );
        }
    })
}
