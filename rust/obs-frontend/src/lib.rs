//! The staticlib the frontend links (frontend/cmake/feature-gpu-status.cmake):
//! obs-gpu-status's C ABI shim.
//!
//! A separate crate so obs-gpu-status stays a plain rlib: an rlib + staticlib
//! crate breaks its own test build (#592).

#![forbid(unsafe_code)]

pub use obs_gpu_status::ffi;
