//! Source context initialization order; failed registration deliberately has no rollback.
use core::ffi::CStr;

pub fn initialize<T>(inputs: T, init: impl FnOnce(T) -> bool, register: impl FnOnce(&[&CStr]) -> bool) -> bool {
    let _ = (inputs, init, register);
    todo!("source context initialization")
}
