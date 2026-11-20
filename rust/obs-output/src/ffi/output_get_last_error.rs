//! The public output last-error query over the real private C fields: the
//! existing output error slot, video and audio slot and encoder error
//! adapters. Validation and priority live in the safe core.
use crate::ffi::encoder_last_error::LastErrorFields as EncoderFields;
use crate::output_get_last_error::{LastErrorFields, last_error};
use core::ffi::{c_char, c_int, c_void};
use core::ptr::NonNull;

unsafe extern "C" {
    fn obs_output_last_error_fields(output: *mut c_void) -> *mut *mut c_char;
    fn obs_output_video_encoder_getter_at(output: *const c_void, idx: usize) -> *mut c_void;
    fn obs_output_encoder_getters_audio_at(output: *const c_void, idx: usize) -> *mut c_void;
    fn obs_encoder_last_error_fields(encoder: *mut c_void) -> EncoderFields;
    fn blog(level: c_int, format: *const c_char, ...);
}

/// A non-NULL output the caller keeps live and readable.
struct Output(*mut c_void);
impl LastErrorFields for Output {
    type Encoder = NonNull<c_void>;
    type Error = NonNull<c_char>;
    fn own(&mut self) -> Option<NonNull<c_char>> {
        // SAFETY: the output is non-NULL and live; the adapter returns the
        // address of its error field, which is only read here.
        NonNull::new(unsafe { *obs_output_last_error_fields(self.0) })
    }
    fn video(&mut self, idx: usize) -> Option<NonNull<c_void>> {
        // SAFETY: the core asks only below MAX_OUTPUT_VIDEO_ENCODERS.
        NonNull::new(unsafe { obs_output_video_encoder_getter_at(self.0, idx) })
    }
    fn audio(&mut self, idx: usize) -> Option<NonNull<c_void>> {
        // SAFETY: the core asks only below MAX_OUTPUT_AUDIO_ENCODERS.
        NonNull::new(unsafe { obs_output_encoder_getters_audio_at(self.0, idx) })
    }
    fn encoder_error(&mut self, encoder: NonNull<c_void>) -> Option<NonNull<c_char>> {
        // SAFETY: a slot of the live output holds this encoder; for a
        // non-NULL encoder the adapter returns its error field's address.
        let fields = unsafe { obs_encoder_last_error_fields(encoder.as_ptr()) };
        // SAFETY: as above; the field is read without its mutex, as in C.
        NonNull::new(unsafe { *fields.message }.cast_mut())
    }
}

/// Borrow the output's error, else its first encoder error, else NULL.
///
/// # Safety
/// A non-NULL output and the encoders in its slots must be live and readable
/// during the call. The result borrows C-owned storage, as in C.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_get_last_error(output: *mut c_void) -> *const c_char {
    let mut fields = (!output.is_null()).then_some(Output(output));
    let error = last_error(fields.as_mut(), || {
        // SAFETY: obs_output_valid()'s static format and arguments.
        unsafe {
            blog(
                400,
                c"%s: Null '%s' parameter".as_ptr(),
                c"obs_output_get_last_error".as_ptr(),
                c"output".as_ptr(),
            )
        }
    });
    error.map_or(core::ptr::null(), |error| error.as_ptr().cast_const())
}
