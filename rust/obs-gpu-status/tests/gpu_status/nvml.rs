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
