//! Borrowed protocol metadata query, preserving service flag selection.
use core::ffi::CStr;

#[derive(Clone, Copy)]
pub struct Protocols<T> {
    pub flags: u32,
    pub protocols: T,
}

pub fn protocols<T: Copy + Default>(
    output: Option<Protocols<T>>,
    log: impl FnOnce(&'static CStr),
) -> T {
    match output {
        Some(fields) if fields.flags & crate::video_checks::OBS_OUTPUT_SERVICE != 0 => {
            fields.protocols
        }
        Some(_) => T::default(),
        None => {
            log(c"obs_output_get_protocols");
            T::default()
        }
    }
}
