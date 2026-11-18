//! `obs_output_video()`: a raw output borrows its own video, an encoded
//! output the video of its first occupied video encoder. No private layout
//! here; the shim reads the output through C adapters.
use crate::video_checks::OBS_OUTPUT_ENCODED;
use core::ffi::CStr;

/// `valid` is `obs_output_valid()`'s NULL check. Only a valid output's
/// `flags` are read; then exactly one of `raw` (no `OBS_OUTPUT_ENCODED`) or
/// `encoded` runs, and its result is returned unchanged. A NULL output logs
/// `obs_output_video` and reads nothing.
pub fn video<T: Default>(
    valid: bool,
    flags: impl FnOnce() -> u32,
    raw: impl FnOnce() -> T,
    encoded: impl FnOnce() -> T,
    log: impl FnOnce(&'static CStr),
) -> T {
    if !valid {
        log(c"obs_output_video");
        return T::default();
    }
    if flags() & OBS_OUTPUT_ENCODED == 0 {
        raw()
    } else {
        encoded()
    }
}
