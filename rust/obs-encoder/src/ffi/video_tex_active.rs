//! Borrow opaque C owners; never reproduce the encoder or mix layout in Rust.
use core::ffi::{c_int, c_void};
use core::ptr::NonNull;

unsafe extern "C" {
    fn obs_encoder_video_tex_active_mix(encoder: *const c_void) -> *const c_void;
    fn obs_encoder_video_tex_active_nv12(mix: *const c_void) -> bool;
    fn obs_encoder_video_tex_active_p010(mix: *const c_void) -> bool;
}

/// # Safety
/// As in C, the encoder and OBS core must be live and synchronized, and the
/// selected mix must remain readable through the query. NULL encoder is outside
/// the C API's contract; NULL media and an absent mix are valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_video_tex_active(
    encoder: *const c_void,
    format: c_int,
) -> bool {
    // Reject an invalid owner before the C adapter can project encoder->media.
    let encoder = NonNull::new(encoder.cast_mut()).expect("the C API requires a live encoder");
    crate::video_tex_active::video_tex_active(
        format,
        || {
            // SAFETY: the owner was checked above; C retains the synchronized
            // get_mix_for_video walk and reads the real encoder media field.
            NonNull::new(unsafe { obs_encoder_video_tex_active_mix(encoder.as_ptr()) }.cast_mut())
        },
        |mix| {
            // SAFETY: the safe core selects only a present mix and the NV12 flag.
            unsafe { obs_encoder_video_tex_active_nv12(mix.as_ptr()) }
        },
        |mix| {
            // SAFETY: the safe core selects only a present mix and the P010 flag.
            unsafe { obs_encoder_video_tex_active_p010(mix.as_ptr()) }
        },
    )
}
