//! Public width and height getters, preserving diagnostics and lazy media reads.
use crate::encoder_size::{SizeRoute, video_size_route};
use core::ffi::CStr;

fn get(
    fields: Option<(i32, bool, u32)>,
    media: impl FnOnce() -> u32,
    log: impl FnOnce(i32, &'static CStr),
    null_format: &'static CStr,
    wrong_type_format: &'static CStr,
) -> u32 {
    let Some((kind, has_media, scaled)) = fields else {
        log(400, null_format);
        return 0;
    };
    match video_size_route(kind, has_media, scaled) {
        SizeRoute::WrongType => {
            log(200, wrong_type_format);
            0
        }
        SizeRoute::NoMedia => 0,
        SizeRoute::Stored => scaled,
        SizeRoute::Media => media(),
    }
}
pub fn width(
    fields: Option<(i32, bool, u32)>,
    media: impl FnOnce() -> u32,
    log: impl FnOnce(i32, &'static CStr),
) -> u32 {
    get(
        fields,
        media,
        log,
        c"obs_encoder_get_width: Null 'encoder' parameter",
        c"obs_encoder_get_width: encoder '%s' is not a video encoder",
    )
}
pub fn height(
    fields: Option<(i32, bool, u32)>,
    media: impl FnOnce() -> u32,
    log: impl FnOnce(i32, &'static CStr),
) -> u32 {
    get(
        fields,
        media,
        log,
        c"obs_encoder_get_height: Null 'encoder' parameter",
        c"obs_encoder_get_height: encoder '%s' is not a video encoder",
    )
}
