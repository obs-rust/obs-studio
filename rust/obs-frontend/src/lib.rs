//! The frontend's one staticlib (frontend/cmake/frontend-rust.cmake): the C
//! ABI of the frontend's Rust crates. One staticlib per binary, so later
//! frontend crates join this one (#2218).
//!
//! A separate crate so the crates stay plain rlibs: an rlib + staticlib crate
//! breaks its own test build (#592).

#![forbid(unsafe_code)]

/// The camera-busy warning (#3724): `frontend/utility/obs-camera-busy.h`.
pub use obs_camera_busy::ffi as camera_busy;
/// The exit watchdog (#4065): `frontend/utility/obs-exit-watchdog.h`.
pub use obs_exit_watchdog::ffi as exit_watchdog;
/// The shared fallback warning (#2218): `frontend/utility/obs-fallback-warning.h`.
pub use obs_fallback_warning::ffi as fallback_warning;
/// GPU acceleration status (#846): `frontend/utility/obs-gpu-status.h`.
pub use obs_gpu_status::ffi as gpu_status;
