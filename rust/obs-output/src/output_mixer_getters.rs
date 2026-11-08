//! Public output mixer queries over a borrowed mask snapshot.
use core::ffi::CStr;

pub fn mixer(mask: Option<usize>, log: impl FnOnce(&'static CStr)) -> usize {
    match mask {
        Some(mask) => crate::video_checks::first_mixer(mask),
        None => {
            log(c"obs_output_get_mixer");
            0
        }
    }
}

pub fn mixers(mask: Option<usize>, log: impl FnOnce(&'static CStr)) -> usize {
    match mask {
        Some(mask) => mask,
        None => {
            log(c"obs_output_get_mixers");
            0
        }
    }
}
