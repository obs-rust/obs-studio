//! C ABI shims for `libobs/util/bmem.h`.
//!
//! Reproduces `bmem.c` control flow exactly, including the counter rules:
//! `bmalloc` counts after a successful allocation, `brealloc(NULL, n)` counts
//! as an allocation (before the size check), and `bfree(NULL)` does not
//! decrement.

use core::ffi::{c_char, c_int, c_long, c_ulong, c_void};

use crate::bmem::{ALIGNMENT, AllocCounter};
#[cfg(not(windows))]
use crate::bmem::{aligned_addr, aligned_offset, raw_addr};

unsafe extern "C" {
    #[cfg(not(windows))]
    fn malloc(size: usize) -> *mut c_void;
    #[cfg(not(windows))]
    fn realloc(ptr: *mut c_void, size: usize) -> *mut c_void;
    #[cfg(not(windows))]
    fn free(ptr: *mut c_void);

    #[cfg(windows)]
    fn _aligned_malloc(size: usize, alignment: usize) -> *mut c_void;
    #[cfg(windows)]
    fn _aligned_realloc(ptr: *mut c_void, size: usize, alignment: usize) -> *mut c_void;
    #[cfg(windows)]
    fn _aligned_free(ptr: *mut c_void);

    fn os_breakpoint();
    fn os_oom();
    fn bcrash(format: *const c_char, ...) -> !;
}

static NUM_ALLOCS: AllocCounter = AllocCounter::new();

#[cfg(windows)]
unsafe fn a_malloc(size: usize) -> *mut c_void {
    // SAFETY: plain allocation.
    unsafe { _aligned_malloc(size, ALIGNMENT) }
}

#[cfg(not(windows))]
unsafe fn a_malloc(size: usize) -> *mut c_void {
    // SAFETY: plain allocation.
    let raw = unsafe { malloc(size.wrapping_add(ALIGNMENT)) };
    if raw.is_null() {
        return raw;
    }
    let diff = aligned_offset(raw as usize);
    // SAFETY: `diff <= ALIGNMENT`, and `raw` has `size + ALIGNMENT` bytes.
    let ptr = unsafe { raw.cast::<u8>().add(diff) };
    // SAFETY: the byte before `ptr` is inside the allocation (`diff >= 1`).
    unsafe { *ptr.sub(1) = diff as u8 };
    debug_assert_eq!(ptr as usize, aligned_addr(raw as usize));
    ptr.cast()
}

#[cfg(windows)]
unsafe fn a_realloc(ptr: *mut c_void, size: usize) -> *mut c_void {
    // SAFETY: `ptr` is null or came from `_aligned_malloc`/`_aligned_realloc`.
    unsafe { _aligned_realloc(ptr, size, ALIGNMENT) }
}

#[cfg(not(windows))]
unsafe fn a_realloc(ptr: *mut c_void, size: usize) -> *mut c_void {
    if ptr.is_null() {
        // SAFETY: plain allocation.
        return unsafe { a_malloc(size) };
    }
    // SAFETY: `ptr` came from `a_malloc`, which stored the offset before it.
    let diff = unsafe { *ptr.cast::<u8>().sub(1) };
    let raw = raw_addr(ptr as usize, diff) as *mut c_void;
    // SAFETY: `raw` is the original `malloc` result.
    let new = unsafe { realloc(raw, size.wrapping_add(diff as usize)) };
    if new.is_null() {
        return new;
    }
    // SAFETY: the new block holds at least `diff` bytes.
    unsafe { new.cast::<u8>().add(diff as usize).cast() }
}

#[cfg(windows)]
unsafe fn a_free(ptr: *mut c_void) {
    // SAFETY: `ptr` came from the aligned allocator.
    unsafe { _aligned_free(ptr) }
}

#[cfg(not(windows))]
unsafe fn a_free(ptr: *mut c_void) {
    if !ptr.is_null() {
        // SAFETY: `ptr` came from `a_malloc`, which stored the offset before it.
        let diff = unsafe { *ptr.cast::<u8>().sub(1) };
        // SAFETY: `raw` is the original `malloc` result.
        unsafe { free(raw_addr(ptr as usize, diff) as *mut c_void) };
    }
}

/// # Safety
///
/// Same contract as C `bmalloc`: the result must be released with `bfree`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bmalloc(size: usize) -> *mut c_void {
    if size == 0 {
        // SAFETY: libobs hooks.
        unsafe {
            os_breakpoint();
            bcrash(
                c"bmalloc: Allocating 0 bytes is broken behavior, please fix your code!".as_ptr(),
            );
        }
    }

    // SAFETY: plain allocation.
    let ptr = unsafe { a_malloc(size) };

    if ptr.is_null() {
        // SAFETY: libobs hooks.
        unsafe {
            os_oom();
            bcrash(
                c"Out of memory while trying to allocate %lu bytes".as_ptr(),
                size as c_ulong,
            );
        }
    }

    NUM_ALLOCS.inc();
    ptr
}

/// # Safety
///
/// `ptr` must be null or a live block from `bmalloc`/`brealloc`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn brealloc(ptr: *mut c_void, size: usize) -> *mut c_void {
    if ptr.is_null() {
        NUM_ALLOCS.inc();
    }

    if size == 0 {
        // SAFETY: libobs hooks.
        unsafe {
            os_breakpoint();
            bcrash(
                c"brealloc: Allocating 0 bytes is broken behavior, please fix your code!".as_ptr(),
            );
        }
    }

    // SAFETY: the caller guarantees `ptr`.
    let ptr = unsafe { a_realloc(ptr, size) };

    if ptr.is_null() {
        // SAFETY: libobs hooks.
        unsafe {
            os_oom();
            bcrash(
                c"Out of memory while trying to allocate %lu bytes".as_ptr(),
                size as c_ulong,
            );
        }
    }

    ptr
}

/// # Safety
///
/// `ptr` must be null or a live block from `bmalloc`/`brealloc`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bfree(ptr: *mut c_void) {
    if !ptr.is_null() {
        NUM_ALLOCS.dec();
        // SAFETY: the caller guarantees `ptr`.
        unsafe { a_free(ptr) };
    }
}

/// # Safety
///
/// Always safe to call; `unsafe` only to match the other shims.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bnum_allocs() -> c_long {
    NUM_ALLOCS.get() as c_long
}

/// # Safety
///
/// Always safe to call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn base_get_alignment() -> c_int {
    ALIGNMENT as c_int
}

/// # Safety
///
/// `ptr` must point to `size` readable bytes when `size` is non-zero.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bmemdup(ptr: *const c_void, size: usize) -> *mut c_void {
    // SAFETY: allocation; crashes on size 0 exactly like C.
    let out = unsafe { bmalloc(size) };
    if size != 0 {
        // SAFETY: `out` has `size` bytes; the caller guarantees `ptr`.
        unsafe { core::ptr::copy_nonoverlapping(ptr.cast::<u8>(), out.cast::<u8>(), size) };
    }
    out
}
