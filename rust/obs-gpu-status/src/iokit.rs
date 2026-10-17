//! macOS GPU utilization from IOKit (obs-rust/obs-studio#1026).
//! Platform-neutral parsing; the reader that copies the dictionary lives in
//! `platform/macos.rs`.
//!
//! Each `IOAccelerator` service (one per GPU) has a `PerformanceStatistics`
//! dictionary. Its `Device Utilization %` is the whole GPU; `Renderer
//! Utilization %` and `Tiler Utilization %` are parts of it. The keys are
//! not documented. They exist on Apple Silicon (M1 Max, macOS 26.7.1,
//! `AGXAcceleratorG13X`).

use crate::Utilization;

/// The `PerformanceStatistics` key for the whole GPU.
pub const DEVICE_UTILIZATION: &str = "Device Utilization %";

/// The whole-GPU utilization in `stats`, the dictionary's number entries as
/// `(key, value)`. `Unavailable` when the key is missing or its value is
/// not 0 to 100.
pub fn device_utilization(stats: &[(String, i64)]) -> Utilization {
    stats
        .iter()
        .find(|(key, _)| key == DEVICE_UTILIZATION)
        .and_then(|&(_, value)| u8::try_from(value).ok())
        .filter(|&p| p <= 100)
        .map_or(Utilization::Unavailable, Utilization::Percent)
}
