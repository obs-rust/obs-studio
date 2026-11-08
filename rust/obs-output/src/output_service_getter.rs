//! Borrowed service lookup; ownership stays with the output.
use core::ffi::CStr;

pub fn service<T: Copy + Default>(output: Option<T>, log: impl FnOnce(&'static CStr)) -> T {
    match output {
        Some(service) => service,
        None => {
            log(c"obs_output_get_service");
            T::default()
        }
    }
}
