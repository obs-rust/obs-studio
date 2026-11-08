//! Borrowed protocol metadata query, preserving service flag selection.
use core::ffi::CStr;

#[derive(Clone, Copy)]
pub struct Protocols<T> {
    pub flags: u32,
    pub protocols: T,
}

pub fn protocols<T: Copy + Default>(_output: Option<Protocols<T>>, _log: impl FnOnce(&'static CStr)) -> T {
    todo!("port obs_output_get_protocols")
}
