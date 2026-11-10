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
pub const STACK_SUPPORTED: bool = true;

type MachPort = u32;

unsafe extern "C" {
    fn pthread_mach_thread_np(thread: libc::pthread_t) -> MachPort;
    fn pthread_get_stackaddr_np(thread: libc::pthread_t) -> *mut std::ffi::c_void;
    fn pthread_get_stacksize_np(thread: libc::pthread_t) -> usize;
    fn thread_suspend(thread: MachPort) -> i32;
    fn thread_resume(thread: MachPort) -> i32;
    fn thread_get_state(thread: MachPort, flavor: i32, state: *mut u32, count: *mut u32) -> i32;
}

/// A thread: its Mach port and its stack's bounds (the frame walk stays in
/// them).
pub struct Thread {
    port: MachPort,
    stack_low: usize,
    stack_high: usize,
}

pub fn current_thread() -> Thread {
    // SAFETY: the calling thread's own pthread handle, port and stack.
    unsafe {
        let me = libc::pthread_self();
        let high = pthread_get_stackaddr_np(me) as usize;
        Thread {
            port: pthread_mach_thread_np(me),
            stack_low: high.saturating_sub(pthread_get_stacksize_np(me)),
            stack_high: high,
        }
    }
}

/// Nothing to load: the walk reads registers and memory only.
pub fn prepare_stack() {}

/// Return addresses carry a pointer authentication code in their high bits
/// on arm64e code; user space addresses fit in 47 bits.
const ADDRESS_MASK: usize = (1 << 47) - 1;

/// The thread's return addresses, innermost first: it is suspended, its
/// program counter and frame pointer read, and the chain of frame records
/// walked (macOS code keeps frame pointers on both architectures).
pub fn capture_stack(thread: &Thread, out: &mut [usize]) -> usize {
    // SAFETY: the port is a live thread of this process; it is resumed after.
    if unsafe { thread_suspend(thread.port) } != 0 {
        return 0;
    }
    let n = match registers(thread.port) {
        Some((pc, fp)) => walk(thread, pc, fp, out),
        None => 0,
    };
    // SAFETY: as above.
    unsafe { thread_resume(thread.port) };
    n
}

/// Follows the frame records `[previous fp, return address]` from `fp`
/// while they stay inside the thread's stack and go up.
fn walk(thread: &Thread, pc: usize, mut fp: usize, out: &mut [usize]) -> usize {
    let mut n = 0;
    let cap = out.len();
    let mut push = |a: usize, n: &mut usize| {
        if *n < cap && a != 0 {
            out[*n] = a & ADDRESS_MASK;
            *n += 1;
        }
    };
    push(pc, &mut n);
    while n < cap && fp.is_multiple_of(8) && fp >= thread.stack_low && fp + 16 <= thread.stack_high {
        // SAFETY: fp is inside the suspended thread's stack, aligned, and
        // its 16-byte frame record ends inside it too.
        let (next, ret) = unsafe { (*(fp as *const usize), *((fp + 8) as *const usize)) };
        push(ret, &mut n);
        if next <= fp {
            break;
        }
        fp = next;
    }
    n
}

/// The suspended thread's program counter and frame pointer.
#[cfg(target_arch = "aarch64")]
fn registers(port: MachPort) -> Option<(usize, usize)> {
    /// `ARM_THREAD_STATE64`: x0-x28, fp, lr, sp, pc, cpsr, flags.
    const FLAVOR: i32 = 6;
    let mut state = [0u64; 34];
    let mut count = 68u32;
    // SAFETY: the buffer holds `count` 32-bit words.
    let kr = unsafe { thread_get_state(port, FLAVOR, state.as_mut_ptr().cast(), &mut count) };
    (kr == 0).then(|| (state[32] as usize, state[29] as usize))
}

/// The suspended thread's program counter and frame pointer.
#[cfg(target_arch = "x86_64")]
fn registers(port: MachPort) -> Option<(usize, usize)> {
    /// `x86_THREAD_STATE64`: rax rbx rcx rdx rdi rsi rbp rsp r8-r15 rip
    /// rflags cs fs gs.
    const FLAVOR: i32 = 4;
    let mut state = [0u64; 21];
    let mut count = 42u32;
    // SAFETY: the buffer holds `count` 32-bit words.
    let kr = unsafe { thread_get_state(port, FLAVOR, state.as_mut_ptr().cast(), &mut count) };
    (kr == 0).then(|| (state[16] as usize, state[6] as usize))
}
