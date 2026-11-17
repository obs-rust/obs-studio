//! Borrowed service handler queries; ownership stays with the service.
use core::ffi::CStr;

#[derive(Clone, Copy)]
pub struct Handlers<T> {
    pub signal: T,
    pub proc: T,
}

pub fn signal<T: Copy + Default>(
    _service: Option<Handlers<T>>,
    _log: impl FnOnce(&'static CStr),
) -> T {
    todo!("#5002 signal handler")
}

pub fn proc<T: Copy + Default>(
    _service: Option<Handlers<T>>,
    _log: impl FnOnce(&'static CStr),
) -> T {
    todo!("#5002 procedure handler")
}
