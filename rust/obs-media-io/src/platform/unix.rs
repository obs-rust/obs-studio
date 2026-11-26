//! Unix side of the [platform boundary](super): the pthread recursive
//! mutex the audio thread port locks.

use core::ffi::c_int;
use std::cell::UnsafeCell;

/// `pthread_mutex_init_recursive`: `audio_output_connect`/`disconnect` lock
/// `input_mutex` and the C comments in `do_audio_output` let an output
/// callback disconnect or connect again from inside the audio thread, which
/// only works because the mutex is recursive.
///
/// The `pthread_mutex_t` lives in the `Shared` box: `init()` runs only after
/// boxing, at the mutex's final address — POSIX forbids moving an
/// initialized mutex, so the previous stack-`new()`-then-move was unsound.
pub(crate) struct RecursiveMutex {
    inner: UnsafeCell<libc::pthread_mutex_t>,
    initialized: bool,
}

// Same-thread re-lock is the point; cross-thread sharing is serialized by
// the mutex itself.
unsafe impl Send for RecursiveMutex {}
unsafe impl Sync for RecursiveMutex {}

impl RecursiveMutex {
    /// Zeroed storage; `init()` must run once at the object's final address
    /// before any `lock`/`unlock`.
    pub(crate) fn uninit() -> Self {
        // SAFETY: pthread_mutex_t is plain POD; zeroed bytes are a valid
        // pre-init representation.
        Self {
            inner: UnsafeCell::new(unsafe { std::mem::zeroed() }),
            initialized: false,
        }
    }

    /// `pthread_mutex_init_recursive` at the object's final address.
    /// # Safety
    /// Call exactly once, before the mutex is shared.
    pub(crate) unsafe fn init(&mut self) -> Result<(), c_int> {
        unsafe {
            let mut attr: libc::pthread_mutexattr_t = std::mem::zeroed();
            if libc::pthread_mutexattr_init(&mut attr) != 0 {
                return Err(-1);
            }
            libc::pthread_mutexattr_settype(&mut attr, libc::PTHREAD_MUTEX_RECURSIVE);
            let rc = libc::pthread_mutex_init(self.inner.get(), &attr);
            libc::pthread_mutexattr_destroy(&mut attr);
            if rc != 0 {
                return Err(-1);
            }
            self.initialized = true;
            Ok(())
        }
    }

    #[inline]
    pub(crate) fn lock(&self) {
        // SAFETY: initialized in open before the mutex is shared.
        unsafe { libc::pthread_mutex_lock(self.inner.get()) };
    }

    #[inline]
    pub(crate) fn unlock(&self) {
        // SAFETY: see lock.
        unsafe { libc::pthread_mutex_unlock(self.inner.get()) };
    }
}

impl Drop for RecursiveMutex {
    fn drop(&mut self) {
        if self.initialized {
            // SAFETY: initialized in open; only destroyed after the audio
            // thread joined (nothing can hold the lock).
            unsafe { libc::pthread_mutex_destroy(self.inner.get()) };
        }
    }
}

/// The BSDs: no audio thread class yet.
/// Unix has no MMCSS registration. The Windows worker calls
/// `AvSetMmThreadCharacteristicsW`; this keeps the call site host-neutral.
#[cfg(not(any(target_os = "linux", target_os = "macos")))] // `super::linux`, `super::macos`
pub(crate) struct AudioThreadClass;

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
impl AudioThreadClass {
    pub(crate) fn enter(_samples_per_sec: u32) -> Self {
        Self
    }

    /// No log line on this host.
    pub(crate) fn notice(&mut self) -> Option<(i32, String)> {
        None
    }
}
