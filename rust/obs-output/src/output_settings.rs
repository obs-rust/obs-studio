//! Settings lookup retains one reference and preserves the stored handle.
use core::ffi::CStr;

pub fn settings<T: Copy + Default>(
    output: Option<T>,
    addref: impl FnOnce(T),
    log: impl FnOnce(&'static CStr),
) -> T {
    match output {
        Some(data) => {
            addref(data);
            data
        }
        None => {
            log(c"obs_output_get_settings");
            T::default()
        }
    }
}
