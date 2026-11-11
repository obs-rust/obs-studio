//! Loadable OBS frontend control module. It never starts or shuts down libobs.
#![deny(unsafe_code)]
#[allow(unsafe_code, reason = "OBS module C ABI exports")]
mod ffi;
mod platform;
