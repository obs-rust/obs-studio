//! Read-only instance encoder metadata, with the original NULL diagnostics.
use core::ffi::CStr;

fn read<T: Default>(
    value: Option<T>,
    function: &'static CStr,
    log: impl FnOnce(&'static CStr, &'static CStr),
) -> T {
    let _ = (value, function, log);
    todo!("#3878")
}

pub fn codec<T: Default>(value: Option<T>, log: impl FnOnce(&'static CStr, &'static CStr)) -> T {
    read(value, c"obs_encoder_get_codec", log)
}

pub fn kind(value: Option<i32>, log: impl FnOnce(&'static CStr, &'static CStr)) -> i32 {
    read(value, c"obs_encoder_get_type", log)
}

pub fn id<T: Default>(value: Option<T>, log: impl FnOnce(&'static CStr, &'static CStr)) -> T {
    read(value, c"obs_encoder_get_id", log)
}

pub fn type_data<T: Default>(
    value: Option<T>,
    log: impl FnOnce(&'static CStr, &'static CStr),
) -> T {
    read(value, c"obs_encoder_get_type_data", log)
}

pub fn caps(value: Option<u32>, log: impl FnOnce(&'static CStr, &'static CStr)) -> u32 {
    read(value, c"obs_encoder_get_caps", log)
}
