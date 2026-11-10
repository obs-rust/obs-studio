//! Encoder settings identity and ownership, independent of private C layouts.
use core::ffi::CStr;

pub fn get<T: Copy + Default>(
    encoder: Option<T>,
    addref: impl FnOnce(T),
    log: impl FnOnce(&'static CStr),
) -> T {
    let _ = (encoder, addref, log);
    todo!("port obs_encoder_get_settings")
}
