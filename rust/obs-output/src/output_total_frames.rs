//! Output frame count conversion, independent of the private output layout.
use core::ffi::CStr;

/// Preserve the C long-to-int conversion and NULL diagnostic.
pub fn get(value: Option<i64>, log: impl FnOnce(&'static CStr)) -> i32 {
    let _ = (value, log);
    todo!("port obs_output_get_total_frames")
}
