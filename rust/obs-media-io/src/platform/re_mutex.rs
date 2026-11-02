//! Recursive mutex whose guard does not hold `&mut` across a callback.
//!
//! `lock` keeps other threads out for as long as the guard lives. `with_mut`
//! hands out `&mut T` only for the closure, and that borrow ends before
//! `with_mut` returns. A callback can `lock` again on this thread and get a
//! new `&mut` without aliasing the previous one.

use std::cell::UnsafeCell;
use std::marker::PhantomData;
use std::sync::{Condvar, Mutex};
use std::thread::{self, ThreadId};

use obs_util::ffi::tsan;

struct Inner {
    owner: Option<ThreadId>,
    depth: usize,
}

pub(crate) struct ReMutex<T> {
    inner: Mutex<Inner>,
    cv: Condvar,
    value: UnsafeCell<T>,
}

/// Holds the lock. `!Send`, so only the thread that locked it can use it.
pub(crate) struct Guard<'a, T> {
    mutex: &'a ReMutex<T>,
    _not_send: PhantomData<*const ()>,
}

impl<T> ReMutex<T> {
    pub(crate) fn new(value: T) -> Self {
        Self {
            inner: Mutex::new(Inner {
                owner: None,
                depth: 0,
            }),
            cv: Condvar::new(),
            value: UnsafeCell::new(value),
        }
    }

    pub(crate) fn lock(&self) -> Guard<'_, T> {
        let id = thread::current().id();
        let mut guard = self.inner.lock().unwrap_or_else(|err| err.into_inner());
        while guard.depth > 0 && guard.owner != Some(id) {
            guard = self.cv.wait(guard).unwrap_or_else(|err| err.into_inner());
        }
        guard.owner = Some(id);
        guard.depth += 1;
        // TSan does not see this `Mutex` and `Condvar`, so tell it what a
        // recursive `pthread_mutex_t` would, as the C file uses (#3507).
        tsan::acquire(&self.inner);
        Guard {
            mutex: self,
            _not_send: PhantomData,
        }
    }

    fn unlock(&self) {
        let mut guard = self.inner.lock().unwrap_or_else(|err| err.into_inner());
        guard.depth -= 1;
        if guard.depth == 0 {
            tsan::release(&self.inner);
            guard.owner = None;
            self.cv.notify_one();
        }
    }
}

impl<T> Guard<'_, T> {
    /// Runs `f` with the protected value. The `&mut` ends before this
    /// returns, so `f` must not stash it. Calling `lock` again from `f`
    /// and then `with_mut` on the inner guard would alias; callers deliver
    /// callbacks only after `with_mut` has returned.
    pub(crate) fn with_mut<R>(&mut self, f: impl FnOnce(&mut T) -> R) -> R {
        // SAFETY: this thread owns the lock (`depth >= 1`). The reference
        // does not escape `f`. The guard is `!Send`.
        f(unsafe { &mut *self.mutex.value.get() })
    }
}

impl<T> Drop for Guard<'_, T> {
    fn drop(&mut self) {
        self.mutex.unlock();
    }
}

// SAFETY: other threads block in `lock` while `depth > 0`. `&mut T` is only
// created inside `with_mut` and does not escape it.
unsafe impl<T: Send> Send for ReMutex<T> {}
unsafe impl<T: Send> Sync for ReMutex<T> {}
