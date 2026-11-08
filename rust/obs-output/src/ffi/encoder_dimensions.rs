//! Borrowed field addresses from the real private encoder, never a mirrored layout.
use crate::encoder_dimensions;
use core::ffi::{CStr, c_char, c_int, c_void};

#[repr(C)]
pub struct DimensionFields {
    pub kind: *const c_int,
    pub media: *const *mut c_void,
    pub width: *const u32,
    pub height: *const u32,
    pub name: *const *mut c_char,
}
unsafe extern "C" {
    fn obs_encoder_dimensions_fields(encoder: *const c_void) -> DimensionFields;
    fn blog(level: c_int, format: *const c_char, ...);
    fn video_output_get_width(video: *const c_void) -> u32;
    fn video_output_get_height(video: *const c_void) -> u32;
}
impl DimensionFields {
    unsafe fn values(&self, axis: *const u32) -> Option<(i32, bool, u32)> {
        // SAFETY: adapter fields are null for NULL, otherwise readable in the live encoder.
        unsafe {
            self.kind
                .as_ref()
                .copied()
                .zip(self.media.as_ref())
                .zip(axis.as_ref().copied())
                .map(|((kind, media), scaled)| (kind, !media.is_null(), scaled))
        }
    }
    fn log(&self, level: c_int, format: &'static CStr) {
        // SAFETY: the borrowed name stays live for this synchronous log; NULL diagnostics ignore the extra argument.
        unsafe {
            blog(
                level,
                format.as_ptr(),
                self.name.as_ref().copied().unwrap_or(core::ptr::null_mut()),
            );
        }
    }
}
/// # Safety
/// Non-null encoders and their media must remain live with readable fields under the original getter's synchronization contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_get_width(encoder: *const c_void) -> u32 {
    // SAFETY: the adapter borrows addresses from the caller's live encoder, or returns nulls.
    let fields = unsafe { obs_encoder_dimensions_fields(encoder) };
    // SAFETY: fields remain readable; the media closure is called only for a present video on the media route.
    unsafe {
        encoder_dimensions::width(
            fields.values(fields.width),
            || video_output_get_width(*fields.media),
            |level, format| fields.log(level, format),
        )
    }
}
/// # Safety
/// Non-null encoders and their media must remain live with readable fields under the original getter's synchronization contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_get_height(encoder: *const c_void) -> u32 {
    // SAFETY: the adapter borrows addresses from the caller's live encoder, or returns nulls.
    let fields = unsafe { obs_encoder_dimensions_fields(encoder) };
    // SAFETY: fields remain readable; the media closure is called only for a present video on the media route.
    unsafe {
        encoder_dimensions::height(
            fields.values(fields.height),
            || video_output_get_height(*fields.media),
            |level, format| fields.log(level, format),
        )
    }
}
