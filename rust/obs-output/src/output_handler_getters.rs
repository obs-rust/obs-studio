//! Borrowed output handler queries; ownership stays with the output.
use core::ffi::CStr;

#[derive(Clone, Copy)]
pub struct Handlers<T> {
    pub signal: T,
    pub proc: T,
}

pub fn signal<T: Copy + Default>(
    output: Option<Handlers<T>>,
    log: impl FnOnce(&'static CStr),
) -> T {
    match output {
        Some(fields) => fields.signal,
        None => {
            log(c"obs_output_get_signal_handler");
            T::default()
        }
    }
}

pub fn proc<T: Copy + Default>(output: Option<Handlers<T>>, log: impl FnOnce(&'static CStr)) -> T {
    match output {
        Some(fields) => fields.proc,
        None => {
            log(c"obs_output_get_proc_handler");
            T::default()
        }
    }
}
