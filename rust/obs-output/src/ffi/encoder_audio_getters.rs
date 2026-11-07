//! Borrowed audio field addresses; the encoder layout remains owned by C.
use crate::encoder_audio_getters;
use core::ffi::{CStr, c_char, c_int, c_void};

#[repr(C)]
pub struct AudioFields {
    pub kind: *const c_int,
    pub media: *const *mut c_void,
    pub rate: *const u32,
    pub frame_size: *const usize,
    pub mixer_index: *const usize,
    pub name: *const *mut c_char,
}
unsafe extern "C" {
    fn obs_encoder_audio_getters_fields(encoder: *const c_void) -> AudioFields;
    fn audio_output_get_sample_rate(audio: *const c_void) -> u32;
    fn blog(level: c_int, format: *const c_char, ...);
}
impl AudioFields {
    unsafe fn rate_values(&self) -> Option<(i32, bool, u32)> {
        // SAFETY: all pointers borrow readable fields of the live encoder, or are null.
        unsafe {
            self.kind
                .as_ref()
                .copied()
                .zip(self.media.as_ref())
                .zip(self.rate.as_ref().copied())
                .map(|((kind, media), rate)| (kind, !media.is_null(), rate))
        }
    }
    unsafe fn size_values(&self, field: *const usize) -> Option<(i32, usize)> {
        // SAFETY: both pointers borrow readable fields of the live encoder, or are null.
        unsafe { self.kind.as_ref().copied().zip(field.as_ref().copied()) }
    }
    fn log(&self, level: c_int, format: &'static CStr) {
        // SAFETY: the borrowed name lives through this synchronous call; null diagnostics ignore it.
        unsafe {
            blog(
                level,
                format.as_ptr(),
                self.name.as_ref().copied().unwrap_or(core::ptr::null_mut()),
            )
        };
    }
}
/// # Safety
/// A non-null encoder and its media must stay live and readable under the original getter's synchronization contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_get_sample_rate(encoder: *const c_void) -> u32 {
    // SAFETY: the adapter borrows fields from the caller's live encoder, or returns nulls.
    let fields = unsafe { obs_encoder_audio_getters_fields(encoder) };
    // SAFETY: fields stay readable; media is accessed only on the audio media route.
    unsafe {
        encoder_audio_getters::sample_rate(
            fields.rate_values(),
            || audio_output_get_sample_rate(*fields.media),
            |l, f| fields.log(l, f),
        )
    }
}
/// # Safety
/// A non-null encoder must stay live with readable fields during this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_get_frame_size(encoder: *const c_void) -> usize {
    // SAFETY: the adapter borrows fields from the caller's live encoder, or returns nulls.
    let fields = unsafe { obs_encoder_audio_getters_fields(encoder) };
    // SAFETY: the frame-size and type fields stay readable for this call.
    encoder_audio_getters::frame_size(unsafe { fields.size_values(fields.frame_size) }, |l, f| {
        fields.log(l, f)
    })
}
/// # Safety
/// A non-null encoder must stay live with readable fields during this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_get_mixer_index(encoder: *const c_void) -> usize {
    // SAFETY: the adapter borrows fields from the caller's live encoder, or returns nulls.
    let fields = unsafe { obs_encoder_audio_getters_fields(encoder) };
    // SAFETY: the mixer-index and type fields stay readable for this call.
    encoder_audio_getters::mixer_index(unsafe { fields.size_values(fields.mixer_index) }, |l, f| {
        fields.log(l, f)
    })
}
