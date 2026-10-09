//! Safe core of `libobs/util/bmem.c`: the alignment arithmetic and the
//! allocation counter.
//!
//! The C ABI shims (`bmalloc`, `brealloc`, `bfree`, ...) live in
//! [`crate::ffi::bmem`] and use these pure functions.

use core::sync::atomic::Ordering;

/// `ALIGNMENT` in `bmem.c`: every block returned by `bmalloc` is aligned to
/// this many bytes.
pub const ALIGNMENT: usize = 32;

/// C `long` is 32-bit on Windows and 64-bit on the other supported targets.
#[cfg(windows)]
pub type CLong = i32;
/// C `long` is 32-bit on Windows and 64-bit on the other supported targets.
#[cfg(not(windows))]
pub type CLong = i64;

/// Atomic with the width of C `long` (`os_atomic_*_long`).
#[cfg(windows)]
pub type CLongAtomic = core::sync::atomic::AtomicI32;
/// Atomic with the width of C `long` (`os_atomic_*_long`).
#[cfg(not(windows))]
pub type CLongAtomic = core::sync::atomic::AtomicI64;

/// Offset added to a raw `malloc` address by the `ALIGNMENT_HACK`:
/// `((~addr) & (ALIGNMENT - 1)) + 1`. Always in `1..=ALIGNMENT`, so it fits
/// in the single byte stored just before the returned pointer. An address that
/// is already aligned gets the full `ALIGNMENT`.
pub const fn aligned_offset(addr: usize) -> usize {
    ((!addr) & (ALIGNMENT - 1)) + 1
}

/// The address handed back to the caller for a raw allocation at `addr`.
pub const fn aligned_addr(addr: usize) -> usize {
    addr.wrapping_add(aligned_offset(addr))
}

/// The raw allocation address recovered from the user address `addr` and the
/// offset byte stored before it.
pub const fn raw_addr(addr: usize, stored_offset: u8) -> usize {
    addr.wrapping_sub(stored_offset as usize)
}

/// The `num_allocs` counter.
#[derive(Debug)]
pub struct AllocCounter(CLongAtomic);

impl AllocCounter {
    pub const fn new() -> Self {
        Self(CLongAtomic::new(0))
    }

    /// `os_atomic_inc_long`.
    pub fn inc(&self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }

    /// `os_atomic_dec_long`.
    pub fn dec(&self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }

    pub fn get(&self) -> CLong {
        self.0.load(Ordering::SeqCst)
    }
}

impl Default for AllocCounter {
    fn default() -> Self {
        Self::new()
    }
}
