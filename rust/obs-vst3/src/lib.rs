#![deny(unsafe_code)]

pub mod bus;
#[allow(unsafe_code)]
mod ffi;
pub mod parameters;
mod platform;
pub mod state;

pub use ffi::{Engine, NativeClass, enumerate};

#[derive(Debug, PartialEq, Eq)]
pub struct Error(pub(crate) &'static str);

pub mod filter;
