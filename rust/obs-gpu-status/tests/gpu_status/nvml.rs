//! Tier 1 tests for obs-rust/obs-studio#1021: NVML utilization through a
//! fake symbol table. The fake functions are real `extern "C"` functions,
//! so the resolve-and-call code runs as it does against libnvidia-ml.

#![allow(unsafe_code)] // the fake NVML functions write through out-pointers

use std::collections::HashMap;
use std::ffi::{CStr, c_char, c_void};

use obs_gpu_status::nvml::{LINUX_CANDIDATES, Nvml, Symbols, WINDOWS_CANDIDATES, utilization};
use obs_gpu_status::{RenderAdapter, Utilization};

const NVML_SUCCESS: i32 = 0;
const NVML_ERROR_NOT_FOUND: i32 = 6;
const NVML_ERROR_DRIVER_NOT_LOADED: i32 = 9;

/// The PCI address of the fake GPU and the handle NVML gives for it.
const BUS_ID: &str = "0000:2b:00.0";
const HANDLE: usize = 0x3060;

#[repr(C)]
struct NvmlUtilization {
    gpu: u32,
    memory: u32,
}

extern "C" fn init_ok() -> i32 {
    NVML_SUCCESS
}

extern "C" fn init_no_driver() -> i32 {
    NVML_ERROR_DRIVER_NOT_LOADED
}

extern "C" fn handle_by_bus_id(bus_id: *const c_char, device: *mut *mut c_void) -> i32 {
    // SAFETY: NVML's contract: a NUL-terminated bus ID and a writable out-pointer.
    let bus_id = unsafe { CStr::from_ptr(bus_id) };
    if bus_id.to_str() != Ok(BUS_ID) {
        return NVML_ERROR_NOT_FOUND;
    }
    unsafe { *device = HANDLE as *mut c_void };
    NVML_SUCCESS
}

macro_rules! utilization_rates {
    ($name:ident, $gpu:expr) => {
        extern "C" fn $name(device: *mut c_void, out: *mut NvmlUtilization) -> i32 {
            if device as usize != HANDLE {
                return NVML_ERROR_NOT_FOUND;
            }
            // SAFETY: NVML's contract: a writable out-pointer.
            unsafe {
                *out = NvmlUtilization {
                    gpu: $gpu,
                    memory: 12,
                }
            };
            NVML_SUCCESS
        }
    };
}
utilization_rates!(rates_37, 37);
utilization_rates!(rates_250, 250);

#[derive(Default)]
struct FakeLib(HashMap<&'static CStr, usize>);

impl FakeLib {
    fn with(mut self, name: &'static CStr, f: *const ()) -> Self {
        self.0.insert(name, f as usize);
        self
    }

    fn full(init: *const (), rates: *const ()) -> Self {
        Self::default()
            .with(c"nvmlInit_v2", init)
            .with(
                c"nvmlDeviceGetHandleByPciBusId_v2",
                handle_by_bus_id as *const (),
            )
            .with(c"nvmlDeviceGetUtilizationRates", rates)
    }
}

impl Symbols for FakeLib {
    fn get(&self, name: &CStr) -> Option<*mut c_void> {
        self.0.get(name).map(|&a| a as *mut c_void)
    }
}

fn pci(addr: &str) -> RenderAdapter {
    RenderAdapter::Pci(addr.into())
}

#[test]
fn utilization_maps_to_a_percent() {
    let nvml = Nvml::load(Box::new(FakeLib::full(
        init_ok as *const (),
        rates_37 as *const (),
    )))
    .expect("fake NVML loads");
    assert_eq!(nvml.utilization(&pci(BUS_ID)), Utilization::Percent(37));
}

#[test]
fn missing_library_is_unavailable() {
    assert_eq!(utilization(None, &pci(BUS_ID)), Utilization::Unavailable);
}

#[test]
fn missing_symbol_is_unavailable() {
    let lib = FakeLib::default()
        .with(c"nvmlInit_v2", init_ok as *const ())
        .with(
            c"nvmlDeviceGetHandleByPciBusId_v2",
            handle_by_bus_id as *const (),
        );
    assert!(Nvml::load(Box::new(lib)).is_none());
}

#[test]
fn failing_init_is_unavailable() {
    let lib = FakeLib::full(init_no_driver as *const (), rates_37 as *const ());
    assert!(Nvml::load(Box::new(lib)).is_none());
}

#[test]
fn gpu_nvml_does_not_know_is_unavailable() {
    // An AMD or Intel card next to an NVIDIA one: NVML has no handle for it.
    let nvml = Nvml::load(Box::new(FakeLib::full(
        init_ok as *const (),
        rates_37 as *const (),
    )))
    .unwrap();
    assert_eq!(
        nvml.utilization(&pci("0000:2c:00.0")),
        Utilization::Unavailable
    );
}

#[test]
fn adapter_without_pci_address_is_unavailable() {
    let nvml = Nvml::load(Box::new(FakeLib::full(
        init_ok as *const (),
        rates_37 as *const (),
    )))
    .unwrap();
    for adapter in [
        RenderAdapter::Ambiguous,
        RenderAdapter::Unknown,
        RenderAdapter::RegistryId(0x1_0000_0b5b),
        RenderAdapter::Luid(0xd2a1),
    ] {
        assert_eq!(
            nvml.utilization(&adapter),
            Utilization::Unavailable,
            "{adapter:?}"
        );
    }
}

#[test]
fn percent_above_100_is_unavailable() {
    let nvml = Nvml::load(Box::new(FakeLib::full(
        init_ok as *const (),
        rates_250 as *const (),
    )))
    .unwrap();
    assert_eq!(nvml.utilization(&pci(BUS_ID)), Utilization::Unavailable);
}

#[test]
fn candidates_cover_the_loader_path_and_nixos() {
    assert_eq!(LINUX_CANDIDATES[0], "libnvidia-ml.so.1");
    assert!(LINUX_CANDIDATES.contains(&"/run/opengl-driver/lib/libnvidia-ml.so.1"));
    assert_eq!(WINDOWS_CANDIDATES, &["nvml.dll"]);
}

// nvmlDeviceGetProcessUtilization (the tooltip's "OBS's own share").

const NVML_ERROR_INSUFFICIENT_SIZE: i32 = 7;
const OBS_PID: u32 = 4242;

/// `nvmlProcessUtilizationSample_t`.
#[repr(C)]
#[derive(Clone, Copy)]
struct Sample {
    pid: u32,
    time_stamp: u64,
    sm_util: u32,
    mem_util: u32,
    enc_util: u32,
    dec_util: u32,
}

const fn sample(pid: u32, time_stamp: u64, sm_util: u32) -> Sample {
    Sample {
        pid,
        time_stamp,
        sm_util,
        mem_util: 0,
        enc_util: 0,
        dec_util: 0,
    }
}

/// OBS twice (20 % and 30 %) and another process (50 %).
const SAMPLES: [Sample; 3] = [
    sample(OBS_PID, 100, 20),
    sample(999, 150, 50),
    sample(OBS_PID, 200, 30),
];

/// The NVML size protocol: without samples newer than `last_seen`,
/// NVML_ERROR_NOT_FOUND; a call with too small a buffer gets
/// NVML_ERROR_INSUFFICIENT_SIZE and the needed count; then the samples. Each
/// generated function records the `last_seen` of every call in its own
/// static, so tests running in parallel do not mix.
macro_rules! process_utilization {
    ($name:ident, $seen:ident) => {
        static $seen: std::sync::Mutex<Vec<u64>> = std::sync::Mutex::new(Vec::new());

        extern "C" fn $name(
            device: *mut c_void,
            out: *mut Sample,
            count: *mut u32,
            last_seen: u64,
        ) -> i32 {
            if device as usize != HANDLE {
                return NVML_ERROR_NOT_FOUND;
            }
            $seen
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(last_seen);
            let fresh: Vec<Sample> = SAMPLES
                .iter()
                .copied()
                .filter(|s| s.time_stamp > last_seen)
                .collect();
            if fresh.is_empty() {
                return NVML_ERROR_NOT_FOUND;
            }
            // SAFETY: NVML's contract: `count` is writable, `out` holds `*count`.
            unsafe {
                if out.is_null() || (*count as usize) < fresh.len() {
                    *count = fresh.len() as u32;
                    return NVML_ERROR_INSUFFICIENT_SIZE;
                }
                for (i, s) in fresh.iter().enumerate() {
                    *out.add(i) = *s;
                }
                *count = fresh.len() as u32;
            }
            NVML_SUCCESS
        }
    };
}
process_utilization!(process_utilization_seen, SEEN);
process_utilization!(process_utilization, UNUSED_SEEN);

fn with_process(lib: FakeLib) -> FakeLib {
    lib.with(
        c"nvmlDeviceGetProcessUtilization",
        process_utilization as *const (),
    )
}

#[test]
fn process_share_is_the_mean_of_its_samples_and_only_new_samples_count() {
    let lib = FakeLib::full(init_ok as *const (), rates_37 as *const ()).with(
        c"nvmlDeviceGetProcessUtilization",
        process_utilization_seen as *const (),
    );
    let nvml = Nvml::load(Box::new(lib)).unwrap();
    assert_eq!(nvml.process_utilization(&pci(BUS_ID), OBS_PID), Some(25));
    // The next call asks only for samples after the newest one seen (200):
    // there are none, so there is no share. Calls: size and fill with 0,
    // then size with 200.
    assert_eq!(nvml.process_utilization(&pci(BUS_ID), OBS_PID), None);
    assert_eq!(*SEEN.lock().unwrap(), vec![0, 0, 200]);
}

#[test]
fn process_without_samples_has_no_share() {
    // A process without GPU work in the window has no sample.
    let nvml = Nvml::load(Box::new(with_process(FakeLib::full(
        init_ok as *const (),
        rates_37 as *const (),
    ))))
    .unwrap();
    assert_eq!(nvml.process_utilization(&pci(BUS_ID), 31337), None);
}

#[test]
fn process_share_is_optional() {
    // Older drivers lack the symbol: the whole-GPU value still works.
    let nvml = Nvml::load(Box::new(FakeLib::full(
        init_ok as *const (),
        rates_37 as *const (),
    )))
    .unwrap();
    assert_eq!(nvml.utilization(&pci(BUS_ID)), Utilization::Percent(37));
    assert_eq!(nvml.process_utilization(&pci(BUS_ID), OBS_PID), None);
}

// nvmlDeviceGetProcessesUtilizationInfo: driver 595.104.02 (RTX 3060) gives
// graphics processes (OBS, Xorg) only through this one; the older
// nvmlDeviceGetProcessUtilization returns NVML_ERROR_NOT_FOUND there.

const NVML_ERROR_ARGUMENT_VERSION_MISMATCH: i32 = 25;

/// `nvmlProcessUtilizationInfo_v1_t`.
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct InfoSample {
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
struct InfoV1 {
    version: u32,
    count: u32,
    last_seen: u64,
    array: *mut InfoSample,
}

/// `nvmlProcessesUtilizationInfo_v1`: NVML_STRUCT_VERSION(…, 1).
const INFO_V1: u32 = std::mem::size_of::<InfoV1>() as u32 | (1 << 24);

/// The size query of the real driver reports its buffer capacity (72), not
/// the number of samples; the fill call then sets the real count.
const DRIVER_CAPACITY: u32 = 72;

macro_rules! processes_utilization_info {
    ($name:ident, $seen:ident) => {
        static $seen: std::sync::Mutex<Vec<u64>> = std::sync::Mutex::new(Vec::new());

        extern "C" fn $name(device: *mut c_void, info: *mut InfoV1) -> i32 {
            if device as usize != HANDLE {
                return NVML_ERROR_NOT_FOUND;
            }
            // SAFETY: NVML's contract: a writable, versioned struct.
            let info = unsafe { &mut *info };
            if info.version != INFO_V1 {
                return NVML_ERROR_ARGUMENT_VERSION_MISMATCH;
            }
            $seen
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(info.last_seen);
            let fresh: Vec<Sample> = SAMPLES
                .iter()
                .copied()
                .filter(|s| s.time_stamp > info.last_seen)
                .collect();
            if fresh.is_empty() {
                return NVML_ERROR_NOT_FOUND;
            }
            if info.array.is_null() || info.count < DRIVER_CAPACITY {
                info.count = DRIVER_CAPACITY;
                return NVML_ERROR_INSUFFICIENT_SIZE;
            }
            for (i, s) in fresh.iter().enumerate() {
                // SAFETY: `array` holds `count` (>= the capacity) entries.
                unsafe {
                    *info.array.add(i) = InfoSample {
                        time_stamp: s.time_stamp,
                        pid: s.pid,
                        sm_util: s.sm_util,
                        ..Default::default()
                    }
                };
            }
            info.count = fresh.len() as u32;
            NVML_SUCCESS
        }
    };
}
processes_utilization_info!(info_seen, INFO_SEEN);
processes_utilization_info!(info_plain, UNUSED_INFO_SEEN);

/// Driver 595: the old function exists but finds no samples.
extern "C" fn process_utilization_595(_: *mut c_void, _: *mut Sample, _: *mut u32, _: u64) -> i32 {
    NVML_ERROR_NOT_FOUND
}

#[test]
fn processes_utilization_info_gives_the_share_and_only_new_samples() {
    let lib = FakeLib::full(init_ok as *const (), rates_37 as *const ()).with(
        c"nvmlDeviceGetProcessesUtilizationInfo",
        info_seen as *const (),
    );
    let nvml = Nvml::load(Box::new(lib)).unwrap();
    assert_eq!(nvml.process_utilization(&pci(BUS_ID), OBS_PID), Some(25));
    assert_eq!(nvml.process_utilization(&pci(BUS_ID), OBS_PID), None);
    // Size and fill with 0, then size with the newest sample seen (200).
    assert_eq!(*INFO_SEEN.lock().unwrap(), vec![0, 0, 200]);
}

#[test]
fn processes_utilization_info_is_preferred_over_the_old_function() {
    // As on driver 595: both symbols exist, only the new one has samples.
    let lib = FakeLib::full(init_ok as *const (), rates_37 as *const ())
        .with(
            c"nvmlDeviceGetProcessUtilization",
            process_utilization_595 as *const (),
        )
        .with(
            c"nvmlDeviceGetProcessesUtilizationInfo",
            info_plain as *const (),
        );
    let nvml = Nvml::load(Box::new(lib)).unwrap();
    assert_eq!(nvml.process_utilization(&pci(BUS_ID), OBS_PID), Some(25));
}

#[test]
fn each_pid_keeps_its_own_window() {
    // Reading OBS's share must not use up the other process's samples.
    let lib = FakeLib::full(init_ok as *const (), rates_37 as *const ()).with(
        c"nvmlDeviceGetProcessesUtilizationInfo",
        info_plain as *const (),
    );
    let nvml = Nvml::load(Box::new(lib)).unwrap();
    assert_eq!(nvml.process_utilization(&pci(BUS_ID), OBS_PID), Some(25));
    assert_eq!(nvml.process_utilization(&pci(BUS_ID), 999), Some(50));
    assert_eq!(nvml.process_utilization(&pci(BUS_ID), OBS_PID), None);
}
