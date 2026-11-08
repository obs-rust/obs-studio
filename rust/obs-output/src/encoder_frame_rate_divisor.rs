//! Public frame-rate-divisor accessor, preserving historical diagnostics.
use core::ffi::CStr;

pub fn get(fields: Option<(i32, u32)>, log: impl FnOnce(i32, &'static CStr)) -> u32 {
    match fields {
        None => {
            log(
                400,
                c"obs_encoder_set_frame_rate_divisor: Null 'encoder' parameter",
            );
            0
        }
        Some((1, divisor)) => divisor,
        Some(_) => {
            log(
                200,
                c"obs_encoder_set_frame_rate_divisor: encoder '%s' is not a video encoder",
            );
            0
        }
    }
}
