//! macOS render GPU (obs-rust/obs-studio#1020): the registry ID of the
//! default Metal device (`MTLCreateSystemDefaultDevice`, then `-registryID`)
//! and the registry IDs of the `IOAccelerator` services. Metal, IOKit and
//! the Objective-C runtime are opened at run time through libloading, so the
//! crate adds no framework to the link. The choice between the two sources
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
