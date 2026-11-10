//! Linux: the loaded images from `dl_iterate_phdr(3)`, in load order (the
//! link map: a `dlopen` appends the library and the dependencies it loads).

use std::ffi::{CStr, c_int, c_void};

use crate::images::Image;

pub const IMAGES_SUPPORTED: bool = true;

/// The loaded images in load order; the main program first. An image's
/// range spans its loadable segments.
pub fn images() -> Vec<Image> {
    let mut out: Vec<Image> = Vec::new();
    // SAFETY: the callback only reads the info it is given, during the call,
    // and writes to `out` through the pointer passed as data.
    unsafe {
        libc::dl_iterate_phdr(Some(add), std::ptr::from_mut(&mut out).cast::<c_void>());
    }
    // The main program has an empty name in the link map.
    if let Some(main) = out.first_mut()
        && main.path.is_empty()
        && let Ok(exe) = std::env::current_exe()
    {
        main.path = exe.to_string_lossy().into_owned();
    }
    out
}

unsafe extern "C" fn add(info: *mut libc::dl_phdr_info, _size: usize, data: *mut c_void) -> c_int {
    // SAFETY: dl_iterate_phdr passes a valid info for this call and the data
    // pointer `images` gave it.
    let (info, out) = unsafe { (&*info, &mut *data.cast::<Vec<Image>>()) };
    let base = info.dlpi_addr as usize;
    let headers: &[libc::Elf64_Phdr] = if info.dlpi_phdr.is_null() {
        &[]
    } else {
        // SAFETY: dlpi_phdr points to dlpi_phnum program headers.
        unsafe { std::slice::from_raw_parts(info.dlpi_phdr, usize::from(info.dlpi_phnum)) }
    };
    let loads = headers.iter().filter(|h| h.p_type == libc::PT_LOAD);
    let start = loads.clone().map(|h| h.p_vaddr as usize).min();
    let end = loads.map(|h| (h.p_vaddr + h.p_memsz) as usize).max();
    if let (Some(start), Some(end)) = (start, end) {
        let path = if info.dlpi_name.is_null() {
            String::new()
        } else {
            // SAFETY: a NUL-terminated name owned by the link map.
            unsafe { CStr::from_ptr(info.dlpi_name) }
                .to_string_lossy()
                .into_owned()
        };
        out.push(Image {
            start: base + start,
            end: base + end,
            path,
        });
    }
    0
}
