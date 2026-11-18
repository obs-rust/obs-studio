//! Borrowed service handler queries; ownership stays with the service.
use core::ffi::CStr;

#[derive(Clone, Copy)]
pub struct Handlers<T> {
    pub signal: T,
    pub proc: T,
}

pub fn signal<T: Copy + Default>(
    service: Option<Handlers<T>>,
    log: impl FnOnce(&'static CStr),
) -> T {
    match service {
        Some(fields) => fields.signal,
        None => {
            log(c"obs_service_get_signal_handler");
            T::default()
        }
    }
}

pub fn proc<T: Copy + Default>(service: Option<Handlers<T>>, log: impl FnOnce(&'static CStr)) -> T {
    match service {
        Some(fields) => fields.proc,
        None => {
            log(c"obs_service_get_proc_handler");
            T::default()
        }
    }
}
