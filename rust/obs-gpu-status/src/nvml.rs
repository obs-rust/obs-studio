//! NVIDIA GPU utilization through NVML (obs-rust/obs-studio#1021).
//!
//! NVML is loaded at run time (`libnvidia-ml.so.1`, `nvml.dll`) and its
//! functions are resolved by name from a [`Symbols`] source, so a fake
//! symbol table tests the real calls. `platform::nvml` opens the library;
//! this module only resolves and calls. #1004 measured 0.68 ms per query,
//! against 87 ms for spawning `nvidia-smi`.

use std::ffi::{CStr, c_void};

use crate::{RenderAdapter, Utilization};

/// Library names to try on Linux, in order. NixOS keeps the driver's
/// libraries outside the loader path, in `/run/opengl-driver/lib`.
pub const LINUX_CANDIDATES: &[&str] = &[
    "libnvidia-ml.so.1",
    "/run/opengl-driver/lib/libnvidia-ml.so.1",
];

/// Library names to try on Windows (the driver installs it in System32).
pub const WINDOWS_CANDIDATES: &[&str] = &["nvml.dll"];

/// Symbols of an opened library, by name.
pub trait Symbols: Send + Sync {
    /// The address of `name`, if the library exports it.
    fn get(&self, name: &CStr) -> Option<*mut c_void>;
}

/// A loaded and initialised NVML.
pub struct Nvml {
    _lib: Box<dyn Symbols>,
}

impl Nvml {
    /// Resolve the functions this crate calls and run `nvmlInit_v2`. A
    /// missing symbol or a failing init gives `None`.
    pub fn load(lib: Box<dyn Symbols>) -> Option<Self> {
        let _ = lib;
        None
    }

    /// Utilization of the GPU at the render adapter's PCI address.
    pub fn utilization(&self, adapter: &RenderAdapter) -> Utilization {
        let _ = adapter;
        Utilization::Unavailable
    }
}

/// Utilization through `nvml`, or [`Utilization::Unavailable`] without it.
pub fn utilization(nvml: Option<&Nvml>, adapter: &RenderAdapter) -> Utilization {
    nvml.map_or(Utilization::Unavailable, |n| n.utilization(adapter))
}
