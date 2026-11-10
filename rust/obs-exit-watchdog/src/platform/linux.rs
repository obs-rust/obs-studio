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
pub const STACK_SUPPORTED: bool = true;

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

/// The thread to capture.
pub struct Thread {
    id: libc::pthread_t,
}

pub fn current_thread() -> Thread {
    // SAFETY: the calling thread's own handle.
    Thread {
        id: unsafe { libc::pthread_self() },
    }
}

/// The deepest stack kept.
const MAX_FRAMES: usize = 64;
/// The handler's own frame and the signal trampoline come first.
const SKIP: usize = 2;
/// How long the capture waits for the thread to run the handler.
const WAIT: Duration = Duration::from_millis(500);

static FRAMES: [AtomicUsize; MAX_FRAMES] = [const { AtomicUsize::new(0) }; MAX_FRAMES];
static COUNT: AtomicUsize = AtomicUsize::new(0);
static DONE: AtomicBool = AtomicBool::new(false);

unsafe extern "C" {
    fn backtrace(buffer: *mut *mut c_void, size: c_int) -> c_int;
}

/// Loads the unwinder `backtrace(3)` uses, outside a signal: its first call
/// may allocate.
pub fn prepare_stack() {
    let mut frames = [std::ptr::null_mut(); 4];
    // SAFETY: the buffer holds 4 entries.
    unsafe { backtrace(frames.as_mut_ptr(), 4) };
}

/// Runs on the captured thread: its own return addresses into `FRAMES`.
extern "C" fn on_signal(_signal: c_int) {
    let mut frames = [std::ptr::null_mut::<c_void>(); MAX_FRAMES];
    // SAFETY: the buffer holds MAX_FRAMES entries; the unwinder was loaded
    // by `prepare_stack`.
    let n = unsafe { backtrace(frames.as_mut_ptr(), MAX_FRAMES as c_int) }.max(0) as usize;
    for (slot, frame) in FRAMES.iter().zip(&frames[..n]) {
        slot.store(*frame as usize, Ordering::Relaxed);
    }
    COUNT.store(n, Ordering::Relaxed);
    DONE.store(true, Ordering::Release);
}

/// The thread's return addresses, innermost first: a real-time signal makes
/// the thread run `backtrace(3)` in a handler (installed now, only for the
/// capture), and this thread waits for it.
pub fn capture_stack(thread: &Thread, out: &mut [usize]) -> usize {
    let signal = libc::SIGRTMIN() + 3;
    DONE.store(false, Ordering::Release);
    // SAFETY: a plain handler for a real-time signal OBS does not use; the
    // target is a live thread of this process.
    unsafe {
        let mut action: libc::sigaction = std::mem::zeroed();
        action.sa_sigaction = on_signal as extern "C" fn(c_int) as usize;
        action.sa_flags = libc::SA_RESTART;
        libc::sigemptyset(&mut action.sa_mask);
        if libc::sigaction(signal, &action, std::ptr::null_mut()) != 0 {
            return 0;
        }
        if libc::pthread_kill(thread.id, signal) != 0 {
            return 0;
        }
    }
    let start = Instant::now();
    while !DONE.load(Ordering::Acquire) {
        if start.elapsed() > WAIT {
            return 0;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    let n = COUNT.load(Ordering::Relaxed);
    let frames = FRAMES.iter().take(n).skip(SKIP);
    let mut taken = 0;
    for (slot, frame) in out.iter_mut().zip(frames) {
        *slot = frame.load(Ordering::Relaxed);
        taken += 1;
    }
    taken
}
