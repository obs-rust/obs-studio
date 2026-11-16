//! Borrow private C fields after NULL validation; delegate assignment to the core.
use crate::output_set_media::{Diagnostic, Media, set_media};
use core::ffi::{c_char, c_int, c_void};
use obs_util::base::{LOG_DEBUG, LOG_WARNING};

#[repr(C)]
pub struct Fields {
    pub flags: *const u32,
    pub video: *mut *mut c_void,
    pub audio: *mut *mut c_void,
    pub name: *const c_char,
}
unsafe extern "C" {
    fn obs_output_set_media_fields(output: *mut c_void) -> Fields;
    fn blog(level: c_int, format: *const c_char, ...);
}

/// # Safety
/// Non-NULL output is live and its media fields are exclusively accessible.
/// Media pointers obey the original C API lifetime contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_set_media(
    output: *mut c_void,
    video: *mut c_void,
    audio: *mut c_void,
) {
    let fields = core::ptr::NonNull::new(output).map(|output| {
        // SAFETY: checked non-NULL; caller supplies a live real-layout object.
        unsafe { obs_output_set_media_fields(output.as_ptr()) }
    });
    let media = fields.as_ref().map(|fields| {
        // SAFETY: distinct video/audio fields of the live output, exclusively
        // borrowed for this call. The flags field is readable and disjoint.
        unsafe {
            Media {
                flags: *fields.flags,
                video: &mut *fields.video,
                audio: &mut *fields.audio,
            }
        }
    });
    set_media(media, video, audio, |diagnostic| {
        // SAFETY: static printf formats; the encoded diagnostic is only emitted
        // with a live output, whose context name follows the C string contract.
        unsafe {
            match diagnostic {
                Diagnostic::NullOutput => blog(
                    LOG_DEBUG,
                    c"obs_output_set_media: Null 'output' parameter".as_ptr(),
                ),
                Diagnostic::EncodedOutput => blog(
                    LOG_WARNING,
                    c"Output '%s': Tried to use %s on an encoded output".as_ptr(),
                    fields.as_ref().map_or(core::ptr::null(), |f| f.name),
                    c"obs_output_set_media".as_ptr(),
                ),
            }
        }
    });
}
