//! macOS render GPU (obs-rust/obs-studio#1020): the registry ID of the
//! default Metal device (`MTLCreateSystemDefaultDevice`, then `-registryID`)
//! and the registry IDs of the `IOAccelerator` services; and the render
//! GPU's `PerformanceStatistics` for the `GPU: x%` label (#1026). Metal,
//! IOKit, CoreFoundation and the Objective-C runtime are opened at run time
//! through libloading, so the crate adds no framework to the link. The choice between the two sources
//! is [`crate::render_adapter_from_registry`].

use std::ffi::{c_char, c_void};

use libloading::Library;

const METAL: &str = "/System/Library/Frameworks/Metal.framework/Metal";
const IOKIT: &str = "/System/Library/Frameworks/IOKit.framework/IOKit";
const OBJC: &str = "/usr/lib/libobjc.A.dylib";

/// `kIOMainPortDefault` (formerly `kIOMasterPortDefault`).
const IO_MAIN_PORT_DEFAULT: u32 = 0;

fn open(path: &str) -> Option<Library> {
    // SAFETY: system frameworks and the Objective-C runtime, which every
    // Cocoa process (OBS included) has loaded already.
    unsafe { Library::new(path) }.ok()
}

/// The registry ID of the GPU that `MTLCreateSystemDefaultDevice()` returns,
/// the device libobs-metal renders on.
pub(super) fn default_metal_registry_id() -> Option<u64> {
    type CreateDevice = unsafe extern "C" fn() -> *mut c_void;
    type RegisterSel = unsafe extern "C" fn(*const c_char) -> *mut c_void;
    type SendU64 = unsafe extern "C" fn(*mut c_void, *mut c_void) -> u64;
    type SendVoid = unsafe extern "C" fn(*mut c_void, *mut c_void);

    let metal = open(METAL)?;
    let objc = open(OBJC)?;
    // SAFETY: the symbols are read with their documented C types; the
    // libraries stay loaded until the end of this function.
    unsafe {
        let create = *metal
            .get::<CreateDevice>(b"MTLCreateSystemDefaultDevice\0")
            .ok()?;
        let sel = *objc.get::<RegisterSel>(b"sel_registerName\0").ok()?;
        let msg_send = *objc.get::<*mut c_void>(b"objc_msgSend\0").ok()?;
        let device = create();
        if device.is_null() {
            return None;
        }
        // objc_msgSend is called through a cast to each method's exact type,
        // as the arm64 ABI requires.
        let registry_id = std::mem::transmute::<*mut c_void, SendU64>(msg_send);
        let release = std::mem::transmute::<*mut c_void, SendVoid>(msg_send);
        // `-[MTLDevice registryID]` returns `uint64_t`. The device came from a
        // Create function, so this code owns it and releases it.
        let id = registry_id(device, sel(c"registryID".as_ptr()));
        release(device, sel(c"release".as_ptr()));
        Some(id)
    }
}

/// The registry entry IDs of all `IOAccelerator` services (one per GPU).
/// An error gives an empty list.
pub(super) fn accelerator_registry_ids() -> Vec<u64> {
    type Matching = unsafe extern "C" fn(*const c_char) -> *mut c_void;
    type GetServices = unsafe extern "C" fn(u32, *mut c_void, *mut u32) -> i32;
    type Next = unsafe extern "C" fn(u32) -> u32;
    type EntryId = unsafe extern "C" fn(u32, *mut u64) -> i32;
    type Release = unsafe extern "C" fn(u32) -> i32;

    let mut ids = Vec::new();
    let Some(iokit) = open(IOKIT) else {
        return ids;
    };
    // SAFETY: the symbols are read with their documented C types; `iokit`
    // stays loaded until the end of this function. Each io_object_t from the
    // iterator is released once, then the iterator.
    unsafe {
        let (Ok(matching), Ok(get_services), Ok(next), Ok(entry_id), Ok(release)) = (
            iokit.get::<Matching>(b"IOServiceMatching\0"),
            iokit.get::<GetServices>(b"IOServiceGetMatchingServices\0"),
            iokit.get::<Next>(b"IOIteratorNext\0"),
            iokit.get::<EntryId>(b"IORegistryEntryGetRegistryEntryID\0"),
            iokit.get::<Release>(b"IOObjectRelease\0"),
        ) else {
            return ids;
        };
        let dict = matching(c"IOAccelerator".as_ptr());
        if dict.is_null() {
            return ids;
        }
        let mut iter = 0u32;
        // IOServiceGetMatchingServices consumes `dict`, also on failure.
        if get_services(IO_MAIN_PORT_DEFAULT, dict, &mut iter) != 0 {
            return ids;
        }
        loop {
            let service = next(iter);
            if service == 0 {
                break;
            }
            let mut id = 0u64;
            if entry_id(service, &mut id) == 0 {
                ids.push(id);
            }
            release(service);
        }
        release(iter);
    }
    ids
}

const CORE_FOUNDATION: &str = "/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation";

/// `kCFStringEncodingUTF8`.
const CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
/// `kCFNumberSInt64Type`.
const CF_NUMBER_SINT64: isize = 4;

/// The number entries of the `PerformanceStatistics` dictionary of the
/// `IOAccelerator` service with registry entry ID `registry_id`
/// (obs-rust/obs-studio#1026), for [`crate::iokit::device_utilization`].
/// Entries that are not numbers are left out; an error gives an empty list.
#[expect(
    clippy::too_many_lines,
    reason = "FFI table: one block per CoreFoundation call, in order"
)]
pub(super) fn accelerator_statistics(registry_id: u64) -> Vec<(String, i64)> {
    type IdMatching = unsafe extern "C" fn(u64) -> *mut c_void;
    type GetService = unsafe extern "C" fn(u32, *mut c_void) -> u32;
    type CreateProperty =
        unsafe extern "C" fn(u32, *const c_void, *const c_void, u32) -> *const c_void;
    type Release = unsafe extern "C" fn(u32) -> i32;
    type CreateString = unsafe extern "C" fn(*const c_void, *const c_char, u32) -> *const c_void;
    type CfRelease = unsafe extern "C" fn(*const c_void);
    type TypeOf = unsafe extern "C" fn(*const c_void) -> usize;
    type TypeId = unsafe extern "C" fn() -> usize;
    type Count = unsafe extern "C" fn(*const c_void) -> isize;
    type KeysAndValues =
        unsafe extern "C" fn(*const c_void, *mut *const c_void, *mut *const c_void);
    type GetCString = unsafe extern "C" fn(*const c_void, *mut c_char, isize, u32) -> u8;
    type NumberValue = unsafe extern "C" fn(*const c_void, isize, *mut c_void) -> u8;

    let mut stats = Vec::new();
    let (Some(iokit), Some(cf)) = (open(IOKIT), open(CORE_FOUNDATION)) else {
        return stats;
    };
    // SAFETY: the symbols are read with their documented C types; both
    // libraries stay loaded until the end of this function. The service is
    // released once, the key string and the copied dictionary are
    // CFReleased once; keys and values are borrowed from the dictionary and
    // read only while it is alive.
    unsafe {
        let (
            Ok(id_matching),
            Ok(get_service),
            Ok(create_property),
            Ok(release),
            Ok(create_string),
            Ok(cf_release),
            Ok(type_of),
            Ok(dictionary_type),
            Ok(string_type),
            Ok(number_type),
            Ok(count),
            Ok(keys_and_values),
            Ok(get_cstring),
            Ok(number_value),
        ) = (
            iokit.get::<IdMatching>(b"IORegistryEntryIDMatching\0"),
            iokit.get::<GetService>(b"IOServiceGetMatchingService\0"),
            iokit.get::<CreateProperty>(b"IORegistryEntryCreateCFProperty\0"),
            iokit.get::<Release>(b"IOObjectRelease\0"),
            cf.get::<CreateString>(b"CFStringCreateWithCString\0"),
            cf.get::<CfRelease>(b"CFRelease\0"),
            cf.get::<TypeOf>(b"CFGetTypeID\0"),
            cf.get::<TypeId>(b"CFDictionaryGetTypeID\0"),
            cf.get::<TypeId>(b"CFStringGetTypeID\0"),
            cf.get::<TypeId>(b"CFNumberGetTypeID\0"),
            cf.get::<Count>(b"CFDictionaryGetCount\0"),
            cf.get::<KeysAndValues>(b"CFDictionaryGetKeysAndValues\0"),
            cf.get::<GetCString>(b"CFStringGetCString\0"),
            cf.get::<NumberValue>(b"CFNumberGetValue\0"),
        )
        else {
            return stats;
        };
        let dict = id_matching(registry_id);
        if dict.is_null() {
            return stats;
        }
        // IOServiceGetMatchingService consumes `dict`, also on failure.
        let service = get_service(IO_MAIN_PORT_DEFAULT, dict);
        if service == 0 {
            return stats;
        }
        let key = create_string(
            std::ptr::null(),
            c"PerformanceStatistics".as_ptr(),
            CF_STRING_ENCODING_UTF8,
        );
        let property = if key.is_null() {
            std::ptr::null()
        } else {
            let property = create_property(service, key, std::ptr::null(), 0);
            cf_release(key);
            property
        };
        release(service);
        if property.is_null() {
            return stats;
        }
        if type_of(property) == dictionary_type() {
            let n = usize::try_from(count(property)).unwrap_or(0);
            let mut keys = vec![std::ptr::null(); n];
            let mut values = vec![std::ptr::null(); n];
            keys_and_values(property, keys.as_mut_ptr(), values.as_mut_ptr());
            for (&k, &v) in keys.iter().zip(&values) {
                if k.is_null()
                    || v.is_null()
                    || type_of(k) != string_type()
                    || type_of(v) != number_type()
                {
                    continue;
                }
                let mut name = [0 as c_char; 128];
                if get_cstring(
                    k,
                    name.as_mut_ptr(),
                    name.len() as isize,
                    CF_STRING_ENCODING_UTF8,
                ) == 0
                {
                    continue;
                }
                let mut value = 0i64;
                // Returns false for a value that does not fit an i64
                // exactly (a fraction); leave that entry out.
                if number_value(v, CF_NUMBER_SINT64, (&raw mut value).cast()) == 0 {
                    continue;
                }
                let name = std::ffi::CStr::from_ptr(name.as_ptr());
                stats.push((name.to_string_lossy().into_owned(), value));
            }
        }
        cf_release(property);
    }
    stats
}
