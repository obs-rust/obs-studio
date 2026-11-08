//! Borrowed output handler queries; ownership stays with the output.
use core::ffi::CStr;

#[derive(Clone, Copy)]
pub struct Handlers<T> {
    pub signal: T,
    pub proc: T,
}

pub fn signal<T: Copy + Default>(_output: Option<Handlers<T>>, _log: impl FnOnce(&'static CStr)) -> T {
    todo!("output handler getter")
}

pub fn proc<T: Copy + Default>(_output: Option<Handlers<T>>, _log: impl FnOnce(&'static CStr)) -> T {
    todo!("output handler getter")
}
