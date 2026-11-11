//! Read the real private layout only after rejecting a NULL encoder.
use crate::media_getters::{Getter, Media, get};
use core::ffi::{c_char, c_int, c_void};

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Fields {
    pub encoder_type: c_int,
    pub media: *mut c_void,
    pub fps_override: *mut c_void,
    pub name: *const c_char,
}

unsafe extern "C" {
    fn obs_encoder_media_getters_fields(encoder: *const c_void) -> Fields;
    fn blog(level: c_int, format: *const c_char, ...);
}

unsafe fn select(encoder: *const c_void, getter: Getter) -> *mut c_void {
    let fields = if encoder.is_null() {
        None
    } else {
        // SAFETY: the public API requires a live encoder; NULL was rejected above.
        Some(unsafe { obs_encoder_media_getters_fields(encoder) })
    };
    let media = fields.map(|fields| Media {
        encoder_type: fields.encoder_type,
        parent: (!fields.media.is_null()).then_some(fields.media),
        fps_override: (!fields.fps_override.is_null()).then_some(fields.fps_override),
    });
    get(media, getter, |level, format| {
        // SAFETY: static format has at most one %s, matching the borrowed name.
        // NULL diagnostics have no substitutions and do not inspect the name.
        unsafe {
            blog(
                level,
                format.as_ptr(),
                fields.map_or(core::ptr::null(), |f| f.name),
            )
        }
    })
    .unwrap_or(core::ptr::null_mut())
}

/// # Safety
/// A non-NULL encoder must be live and readable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_video(encoder: *const c_void) -> *mut c_void {
    // SAFETY: forwards the public API's live encoder precondition.
    unsafe { select(encoder, Getter::Video) }
}

/// # Safety
/// A non-NULL encoder must be live and readable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_parent_video(encoder: *const c_void) -> *mut c_void {
    // SAFETY: forwards the public API's live encoder precondition.
    unsafe { select(encoder, Getter::ParentVideo) }
}

/// # Safety
/// A non-NULL encoder must be live and readable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_audio(encoder: *const c_void) -> *mut c_void {
    // SAFETY: forwards the public API's live encoder precondition.
    unsafe { select(encoder, Getter::Audio) }
}
