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
const NVML_ERROR_INSUFFICIENT_SIZE: i32 = 7;

/// `nvmlUtilization_t`.
#[repr(C)]
struct NvmlUtilization {
    gpu: u32,
    memory: u32,
}

type InitFn = unsafe extern "C" fn() -> i32;
type HandleByPciBusIdFn = unsafe extern "C" fn(*const c_char, *mut *mut c_void) -> i32;
type UtilizationRatesFn = unsafe extern "C" fn(*mut c_void, *mut NvmlUtilization) -> i32;
type ProcessUtilizationFn =
    unsafe extern "C" fn(*mut c_void, *mut NvmlProcessSample, *mut u32, u64) -> i32;

/// `nvmlProcessUtilizationSample_t`.
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct NvmlProcessSample {
    pid: u32,
    time_stamp: u64,
    sm_util: u32,
    mem_util: u32,
    enc_util: u32,
    dec_util: u32,
}

/// `nvmlProcessUtilizationInfo_v1_t`.
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct NvmlProcessUtilizationInfo {
    time_stamp: u64,
    pid: u32,
    sm_util: u32,
    mem_util: u32,
    enc_util: u32,
    dec_util: u32,
    jpg_util: u32,
    ofa_util: u32,
}

/// `nvmlProcessesUtilizationInfo_v1_t`.
#[repr(C)]
struct NvmlProcessesUtilizationInfo {
    version: u32,
    count: u32,
    last_seen: u64,
    array: *mut NvmlProcessUtilizationInfo,
}

/// `nvmlProcessesUtilizationInfo_v1`: `NVML_STRUCT_VERSION(…, 1)`, the
/// struct size with the version in the top byte.
const PROCESSES_UTILIZATION_INFO_V1: u32 =
    std::mem::size_of::<NvmlProcessesUtilizationInfo>() as u32 | (1 << 24);

type ProcessesUtilizationInfoFn =
    unsafe extern "C" fn(*mut c_void, *mut NvmlProcessesUtilizationInfo) -> i32;

/// One process sample from either NVML function: (pid, time stamp, SM %).
type ProcessSample = (u32, u64, u32);

/// A loaded and initialised NVML. It stays initialised for the life of the
/// process (no `nvmlShutdown`), like the library itself.
pub struct Nvml {
    handle_by_pci_bus_id: HandleByPciBusIdFn,
    utilization_rates: UtilizationRatesFn,
    /// Optional, preferred: newer drivers serve graphics processes only
    /// through it (driver 595 returns no samples from the older function).
    processes_utilization_info: Option<ProcessesUtilizationInfoFn>,
    /// Optional fallback for drivers without the newer function.
    process_utilization: Option<ProcessUtilizationFn>,
    /// Per PID, the newest process sample seen, so each call reads only
    /// newer ones and reading one PID does not use up another's samples.
    last_seen: std::sync::Mutex<std::collections::HashMap<u32, u64>>,
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
        // SAFETY: as above; the share is optional, so a missing symbol is None.
        let (processes_utilization_info, process_utilization) = unsafe {
            (
                function::<ProcessesUtilizationInfoFn>(
                    &*lib,
                    c"nvmlDeviceGetProcessesUtilizationInfo",
                ),
                function::<ProcessUtilizationFn>(&*lib, c"nvmlDeviceGetProcessUtilization"),
            )
        };
        // SAFETY: nvmlInit_v2 takes no arguments; the library is loaded.
        if unsafe { init() } != NVML_SUCCESS {
            return None;
        }
        Some(Self {
            handle_by_pci_bus_id,
            utilization_rates,
            processes_utilization_info,
            process_utilization,
            last_seen: std::sync::Mutex::new(std::collections::HashMap::new()),
            _lib: lib,
        })
    }

    /// Utilization of the GPU at the render adapter's PCI address
    /// (`nvmlDeviceGetHandleByPciBusId_v2` takes the sysfs form,
    /// `0000:2b:00.0`). Other adapters, an unknown address and a reading
    /// above 100 % are [`Utilization::Unavailable`].
    pub fn utilization(&self, adapter: &RenderAdapter) -> Utilization {
        let Some(device) = self.device(adapter) else {
            return Utilization::Unavailable;
        };
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

impl Nvml {
    /// The NVML device at the render adapter's PCI address.
    fn device(&self, adapter: &RenderAdapter) -> Option<*mut c_void> {
        let RenderAdapter::Pci(address) = adapter else {
            return None;
        };
        let bus_id = CString::new(address.as_str()).ok()?;
        let mut device = std::ptr::null_mut();
        // SAFETY: a NUL-terminated bus ID and a writable out-pointer, as
        // nvmlDeviceGetHandleByPciBusId_v2 requires; `_lib` is loaded.
        let ret = unsafe { (self.handle_by_pci_bus_id)(bus_id.as_ptr(), &mut device) };
        (ret == NVML_SUCCESS && !device.is_null()).then_some(device)
    }
}

/// Samples newer than `last_seen` from `nvmlDeviceGetProcessesUtilizationInfo`.
/// The size query reports the driver's buffer capacity (72 on driver 595),
/// not the number of samples; the fill call sets the real count.
fn samples_v1_info(
    get: ProcessesUtilizationInfoFn,
    device: *mut c_void,
    last_seen: u64,
) -> Option<Vec<ProcessSample>> {
    let mut info = NvmlProcessesUtilizationInfo {
        version: PROCESSES_UTILIZATION_INFO_V1,
        count: 0,
        last_seen,
        array: std::ptr::null_mut(),
    };
    // SAFETY: a versioned struct with a null array asks for the size.
    if unsafe { get(device, &mut info) } != NVML_ERROR_INSUFFICIENT_SIZE || info.count == 0 {
        return None;
    }
    let mut buffer = vec![NvmlProcessUtilizationInfo::default(); info.count as usize];
    info.array = buffer.as_mut_ptr();
    // SAFETY: `array` holds `count` writable entries.
    if unsafe { get(device, &mut info) } != NVML_SUCCESS {
        return None;
    }
    buffer.truncate(info.count as usize);
    Some(
        buffer
            .iter()
            .map(|s| (s.pid, s.time_stamp, s.sm_util))
            .collect(),
    )
}

/// Samples newer than `last_seen` from the older
/// `nvmlDeviceGetProcessUtilization` (same size protocol, plain arguments).
fn samples_v0(
    get: ProcessUtilizationFn,
    device: *mut c_void,
    last_seen: u64,
) -> Option<Vec<ProcessSample>> {
    let mut count = 0u32;
    // SAFETY: a null buffer with count 0 asks NVML for the size.
    let ret = unsafe { get(device, std::ptr::null_mut(), &mut count, last_seen) };
    if ret != NVML_ERROR_INSUFFICIENT_SIZE || count == 0 {
        return None;
    }
    let mut buffer = vec![NvmlProcessSample::default(); count as usize];
    // SAFETY: `buffer` holds `count` writable entries.
    if unsafe { get(device, buffer.as_mut_ptr(), &mut count, last_seen) } != NVML_SUCCESS {
        return None;
    }
    buffer.truncate(count as usize);
    Some(
        buffer
            .iter()
            .map(|s| (s.pid, s.time_stamp, s.sm_util))
            .collect(),
    )
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
    /// The SM utilization of process `pid` since the previous call (#1004:
    /// the tooltip shows OBS's own share), from
    /// `nvmlDeviceGetProcessesUtilizationInfo`, or the older
    /// `nvmlDeviceGetProcessUtilization` on drivers without it. `None`
    /// without either symbol, without a PCI address, or without a sample for
    /// `pid` in the window (no GPU work). The driver reports PIDs in the
    /// caller's PID namespace (checked in an LXC container, driver 595).
    pub fn process_utilization(&self, adapter: &RenderAdapter, pid: u32) -> Option<u32> {
        let device = self.device(adapter)?;
        let mut seen = self.last_seen.lock().unwrap_or_else(|e| e.into_inner());
        let last_seen = seen.get(&pid).copied().unwrap_or(0);
        let samples = match (self.processes_utilization_info, self.process_utilization) {
            (Some(get), _) => samples_v1_info(get, device, last_seen)?,
            (None, Some(get)) => samples_v0(get, device, last_seen)?,
            (None, None) => return None,
        };
        if let Some(newest) = samples.iter().map(|&(_, ts, _)| ts).max() {
            seen.insert(pid, last_seen.max(newest));
        }
        let own: Vec<u32> = samples
            .iter()
            .filter(|&&(p, _, _)| p == pid)
            .map(|&(_, _, sm)| sm)
            .collect();
        if own.is_empty() {
            return None;
        }
        let mean = own.iter().map(|&u| u64::from(u)).sum::<u64>() / own.len() as u64;
        u32::try_from(mean).ok().filter(|&m| m <= 100)
    }
}

/// Utilization through `nvml`, or [`Utilization::Unavailable`] without it.
pub fn utilization(nvml: Option<&Nvml>, adapter: &RenderAdapter) -> Utilization {
    nvml.map_or(Utilization::Unavailable, |n| n.utilization(adapter))
}
