//! macOS: the loaded images from dyld, in load order (an image loaded later
//! gets a higher index). An image's range is its `__TEXT` segment: the
//! images of the shared cache share their `__LINKEDIT` and interleave their
//! data, and code addresses are all the watchdog looks up.

use std::ffi::{CStr, c_char};

use crate::images::Image;

pub const IMAGES_SUPPORTED: bool = true;

/// `LC_SEGMENT_64` in <mach-o/loader.h>.
const LC_SEGMENT_64: u32 = 0x19;

/// `struct mach_header_64` in <mach-o/loader.h>.
#[repr(C)]
struct MachHeader64 {
    magic: u32,
    cputype: i32,
    cpusubtype: i32,
    filetype: u32,
    ncmds: u32,
    sizeofcmds: u32,
    flags: u32,
    reserved: u32,
}

/// `struct load_command`.
#[repr(C)]
struct LoadCommand {
    cmd: u32,
    cmdsize: u32,
}

/// `struct segment_command_64`.
#[repr(C)]
struct SegmentCommand64 {
    cmd: u32,
    cmdsize: u32,
    segname: [c_char; 16],
    vmaddr: u64,
    vmsize: u64,
    fileoff: u64,
    filesize: u64,
    maxprot: i32,
    initprot: i32,
    nsects: u32,
    flags: u32,
}

unsafe extern "C" {
    fn _dyld_image_count() -> u32;
    fn _dyld_get_image_header(index: u32) -> *const MachHeader64;
    fn _dyld_get_image_vmaddr_slide(index: u32) -> isize;
    fn _dyld_get_image_name(index: u32) -> *const c_char;
}

/// The loaded images in load order; the main program first.
pub fn images() -> Vec<Image> {
    // SAFETY: dyld's image functions take any index below the count; an
    // image loaded or unloaded meanwhile gives a null header or name, which
    // is skipped.
    let count = unsafe { _dyld_image_count() };
    (0..count)
        .filter_map(|index| unsafe {
            let header = _dyld_get_image_header(index);
            let name = _dyld_get_image_name(index);
            if header.is_null() || name.is_null() {
                return None;
            }
            let (start, size) = text_segment(header)?;
            let start = start.wrapping_add_signed(_dyld_get_image_vmaddr_slide(index));
            Some(Image {
                start,
                end: start + size,
                path: CStr::from_ptr(name).to_string_lossy().into_owned(),
            })
        })
        .collect()
}

/// The `__TEXT` segment's unslid address and size.
///
/// # Safety
/// `header` points to a mapped 64-bit Mach-O header with its load commands.
unsafe fn text_segment(header: *const MachHeader64) -> Option<(usize, usize)> {
    // SAFETY: the load commands follow the header, `ncmds` of them, each
    // `cmdsize` long.
    unsafe {
        let mut command = header.add(1).cast::<u8>();
        for _ in 0..(*header).ncmds {
            let load = &*command.cast::<LoadCommand>();
            if load.cmd == LC_SEGMENT_64 {
                let segment = &*command.cast::<SegmentCommand64>();
                let name = CStr::from_ptr(segment.segname.as_ptr());
                if name.to_bytes() == b"__TEXT" {
                    return Some((segment.vmaddr as usize, segment.vmsize as usize));
                }
            }
            if load.cmdsize == 0 {
                return None;
            }
            command = command.add(load.cmdsize as usize);
        }
        None
    }
}
