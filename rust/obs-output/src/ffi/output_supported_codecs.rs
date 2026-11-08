//! Field-address conversion only; the private output layout stays in C.
use crate::output_supported_codecs::{self, Codecs};
use core::ffi::{CStr, c_char, c_int, c_void};

#[repr(C)]
pub struct Fields {
    pub video: *const *const c_char,
    pub audio: *const *const c_char,
}

unsafe extern "C" {
    fn obs_output_supported_codecs_fields(output: *const c_void) -> Fields;
    fn blog(level: c_int, format: *const c_char, ...);
}

unsafe fn fields(output: *const c_void) -> Option<Codecs<*const c_char>> {
    if output.is_null() {
        return None;
    }
    // SAFETY: null is rejected before taking C member addresses; the caller
    // keeps this output and its metadata fields readable throughout the call.
    unsafe {
        let fields = obs_output_supported_codecs_fields(output);
        Some(Codecs {
            video: *fields.video,
            audio: *fields.audio,
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
pub unsafe extern "C" fn obs_output_get_supported_video_codecs(
    output: *const c_void,
) -> *const c_char {
    // SAFETY: the caller supplies a live output or null; fields rejects null first.
    output_supported_codecs::video(unsafe { fields(output) }, log)
}

/// # Safety
/// A non-null output must remain live and readable under the original C contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_get_supported_audio_codecs(
    output: *const c_void,
) -> *const c_char {
    // SAFETY: the caller supplies a live output or null; fields rejects null first.
    output_supported_codecs::audio(unsafe { fields(output) }, log)
}
