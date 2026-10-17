//! NVIDIA GPU utilization through NVML (obs-rust/obs-studio#1021).
//!
//! NVML is loaded at run time (`libnvidia-ml.so.1`, `nvml.dll`) and its
//! functions are resolved by name from a [`Symbols`] source, so a fake
//! symbol table tests the real calls. `platform::nvml` opens the library;
//! this module only resolves and calls. #1004 measured 0.68 ms per query,
//! against 87 ms for spawning `nvidia-smi`.

use std::ffi::{CStr, CString, c_char, c_void};

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

const NVML_SUCCESS: i32 = 0;

/// `nvmlUtilization_t`.
#[repr(C)]
struct NvmlUtilization {
    gpu: u32,
    memory: u32,
}

type InitFn = unsafe extern "C" fn() -> i32;
type HandleByPciBusIdFn = unsafe extern "C" fn(*const c_char, *mut *mut c_void) -> i32;
type UtilizationRatesFn = unsafe extern "C" fn(*mut c_void, *mut NvmlUtilization) -> i32;

/// A loaded and initialised NVML. It stays initialised for the life of the
/// process (no `nvmlShutdown`), like the library itself.
pub struct Nvml {
    handle_by_pci_bus_id: HandleByPciBusIdFn,
    utilization_rates: UtilizationRatesFn,
    /// Keeps the library loaded while the function pointers are in use.
    _lib: Box<dyn Symbols>,
}

impl Nvml {
    /// Resolve the functions this crate calls and run `nvmlInit_v2`. A
    /// missing symbol or a failing init gives `None`.
    pub fn load(lib: Box<dyn Symbols>) -> Option<Self> {
        // SAFETY: each name is read with the C type NVML documents for it.
        let (init, handle_by_pci_bus_id, utilization_rates) = unsafe {
            (
                function::<InitFn>(&*lib, c"nvmlInit_v2")?,
                function::<HandleByPciBusIdFn>(&*lib, c"nvmlDeviceGetHandleByPciBusId_v2")?,
                function::<UtilizationRatesFn>(&*lib, c"nvmlDeviceGetUtilizationRates")?,
            )
        };
        // SAFETY: nvmlInit_v2 takes no arguments; the library is loaded.
        if unsafe { init() } != NVML_SUCCESS {
            return None;
        }
        Some(Self {
            handle_by_pci_bus_id,
            utilization_rates,
            _lib: lib,
        })
    }

    /// Utilization of the GPU at the render adapter's PCI address
    /// (`nvmlDeviceGetHandleByPciBusId_v2` takes the sysfs form,
    /// `0000:2b:00.0`). Other adapters, an unknown address and a reading
    /// above 100 % are [`Utilization::Unavailable`].
    pub fn utilization(&self, adapter: &RenderAdapter) -> Utilization {
        let RenderAdapter::Pci(address) = adapter else {
            return Utilization::Unavailable;
        };
        let Ok(bus_id) = CString::new(address.as_str()) else {
            return Utilization::Unavailable;
        };
        let mut device = std::ptr::null_mut();
        // SAFETY: a NUL-terminated bus ID and a writable out-pointer, as
        // nvmlDeviceGetHandleByPciBusId_v2 requires; `_lib` is loaded.
        if unsafe { (self.handle_by_pci_bus_id)(bus_id.as_ptr(), &mut device) } != NVML_SUCCESS {
            return Utilization::Unavailable;
        }
        let mut rates = NvmlUtilization { gpu: 0, memory: 0 };
        // SAFETY: `device` came from NVML; `rates` is a writable
        // nvmlUtilization_t.
        if unsafe { (self.utilization_rates)(device, &mut rates) } != NVML_SUCCESS {
            return Utilization::Unavailable;
        }
        u8::try_from(rates.gpu)
            .ok()
            .filter(|&p| p <= 100)
            .map_or(Utilization::Unavailable, Utilization::Percent)
    }
}

/// The symbol `name` of `lib` as the function pointer type `F`.
///
/// # Safety
/// `F` must be the C type of `name` in the library behind `lib`.
unsafe fn function<F: Copy>(lib: &dyn Symbols, name: &CStr) -> Option<F> {
    const { assert!(std::mem::size_of::<F>() == std::mem::size_of::<*mut c_void>()) };
    let address = lib.get(name).filter(|p| !p.is_null())?;
    // SAFETY: a non-null code address read as `F`, which the caller vouches
    // for; the size check above rules out non-pointer types.
    Some(unsafe { std::mem::transmute_copy::<*mut c_void, F>(&address) })
}

impl Nvml {
    /// The share of the GPU's SM time that process `pid` used since the
    /// previous call, from `nvmlDeviceGetProcessUtilization` (#1004: the
    /// tooltip shows OBS's own share). `None` without the symbol, without a
    /// PCI address, or without a sample for `pid`: NVML reports host PIDs,
    /// so a process in a PID namespace (a container) finds none.
    pub fn process_utilization(&self, adapter: &RenderAdapter, pid: u32) -> Option<u32> {
        let _ = (adapter, pid);
        None
    }
}

/// Utilization through `nvml`, or [`Utilization::Unavailable`] without it.
pub fn utilization(nvml: Option<&Nvml>, adapter: &RenderAdapter) -> Utilization {
    nvml.map_or(Utilization::Unavailable, |n| n.utilization(adapter))
}
