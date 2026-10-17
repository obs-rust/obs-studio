//! Low-level bindings to the original libobs C ABI.
//!
//! `bundled` builds the native library through obs-build and cc::Build. Without
//! it the declarations can be used by a parent supplying its own libobs linkage.
#![deny(unsafe_code)]

#[allow(unsafe_code)]
pub mod ffi;
