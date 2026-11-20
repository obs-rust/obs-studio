//! Borrow the live C encoder and preserve its raw-video callback identity.
use core::ffi::c_void;

type Callback = unsafe extern "C" fn(*mut c_void, *mut c_void);

unsafe extern "C" {
    fn obs_encoder_align_media(encoder: *const c_void) -> *mut c_void;
    fn obs_encoder_align_divisor(encoder: *const c_void) -> u32;
    fn obs_encoder_align_receive_video() -> Option<Callback>;
    fn obs_encoder_align_forward(
        media: *mut c_void,
        timestamp: u64,
        callback: Option<Callback>,
        encoder: *mut c_void,
    );
}

/// # Safety
/// A non-null encoder and its media must remain live under the same locking
/// and lifetime rules as the original C alignment operation.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_align_frame_rate_divisor(
    encoder: *mut c_void,
    start_timestamp: u64,
) {
    if encoder.is_null() {
        return;
    }
    // SAFETY: NULL was rejected before either real-layout field accessor.
    let (media, divisor) = unsafe {
        (
            core::ptr::NonNull::new(obs_encoder_align_media(encoder)),
            obs_encoder_align_divisor(encoder),
        )
    };
    crate::encoder_align_frame_rate_divisor::align(
        Some((media, divisor)),
        start_timestamp,
        |media, timestamp| {
            // SAFETY: the C seam returns the actual static receive_video used
            // by start_raw_video. It is passed through, never invoked here.
            unsafe {
                obs_encoder_align_forward(
                    media.as_ptr(),
                    timestamp,
                    obs_encoder_align_receive_video(),
                    encoder,
                );
            }
        },
    );
}
