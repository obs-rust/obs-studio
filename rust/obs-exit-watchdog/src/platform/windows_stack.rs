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
const RBP: usize = 5;

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

/// In the helper (#4589): the return addresses of thread `thread_id` of
/// the process `process` (a handle with `PROCESS_VM_READ` and
/// `PROCESS_QUERY_INFORMATION`), innermost first; returns how many.
///
/// OBS is blocked in `ExitProcess`, so the walk runs from outside: the
/// thread is suspended, its registers read, and its stack walked with
/// dbghelp's `StackWalk64` over the process's memory, as
/// `libobs/obs-win-crash-handler.c.h` does. dbghelp is loaded at run time;
/// without it, or without the thread, no frames are given.
pub fn capture_remote(process: u64, thread_id: u64, out: &mut [usize]) -> usize {
    let process: *mut c_void =
        std::ptr::without_provenance_mut(usize::try_from(process).unwrap_or(0));
    let Ok(thread_id) = u32::try_from(thread_id) else {
        return 0;
    };
    let Some(dbghelp) = DbgHelp::load() else {
        return 0;
    };
    // SAFETY: opens a thread of the process by its id; null on failure.
    let thread = unsafe {
        OpenThread(
            THREAD_SUSPEND_RESUME | THREAD_GET_CONTEXT | THREAD_QUERY_INFORMATION,
            0,
            thread_id,
        )
    };
    if thread.is_null() {
        return 0;
    }
    let mut n = 0;
    // SAFETY: a valid thread handle with suspend/resume rights.
    if unsafe { SuspendThread(thread) } != u32::MAX {
        // SAFETY: an all-zero CONTEXT is valid input once `flags` is set.
        let mut context: Context = unsafe { std::mem::zeroed() };
        context.flags = CONTEXT_FULL;
        // SAFETY: the thread is suspended; the context is aligned and large
        // enough; dbghelp gets the process and thread it walks.
        unsafe {
            if GetThreadContext(thread, &raw mut context) != 0 {
                n = dbghelp.walk(process, thread, &mut context, out);
            }
            ResumeThread(thread);
        }
    }
    // SAFETY: the handle opened above, closed once.
    unsafe { CloseHandle(thread) };
    n
}

const IMAGE_FILE_MACHINE_AMD64: u32 = 0x8664;
const ADDR_MODE_FLAT: u32 = 3;

#[repr(C)]
#[derive(Default)]
struct Address64 {
    offset: u64,
    segment: u16,
    mode: u32,
}

/// `STACKFRAME64`, with room to spare for `KDHELP64` at its end.
#[repr(C)]
#[derive(Default)]
struct StackFrame64 {
    pc: Address64,
    ret: Address64,
    frame: Address64,
    stack: Address64,
    bstore: Address64,
    function_table_entry: usize,
    params: [u64; 4],
    far: i32,
    virtual_frame: i32,
    reserved: [u64; 3],
    kd_help: [u64; 32],
}

type SymInitializeW = unsafe extern "system" fn(*mut c_void, *const u16, i32) -> i32;
type SymCleanup = unsafe extern "system" fn(*mut c_void) -> i32;
type TableAccess = unsafe extern "system" fn(*mut c_void, u64) -> *mut c_void;
type ModuleBase = unsafe extern "system" fn(*mut c_void, u64) -> u64;
type StackWalk64 = unsafe extern "system" fn(
    u32,
    *mut c_void,
    *mut c_void,
    *mut StackFrame64,
    *mut c_void,
    *const c_void,
    Option<TableAccess>,
    Option<ModuleBase>,
    *const c_void,
) -> i32;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn LoadLibraryW(name: *const u16) -> *mut c_void;
    fn GetProcAddress(module: *mut c_void, name: *const u8) -> *mut c_void;
}

/// The dbghelp functions the walk needs.
struct DbgHelp {
    initialize: SymInitializeW,
    cleanup: SymCleanup,
    stack_walk: StackWalk64,
    table_access: TableAccess,
    module_base: ModuleBase,
}

impl DbgHelp {
    fn load() -> Option<Self> {
        let name: Vec<u16> = "dbghelp.dll".encode_utf16().chain([0]).collect();
        // SAFETY: NUL-terminated name; the library stays loaded for the
        // life of the helper.
        let module = unsafe { LoadLibraryW(name.as_ptr()) };
        if module.is_null() {
            return None;
        }
        let get = |symbol: &[u8]| {
            // SAFETY: `symbol` is NUL-terminated; a null result is checked.
            let address = unsafe { GetProcAddress(module, symbol.as_ptr()) };
            (!address.is_null()).then_some(address)
        };
        // SAFETY: each address is dbghelp's export of that name, whose
        // signature the type spells out.
        unsafe {
            Some(Self {
                initialize: std::mem::transmute::<*mut c_void, SymInitializeW>(get(
                    b"SymInitializeW\0",
                )?),
                cleanup: std::mem::transmute::<*mut c_void, SymCleanup>(get(b"SymCleanup\0")?),
                stack_walk: std::mem::transmute::<*mut c_void, StackWalk64>(get(b"StackWalk64\0")?),
                table_access: std::mem::transmute::<*mut c_void, TableAccess>(get(
                    b"SymFunctionTableAccess64\0",
                )?),
                module_base: std::mem::transmute::<*mut c_void, ModuleBase>(get(
                    b"SymGetModuleBase64\0",
                )?),
            })
        }
    }

    /// Walks the suspended `thread` of `process` from `context` into `out`.
    ///
    /// # Safety
    /// `thread` must be suspended, `process` must allow reading its memory
    /// and modules, and `context` must hold the thread's registers.
    unsafe fn walk(
        &self,
        process: *mut c_void,
        thread: *mut c_void,
        context: &mut Context,
        out: &mut [usize],
    ) -> usize {
        // SAFETY: the caller's handles; invade loads the process's modules.
        if unsafe { (self.initialize)(process, std::ptr::null(), 1) } == 0 {
            return 0;
        }
        let flat = |offset| Address64 {
            offset,
            segment: 0,
            mode: ADDR_MODE_FLAT,
        };
        let mut frame = StackFrame64 {
            pc: flat(context.rip),
            frame: flat(context.integer[RBP]),
            stack: flat(context.integer[RSP]),
            ..StackFrame64::default()
        };
        let mut n = 0;
        while n < out.len() {
            // SAFETY: valid frame and context; the default memory reader
            // reads `process`; dbghelp's own table and module callbacks.
            let more = unsafe {
                (self.stack_walk)(
                    IMAGE_FILE_MACHINE_AMD64,
                    process,
                    thread,
                    &raw mut frame,
                    std::ptr::from_mut(context).cast(),
                    std::ptr::null(),
                    Some(self.table_access),
                    Some(self.module_base),
                    std::ptr::null(),
                )
            };
            let pc = usize::try_from(frame.pc.offset).unwrap_or(0);
            if more == 0 || pc == 0 {
                break;
            }
            out[n] = pc;
            n += 1;
        }
        // SAFETY: pairs the initialize above.
        unsafe { (self.cleanup)(process) };
        n
    }
}
