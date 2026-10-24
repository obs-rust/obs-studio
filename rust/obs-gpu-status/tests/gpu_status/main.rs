//! obs-gpu-status integration tests: gpu_status (one module per former tests/*.rs file, #968).

mod backoff;
mod busy_percent;
mod classify;
mod device_cache;
mod dxgi;
mod fdinfo;
mod ffi;
// obs-fallback-warning (#2218): the frontend's other Rust, same category.
#[path = "../../../obs-fallback-warning/tests/fallback_warning/main.rs"]
mod fallback_warning;
mod frame_window;
mod frames;
mod iokit;
mod locale;
mod log_lines;
mod nvml;
mod pdh;
mod render_adapter;
mod report;
mod usage;
