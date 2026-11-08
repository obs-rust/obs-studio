//! Public output mixer queries over a borrowed mask snapshot.
use core::ffi::CStr;

pub fn mixer(_mask: Option<usize>, _log: impl FnOnce(&'static CStr)) -> usize {
    todo!("port obs_output_get_mixer")
}

pub fn mixers(_mask: Option<usize>, _log: impl FnOnce(&'static CStr)) -> usize {
    todo!("port obs_output_get_mixers")
}
