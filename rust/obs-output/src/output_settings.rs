//! Settings lookup retains one reference and preserves the stored handle.
use core::ffi::CStr;

pub fn settings<T: Copy + Default>(
    output: Option<T>,
    addref: impl FnOnce(T),
    log: impl FnOnce(&'static CStr),
) -> T {
    let _ = (output, addref, log);
    todo!()
}
