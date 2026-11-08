//! Public audio getters, preserving exact diagnostics and lazy media access.
use crate::encoder_size::{SizeRoute, sample_rate_route};
use core::ffi::CStr;

pub fn sample_rate(
    fields: Option<(i32, bool, u32)>,
    media: impl FnOnce() -> u32,
    log: impl FnOnce(i32, &'static CStr),
) -> u32 {
    let Some((kind, has_media, rate)) = fields else {
        log(
            400,
            c"obs_encoder_get_sample_rate: Null 'encoder' parameter",
        );
        return 0;
    };
    match sample_rate_route(kind, has_media, rate) {
        SizeRoute::WrongType => {
            log(
                200,
                c"obs_encoder_get_sample_rate: encoder '%s' is not an audio encoder",
            );
            0
        }
        SizeRoute::NoMedia => 0,
        SizeRoute::Stored => rate,
        SizeRoute::Media => media(),
    }
}
fn size(
    fields: Option<(i32, usize)>,
    log: impl FnOnce(i32, &'static CStr),
    null: &'static CStr,
    wrong: &'static CStr,
) -> usize {
    match fields {
        None => {
            log(400, null);
            0
        }
        Some((0, value)) => value,
        Some(_) => {
            log(200, wrong);
            0
        }
    }
}
pub fn frame_size(fields: Option<(i32, usize)>, log: impl FnOnce(i32, &'static CStr)) -> usize {
    size(
        fields,
        log,
        c"obs_encoder_get_frame_size: Null 'encoder' parameter",
        c"obs_encoder_get_frame_size: encoder '%s' is not an audio encoder",
    )
}
pub fn mixer_index(fields: Option<(i32, usize)>, log: impl FnOnce(i32, &'static CStr)) -> usize {
    size(
        fields,
        log,
        c"obs_encoder_get_mixer_index: Null 'encoder' parameter",
        c"obs_encoder_get_mixer_index: encoder '%s' is not an audio encoder",
    )
}
