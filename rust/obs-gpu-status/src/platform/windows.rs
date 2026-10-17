//! Windows GPU enumeration through DXGI (obs-rust/obs-studio#942):
//! `CreateDXGIFactory1`, then `IDXGIFactory1::EnumAdapters1` and
//! `IDXGIAdapter1::GetDesc1`, called through their COM vtables. The mapping
//! to [`GpuDevice`] is [`crate::dxgi::devices_from_dxgi`].

use std::ffi::c_void;

use crate::dxgi::AdapterDesc;

#[repr(C)]
struct Guid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

/// `IID_IDXGIFactory1`: 770aae78-f26f-4dba-a829-253c83d1b387.
const IID_IDXGI_FACTORY1: Guid = Guid {
    data1: 0x770a_ae78,
    data2: 0xf26f,
    data3: 0x4dba,
    data4: [0xa8, 0x29, 0x25, 0x3c, 0x83, 0xd1, 0xb3, 0x87],
};

/// `DXGI_ERROR_NOT_FOUND`: `EnumAdapters1` ran past the last adapter.
const DXGI_ERROR_NOT_FOUND: i32 = 0x887A_0002_u32 as i32;

/// `DXGI_ADAPTER_DESC1`.
#[repr(C)]
struct AdapterDesc1 {
    description: [u16; 128],
    vendor_id: u32,
    device_id: u32,
    sub_sys_id: u32,
    revision: u32,
    dedicated_video_memory: usize,
    dedicated_system_memory: usize,
    shared_system_memory: usize,
    adapter_luid_low: u32,
    adapter_luid_high: i32,
    flags: u32,
}

// `sizeof(DXGI_ADAPTER_DESC1)` from dxgi.h on 64-bit Windows.
#[cfg(target_pointer_width = "64")]
const _: () = assert!(std::mem::size_of::<AdapterDesc1>() == 312);

// Vtable slots: IUnknown 0-2, IDXGIObject 3-6, IDXGIFactory 7-11,
// IDXGIFactory1 12-13; IDXGIAdapter 7-9, IDXGIAdapter1 10.
const SLOT_RELEASE: usize = 2;
const SLOT_ENUM_ADAPTERS1: usize = 12;
const SLOT_GET_DESC1: usize = 10;

#[link(name = "dxgi")]
unsafe extern "system" {
    fn CreateDXGIFactory1(riid: *const Guid, factory: *mut *mut c_void) -> i32;
}

/// The vtable entry at `slot` of COM object `object`.
///
/// # Safety
/// `object` is a live COM interface pointer whose vtable has `slot`.
unsafe fn vtable_slot(object: *mut c_void, slot: usize) -> *const c_void {
    // SAFETY: a COM object starts with a pointer to its vtable of function
    // pointers; the caller guarantees the slot exists.
    unsafe { *(*(object as *const *const *const c_void)).add(slot) }
}

/// # Safety
/// `object` is a live COM interface pointer the caller owns one reference to.
unsafe fn release(object: *mut c_void) {
    // SAFETY: slot 2 of every COM interface is IUnknown::Release.
    unsafe {
        let f: unsafe extern "system" fn(*mut c_void) -> u32 =
            std::mem::transmute(vtable_slot(object, SLOT_RELEASE));
        f(object);
    }
}

/// Every adapter DXGI lists, software ones included; empty when DXGI fails.
pub fn dxgi_adapters() -> Vec<AdapterDesc> {
    let mut adapters = Vec::new();
    let mut factory: *mut c_void = std::ptr::null_mut();
    // SAFETY: a valid IID and out-pointer; on success we own `factory`.
    if unsafe { CreateDXGIFactory1(&IID_IDXGI_FACTORY1, &mut factory) } < 0 || factory.is_null() {
        return adapters;
    }
    // SAFETY: `factory` is a live IDXGIFactory1 (slots checked above).
    let enum_adapters1: unsafe extern "system" fn(*mut c_void, u32, *mut *mut c_void) -> i32 =
        unsafe { std::mem::transmute(vtable_slot(factory, SLOT_ENUM_ADAPTERS1)) };
    for index in 0u32.. {
        let mut adapter: *mut c_void = std::ptr::null_mut();
        // SAFETY: a live factory and an out-pointer; on success we own `adapter`.
        let hr = unsafe { enum_adapters1(factory, index, &mut adapter) };
        if hr == DXGI_ERROR_NOT_FOUND || hr < 0 || adapter.is_null() {
            break;
        }
        // SAFETY: `adapter` is a live IDXGIAdapter1; `desc` is writable and
        // fully initialised by a successful GetDesc1.
        unsafe {
            let get_desc1: unsafe extern "system" fn(*mut c_void, *mut AdapterDesc1) -> i32 =
                std::mem::transmute(vtable_slot(adapter, SLOT_GET_DESC1));
            let mut desc: AdapterDesc1 = std::mem::zeroed();
            if get_desc1(adapter, &mut desc) >= 0 {
                let len = desc.description.iter().position(|&c| c == 0).unwrap_or(128);
                adapters.push(AdapterDesc {
                    description: String::from_utf16_lossy(&desc.description[..len]),
                    vendor_id: desc.vendor_id,
                    device_id: desc.device_id,
                    flags: desc.flags,
                    luid: (u64::from(desc.adapter_luid_high as u32) << 32)
                        | u64::from(desc.adapter_luid_low),
                });
            }
            release(adapter);
        }
    }
    // SAFETY: we own one reference to `factory`.
    unsafe { release(factory) };
    adapters
}
