//! Synchronous media queries with lazy private-field access.
use core::ffi::CStr;

/// Reads occur only after each preceding admission step succeeds.
pub trait Source {
    fn has_data(&self) -> bool;
    fn output_flags(&self) -> u32;
    fn duration(&self) -> Option<i64>;
    fn time(&self) -> Option<i64>;
    fn state(&self) -> Option<u32>;
}

pub fn duration(_source: Option<&impl Source>, _log: impl FnOnce(&'static CStr)) -> i64 {
    todo!()
}
pub fn time(_source: Option<&impl Source>, _log: impl FnOnce(&'static CStr)) -> i64 {
    todo!()
}
pub fn state(_source: Option<&impl Source>, _log: impl FnOnce(&'static CStr)) -> u32 {
    todo!()
}
