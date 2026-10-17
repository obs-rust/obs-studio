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

// PDH: the `GPU Engine` counter set (obs-rust/obs-studio#1022). The parsing
// and aggregation of the samples is [`crate::pdh::pdh_utilization`].

/// `PDH_FMT_COUNTERVALUE`: a status and a union whose `double` member we read.
#[repr(C)]
struct PdhFmtCounterValue {
    c_status: u32,
    double_value: f64,
}

/// `PDH_FMT_COUNTERVALUE_ITEM_W`.
#[repr(C)]
struct PdhFmtCounterValueItemW {
    name: *const u16,
    value: PdhFmtCounterValue,
}

// `sizeof(PDH_FMT_COUNTERVALUE_ITEM_W)` from pdh.h (24 on both widths: the
// union holds a double, so the value is 8-aligned).
const _: () = assert!(std::mem::size_of::<PdhFmtCounterValueItemW>() == 24);

const PDH_FMT_DOUBLE: u32 = 0x0000_0200;
/// Per-process shares are summed afterwards, so PDH must not cap them.
const PDH_FMT_NOCAP100: u32 = 0x0000_8000;
const PDH_MORE_DATA: u32 = 0x8000_07D2;
const PDH_CSTATUS_VALID_DATA: u32 = 0;
const PDH_CSTATUS_NEW_DATA: u32 = 1;

/// The rate counter needs two collections this far apart.
const PDH_SAMPLE_INTERVAL: std::time::Duration = std::time::Duration::from_millis(100);

#[link(name = "pdh")]
unsafe extern "system" {
    fn PdhOpenQueryW(data_source: *const u16, user_data: usize, query: *mut isize) -> u32;
    fn PdhAddEnglishCounterW(
        query: isize,
        path: *const u16,
        user_data: usize,
        counter: *mut isize,
    ) -> u32;
    fn PdhCollectQueryData(query: isize) -> u32;
    fn PdhGetFormattedCounterArrayW(
        counter: isize,
        format: u32,
        buffer_size: *mut u32,
        item_count: *mut u32,
        items: *mut PdhFmtCounterValueItemW,
    ) -> u32;
    fn PdhCloseQuery(query: isize) -> u32;
}

/// Closes the query on every return path.
struct Query(isize);

impl Drop for Query {
    fn drop(&mut self) {
        // SAFETY: `self.0` came from a successful PdhOpenQueryW and is closed once.
        unsafe { PdhCloseQuery(self.0) };
    }
}

/// An open `\GPU Engine(*)\Utilization Percentage` query (#1022): opened
/// and given its counter once, then one collection per tick, with the
/// previous tick as the rate counter's baseline.
pub struct EngineQuery {
    query: Query,
    counter: isize,
    primed: bool,
}

impl EngineQuery {
    /// `None` when PDH fails (no GPU, counters disabled).
    pub fn open() -> Option<Self> {
        let mut handle: isize = 0;
        // SAFETY: a null data source (live data) and a valid out-pointer.
        if unsafe { PdhOpenQueryW(std::ptr::null(), 0, &mut handle) } != 0 {
            return None;
        }
        let query = Query(handle);
        let path: Vec<u16> = "\\GPU Engine(*)\\Utilization Percentage\0"
            .encode_utf16()
            .collect();
        let mut counter: isize = 0;
        // SAFETY: a live query, a NUL-terminated wide path and a valid out-pointer.
        if unsafe { PdhAddEnglishCounterW(query.0, path.as_ptr(), 0, &mut counter) } != 0 {
            return None;
        }
        Some(Self {
            query,
            counter,
            primed: false,
        })
    }
}

impl crate::pdh::EngineCounters for EngineQuery {
    fn collect(&mut self) -> Option<Vec<(String, f64)>> {
        // SAFETY: a live query with one counter.
        if unsafe { PdhCollectQueryData(self.query.0) } != 0 {
            return None;
        }
        if !self.primed {
            self.primed = true;
            return Some(Vec::new());
        }
        Some(read_counter(self.counter))
    }
}

/// One sample of `\GPU Engine(*)\Utilization Percentage`: every instance
/// name with its value. Empty when PDH fails (no GPU, counters disabled).
/// Blocks for [`PDH_SAMPLE_INTERVAL`]; the label keeps an [`EngineQuery`]
/// open instead.
pub fn gpu_engine_samples() -> Vec<(String, f64)> {
    use crate::pdh::EngineCounters;
    let Some(mut query) = EngineQuery::open() else {
        return Vec::new();
    };
    if query.collect().is_none() {
        return Vec::new();
    }
    std::thread::sleep(PDH_SAMPLE_INTERVAL);
    query.collect().unwrap_or_default()
}

/// The values of a collected wildcard counter, by instance name. Empty when
/// PDH has none (for example no GPU process yet).
fn read_counter(counter: isize) -> Vec<(String, f64)> {
    let format = PDH_FMT_DOUBLE | PDH_FMT_NOCAP100;
    // Instances come and go between the sizing call and the read, so retry.
    for _ in 0..3 {
        let mut size: u32 = 0;
        let mut count: u32 = 0;
        // SAFETY: a null buffer with size 0 asks PDH for the size needed.
        let status = unsafe {
            PdhGetFormattedCounterArrayW(
                counter,
                format,
                &mut size,
                &mut count,
                std::ptr::null_mut(),
            )
        };
        if status != PDH_MORE_DATA || size == 0 {
            return Vec::new();
        }
        // u64 storage keeps the items 8-aligned; PDH puts the names after them.
        let mut buffer = vec![0u64; (size as usize).div_ceil(8)];
        let items = buffer.as_mut_ptr().cast::<PdhFmtCounterValueItemW>();
        // SAFETY: `buffer` holds at least `size` writable, 8-aligned bytes.
        let status =
            unsafe { PdhGetFormattedCounterArrayW(counter, format, &mut size, &mut count, items) };
        if status == PDH_MORE_DATA {
            continue;
        }
        if status != 0 {
            return Vec::new();
        }
        let mut samples = Vec::with_capacity(count as usize);
        for i in 0..count as usize {
            // SAFETY: PDH wrote `count` items into `buffer`; each name is a
            // NUL-terminated wide string inside `buffer`, alive until it drops.
            unsafe {
                let item = &*items.add(i);
                let status = item.value.c_status;
                if (status != PDH_CSTATUS_VALID_DATA && status != PDH_CSTATUS_NEW_DATA)
                    || item.name.is_null()
                {
                    continue;
                }
                let mut len = 0;
                while *item.name.add(len) != 0 {
                    len += 1;
                }
                let name = String::from_utf16_lossy(std::slice::from_raw_parts(item.name, len));
                samples.push((name, item.value.double_value));
            }
        }
        return samples;
    }
    Vec::new()
}
