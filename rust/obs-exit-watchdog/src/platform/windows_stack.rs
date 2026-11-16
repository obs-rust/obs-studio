//! Windows x86_64: another thread's return addresses (#4589).
//!
//! The armed thread is suspended, its registers read with
//! `GetThreadContext`, and its stack unwound frame by frame with the
//! image's unwind data (`RtlLookupFunctionEntry`, `RtlVirtualUnwind`), as
//! x64 Windows code has no frame pointer chain. A frame without unwind data
//! is a leaf: its return address is at the stack pointer. The walk uses
//! only the fixed context and the output buffer: no allocation, no dbghelp.
//!
//! `RtlLookupFunctionEntry` takes a loader lock for dynamically registered
//! function tables (JIT code). A thread suspended while it holds that lock
//! would block the capture; OBS's main thread in `exit()` does not register
//! such tables.

use std::ffi::c_void;

pub const STACK_SUPPORTED: bool = true;

const THREAD_SUSPEND_RESUME: u32 = 0x0002;
const THREAD_GET_CONTEXT: u32 = 0x0008;
const THREAD_QUERY_INFORMATION: u32 = 0x0040;

const CONTEXT_AMD64: u32 = 0x0010_0000;
/// `CONTEXT_FULL`: control, integer and floating-point registers, which
/// `RtlVirtualUnwind` restores.
const CONTEXT_FULL: u32 = CONTEXT_AMD64 | 0x1 | 0x2 | 0x8;
const UNW_FLAG_NHANDLER: u32 = 0;

/// The x64 `CONTEXT` (1232 bytes, 16-byte aligned): only the fields the
/// walk reads are named.
#[repr(C, align(16))]
struct Context {
    home: [u64; 6],
    flags: u32,
    mx_csr: u32,
    segments: [u16; 6],
    eflags: u32,
    debug: [u64; 6],
    /// Rax, Rcx, Rdx, Rbx, Rsp, Rbp, Rsi, Rdi, R8 to R15.
    integer: [u64; 16],
    rip: u64,
    rest: [u8; 1232 - 256],
}

const RSP: usize = 4;

const _: () = assert!(size_of::<Context>() == 1232);
const _: () = assert!(std::mem::offset_of!(Context, rip) == 248);

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetCurrentThreadId() -> u32;
    fn OpenThread(access: u32, inherit: i32, thread_id: u32) -> *mut c_void;
    fn CloseHandle(handle: *mut c_void) -> i32;
    fn SuspendThread(thread: *mut c_void) -> u32;
    fn ResumeThread(thread: *mut c_void) -> u32;
    fn GetThreadContext(thread: *mut c_void, context: *mut Context) -> i32;
    fn RtlLookupFunctionEntry(pc: u64, image_base: *mut u64, history: *mut c_void) -> *mut c_void;
    fn RtlVirtualUnwind(
        handler_type: u32,
        image_base: u64,
        pc: u64,
        function: *mut c_void,
        context: *mut Context,
        handler_data: *mut *mut c_void,
        establisher_frame: *mut u64,
        context_pointers: *mut c_void,
    ) -> *mut c_void;
}

/// A thread handle that another thread can suspend and read (null if it
/// could not be opened: then nothing is captured).
pub struct Thread(usize);

impl Drop for Thread {
    fn drop(&mut self) {
        if self.0 != 0 {
            // SAFETY: a handle this struct opened and owns.
            unsafe { CloseHandle(std::ptr::with_exposed_provenance_mut(self.0)) };
        }
    }
}

pub fn current_thread() -> Thread {
    // SAFETY: opens the calling thread by its id; a failure gives null.
    let handle = unsafe {
        OpenThread(
            THREAD_SUSPEND_RESUME | THREAD_GET_CONTEXT | THREAD_QUERY_INFORMATION,
            0,
            GetCurrentThreadId(),
        )
    };
    Thread(handle.expose_provenance())
}

pub fn prepare_stack() {}

pub fn capture_stack(thread: &Thread, out: &mut [usize]) -> usize {
    let handle: *mut c_void = std::ptr::with_exposed_provenance_mut(thread.0);
    if handle.is_null() || out.is_empty() {
        return 0;
    }
    // SAFETY: a valid thread handle with suspend/resume rights.
    if unsafe { SuspendThread(handle) } == u32::MAX {
        return 0;
    }
    // SAFETY: an all-zero CONTEXT is valid input once `flags` is set.
    let mut context: Context = unsafe { std::mem::zeroed() };
    context.flags = CONTEXT_FULL;
    // GetThreadContext also waits until the suspension took effect.
    // SAFETY: the thread is suspended; the context is aligned and large
    // enough.
    let n = if unsafe { GetThreadContext(handle, &raw mut context) } != 0 {
        // SAFETY: the thread stays suspended for the whole walk.
        unsafe { walk(&mut context, out) }
    } else {
        0
    };
    // SAFETY: resumes the suspension above.
    unsafe { ResumeThread(handle) };
    n
}

/// Unwinds `context` frame by frame into `out`; returns the frame count.
///
/// # Safety
/// The thread `context` was read from must be suspended, so its stack
/// does not change.
unsafe fn walk(context: &mut Context, out: &mut [usize]) -> usize {
    let mut n = 0;
    while n < out.len() {
        let pc = context.rip;
        if pc == 0 {
            break;
        }
        out[n] = usize::try_from(pc).unwrap_or(0);
        n += 1;
        let sp = context.integer[RSP];
        let mut image_base = 0u64;
        let history = std::ptr::null_mut();
        // SAFETY: plain lookup in the loaded images' unwind tables.
        let function = unsafe { RtlLookupFunctionEntry(pc, &raw mut image_base, history) };
        if function.is_null() {
            // A leaf function: the return address is at the stack pointer.
            // SAFETY: the suspended thread's own stack, at its pointer.
            context.rip = unsafe { *(std::ptr::with_exposed_provenance::<u64>(sp as usize)) };
            context.integer[RSP] = sp + 8;
        } else {
            let mut handler_data = std::ptr::null_mut();
            let mut frame = 0u64;
            // SAFETY: the function entry belongs to `image_base`; the
            // context is the thread's current frame.
            unsafe {
                RtlVirtualUnwind(
                    UNW_FLAG_NHANDLER,
                    image_base,
                    pc,
                    function,
                    context,
                    &raw mut handler_data,
                    &raw mut frame,
                    std::ptr::null_mut(),
                );
            }
        }
        // The stack only grows towards lower addresses: an unwind that does
        // not move up has run out of real frames.
        if context.integer[RSP] <= sp {
            break;
        }
    }
    n
}
