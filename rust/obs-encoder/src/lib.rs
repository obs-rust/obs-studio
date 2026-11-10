//! Encoder state ownership with a thin libobs ABI boundary.
#![deny(unsafe_code)]
#[allow(unsafe_code)]
pub mod ffi;
pub mod last_error;
