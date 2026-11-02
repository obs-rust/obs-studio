//! Tells ThreadSanitizer about an order that Rust code creates (#3507).
//!
//! The tsan pass builds the C code with `-fsanitize=thread` but not the Rust
//! code. TSan does not see a Rust `Mutex` or `Condvar`, so C code that hands
//! data to another thread through one shows as a data race. A primitive calls
//! [`release`] where its order starts and [`acquire`] where it ends, on the
//! same address. Annotate only an order that the primitive guarantees: a wider
//! one hides real races.
//!
//! Without the `tsan` feature these calls compile to nothing.

/// The caller's earlier memory accesses come before any later [`acquire`] on
/// `addr`.
#[inline]
pub fn release<T>(addr: &T) {
    #[cfg(feature = "tsan")]
    // SAFETY: TSan only records the address as a synchronization object; it
    // does not read or write through it.
    unsafe {
        sys::__tsan_release(core::ptr::from_ref(addr).cast_mut().cast());
    }
    #[cfg(not(feature = "tsan"))]
    let _ = addr;
}

/// The caller's later memory accesses come after every earlier [`release`] on
/// `addr`.
#[inline]
pub fn acquire<T>(addr: &T) {
    #[cfg(feature = "tsan")]
    // SAFETY: as in [`release`].
    unsafe {
        sys::__tsan_acquire(core::ptr::from_ref(addr).cast_mut().cast());
    }
    #[cfg(not(feature = "tsan"))]
    let _ = addr;
}

#[cfg(feature = "tsan")]
mod sys {
    use core::ffi::c_void;

    unsafe extern "C" {
        pub fn __tsan_acquire(addr: *mut c_void);
        pub fn __tsan_release(addr: *mut c_void);
    }
}
