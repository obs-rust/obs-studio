//! Windows side of the [platform boundary](super): the recursive lock the
//! audio thread port takes, and the MMCSS task the C worker registers.
//!
//! `audio-io.c` locks `input_mutex` with `pthread_mutex_init_recursive` and,
//! on Windows, calls `AvSetMmThreadCharacteristicsW(L"Audio")` for the
//! worker's lifetime. A Win32 critical section is recursive. The section
//! stays at the address `init` ran at: moving an initialized section is
//! undefined, same as a pthread mutex.

use core::ffi::c_int;
use core::ffi::c_void;
use std::cell::UnsafeCell;

#[repr(C)]
struct CriticalSection {
    debug_info: *mut c_void,
    lock_count: i32,
    recursion_count: i32,
    owning_thread: *mut c_void,
    lock_semaphore: *mut c_void,
    spin_count: usize,
}

#[link(name = "kernel32")]
#[allow(non_snake_case)]
unsafe extern "system" {
    fn InitializeCriticalSectionAndSpinCount(section: *mut CriticalSection, spin: u32) -> i32;
    fn DeleteCriticalSection(section: *mut CriticalSection);
    fn EnterCriticalSection(section: *mut CriticalSection);
    fn LeaveCriticalSection(section: *mut CriticalSection);
}

#[link(name = "avrt")]
#[allow(non_snake_case)]
unsafe extern "system" {
    fn AvSetMmThreadCharacteristicsW(task_name: *const u16, task_index: *mut u32) -> *mut c_void;
    fn AvRevertMmThreadCharacteristics(handle: *mut c_void) -> i32;
}

pub(crate) struct RecursiveMutex {
    inner: UnsafeCell<CriticalSection>,
    initialized: bool,
}

// SAFETY: `EnterCriticalSection` / `LeaveCriticalSection` are thread-safe on
// one section. `Drop` (`DeleteCriticalSection`) stays off those calls, same
// as the C mutex destroy after join.
unsafe impl Send for RecursiveMutex {}
// SAFETY: see `Send`.
unsafe impl Sync for RecursiveMutex {}

impl RecursiveMutex {
    /// Zeroed storage. `init` must run once at the object's final address
    /// before any `lock` / `unlock`.
    pub(crate) fn uninit() -> Self {
        Self {
            // SAFETY: a zeroed `CRITICAL_SECTION` is the pre-init state
            // `InitializeCriticalSectionAndSpinCount` accepts.
            inner: UnsafeCell::new(unsafe { core::mem::zeroed() }),
            initialized: false,
        }
    }

    /// `InitializeCriticalSectionAndSpinCount` at the object's final address.
    ///
    /// # Safety
    /// Call exactly once, before the mutex is shared.
    pub(crate) unsafe fn init(&mut self) -> Result<(), c_int> {
        // SAFETY: `inner` is zeroed storage exclusive to this call. A
        // nonzero return means the section is initialized at this address.
        let ok = unsafe { InitializeCriticalSectionAndSpinCount(self.inner.get(), 0) };
        if ok == 0 {
            return Err(-1);
        }
        self.initialized = true;
        Ok(())
    }

    #[inline]
    pub(crate) fn lock(&self) {
        // SAFETY: `init` ran at this address before the mutex was shared.
        unsafe { EnterCriticalSection(self.inner.get()) };
    }

    #[inline]
    pub(crate) fn unlock(&self) {
        // SAFETY: see `lock`. This thread holds the section.
        unsafe { LeaveCriticalSection(self.inner.get()) };
    }
}

impl Drop for RecursiveMutex {
    fn drop(&mut self) {
        if self.initialized {
            // SAFETY: initialized in `open`. Drop runs only after the audio
            // thread has joined, so nothing still holds the section.
            unsafe { DeleteCriticalSection(self.inner.get()) };
        }
    }
}

/// `AvSetMmThreadCharacteristicsW(L"Audio")` for the worker's lifetime.
/// A null handle means the call failed; the C worker continues either way.
pub(crate) struct AudioThreadClass {
    handle: *mut c_void,
}

impl AudioThreadClass {
    pub(crate) fn enter(_samples_per_sec: u32) -> Self {
        let name: Vec<u16> = "Audio".encode_utf16().chain(core::iter::once(0)).collect();
        let mut index = 0u32;
        // SAFETY: `name` is NUL-terminated UTF-16 and `index` is a live
        // `DWORD`. A null return is the documented failure and is not a handle.
        let handle = unsafe { AvSetMmThreadCharacteristicsW(name.as_ptr(), &mut index) };
        Self { handle }
    }

    /// No log line: the C worker logs nothing for MMCSS either.
    pub(crate) fn notice(&mut self) -> Option<(i32, String)> {
        None
    }
}

impl Drop for AudioThreadClass {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            // SAFETY: `handle` is the value `AvSetMmThreadCharacteristicsW`
            // returned on this thread.
            unsafe { AvRevertMmThreadCharacteristics(self.handle) };
        }
    }
}
