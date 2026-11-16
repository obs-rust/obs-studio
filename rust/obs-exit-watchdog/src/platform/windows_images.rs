//! Windows: the loaded modules in load order (#4589), through Toolhelp.
//!
//! `Module32FirstW`/`Module32NextW` list the modules of the loader's
//! load-order list: the executable first, then each DLL when it was loaded,
//! a DLL's dependencies right after it. That is the order the #4549 owner
//! rule needs (`a_library_loaded_later_comes_last` checks it).

use std::ffi::c_void;

use crate::images::Image;

pub const IMAGES_SUPPORTED: bool = true;

const TH32CS_SNAPMODULE: u32 = 0x8;
const INVALID_HANDLE_VALUE: isize = -1;
const ERROR_BAD_LENGTH: u32 = 24;
const MAX_MODULE_NAME32: usize = 255;
const MAX_PATH: usize = 260;

#[repr(C)]
struct ModuleEntry32W {
    size: u32,
    module_id: u32,
    process_id: u32,
    global_usage: u32,
    process_usage: u32,
    base: *mut u8,
    base_size: u32,
    module: *mut c_void,
    name: [u16; MAX_MODULE_NAME32 + 1],
    path: [u16; MAX_PATH],
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateToolhelp32Snapshot(flags: u32, process_id: u32) -> *mut c_void;
    fn Module32FirstW(snapshot: *mut c_void, entry: *mut ModuleEntry32W) -> i32;
    fn Module32NextW(snapshot: *mut c_void, entry: *mut ModuleEntry32W) -> i32;
    fn CloseHandle(handle: *mut c_void) -> i32;
    fn GetLastError() -> u32;
}

/// The modules of this process, in load order; empty if the snapshot fails.
pub fn images() -> Vec<Image> {
    let Some(snapshot) = snapshot() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    // SAFETY: an all-zero MODULEENTRY32W is valid; `size` is set as the
    // API requires; the snapshot is closed once.
    unsafe {
        let mut entry: ModuleEntry32W = std::mem::zeroed();
        entry.size = u32::try_from(size_of::<ModuleEntry32W>()).unwrap_or(u32::MAX);
        let mut more = Module32FirstW(snapshot, &raw mut entry) != 0;
        while more {
            let start = entry.base.addr();
            let len = entry.path.iter().position(|&c| c == 0).unwrap_or(MAX_PATH);
            out.push(Image {
                start,
                end: start + entry.base_size as usize,
                path: String::from_utf16_lossy(&entry.path[..len]),
            });
            more = Module32NextW(snapshot, &raw mut entry) != 0;
        }
        CloseHandle(snapshot);
    }
    out
}

/// A module snapshot of this process. It fails with `ERROR_BAD_LENGTH`
/// while another thread loads or unloads a module, and succeeds on a retry.
fn snapshot() -> Option<*mut c_void> {
    for _ in 0..8 {
        // SAFETY: plain calls; 0 is the own process.
        let (snapshot, error) = unsafe {
            let s = CreateToolhelp32Snapshot(TH32CS_SNAPMODULE, 0);
            (s, GetLastError())
        };
        if snapshot.addr().cast_signed() != INVALID_HANDLE_VALUE {
            return Some(snapshot);
        }
        if error != ERROR_BAD_LENGTH {
            return None;
        }
    }
    None
}
