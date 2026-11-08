//! Public encoder GPU scaling getters with the original diagnostics.
use core::ffi::CStr;

fn get(
    fields: Option<(i32, i32)>,
    null_format: &'static CStr,
    wrong_type_format: &'static CStr,
    log: impl FnOnce(i32, &'static CStr),
) -> i32 {
    match fields {
        Some((1, scale)) => scale,
        Some(_) => {
            log(200, wrong_type_format);
            0
        }
        None => {
            log(400, null_format);
            0
        }
    }
}

pub fn scale_type(fields: Option<(i32, i32)>, log: impl FnOnce(i32, &'static CStr)) -> i32 {
    get(
        fields,
        c"obs_encoder_get_scale_type: Null 'encoder' parameter",
        c"obs_encoder_get_scale_type: encoder '%s' is not a video encoder",
        log,
    )
}

pub fn enabled(fields: Option<(i32, i32)>, log: impl FnOnce(i32, &'static CStr)) -> bool {
    get(
        fields,
        c"obs_encoder_gpu_scaling_enabled: Null 'encoder' parameter",
        c"obs_encoder_gpu_scaling_enabled: encoder '%s' is not a video encoder",
        log,
    ) != 0
}
