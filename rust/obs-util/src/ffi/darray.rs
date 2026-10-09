//! Raw mirror of `struct darray` and the `darray.h` inline functions.
//!
//! `darray.h` is header-inline, so nothing here is exported: these are plain
//! Rust functions that reproduce the header byte for byte (including the
//! `bmalloc`/`bfree` allocations) for the array-serializer shim. The
//! allocator is the Rust port in [`crate::ffi::bmem`], re-exported here.

use core::ffi::c_void;
use core::ptr;

use crate::darray::grow_capacity;

pub use crate::ffi::bmem::{bfree, bmalloc, brealloc};

/// Mirrors `struct darray` in `libobs/util/darray.h`.
#[repr(C)]
#[allow(non_camel_case_types)]
#[derive(Debug)]
pub struct darray {
    pub array: *mut c_void,
    pub num: usize,
    pub capacity: usize,
}

/// # Safety
///
/// `dst` must point to a valid `darray` whose `array` came from `bmalloc`
/// (or is null).
pub unsafe fn darray_free(dst: *mut darray) {
    // SAFETY: the caller guarantees `dst` is valid and `array` is freeable.
    unsafe {
        bfree((*dst).array);
        (*dst).array = ptr::null_mut();
        (*dst).num = 0;
        (*dst).capacity = 0;
    }
}

/// # Safety
///
/// `dst` must point to a valid `darray` of `element_size`-byte items.
pub unsafe fn darray_reserve(element_size: usize, dst: *mut darray, capacity: usize) {
    // SAFETY: the caller guarantees `dst` is valid.
    let d = unsafe { &mut *dst };
    if capacity == 0 || capacity <= d.capacity {
        return;
    }
    // SAFETY: plain allocation.
    let p = unsafe { bmalloc(element_size * capacity) };
    if !d.array.is_null() {
        if d.num != 0 {
            // SAFETY: the old buffer holds `num` items; the new one has room
            // for `capacity > old capacity >= num` items; they are distinct.
            unsafe {
                ptr::copy_nonoverlapping(d.array as *const u8, p as *mut u8, element_size * d.num)
            };
        }
        // SAFETY: the old buffer came from `bmalloc`.
        unsafe { bfree(d.array) };
    }
    d.array = p;
    d.capacity = capacity;
}

/// # Safety
///
/// `dst` must point to a valid `darray` of `element_size`-byte items.
pub unsafe fn darray_ensure_capacity(element_size: usize, dst: *mut darray, new_size: usize) {
    // SAFETY: the caller guarantees `dst` is valid.
    let d = unsafe { &mut *dst };
    if new_size <= d.capacity {
        return;
    }
    let new_cap = grow_capacity(d.capacity, new_size);
    // SAFETY: plain allocation.
    let p = unsafe { bmalloc(element_size * new_cap) };
    if !d.array.is_null() {
        if d.capacity != 0 {
            // SAFETY: the old buffer holds `capacity` items (as in C); the new
            // one holds `new_cap > capacity`; they are distinct.
            unsafe {
                ptr::copy_nonoverlapping(
                    d.array as *const u8,
                    p as *mut u8,
                    element_size * d.capacity,
                )
            };
        }
        // SAFETY: the old buffer came from `bmalloc`.
        unsafe { bfree(d.array) };
    }
    d.array = p;
    d.capacity = new_cap;
}

/// # Safety
///
/// `dst` must point to a valid `darray`.
pub unsafe fn darray_clear(dst: *mut darray) {
    // SAFETY: the caller guarantees `dst` is valid.
    unsafe { (*dst).num = 0 };
}

/// # Safety
///
/// `dst` must point to a valid `darray` of `element_size`-byte items.
pub unsafe fn darray_resize(element_size: usize, dst: *mut darray, size: usize) {
    // SAFETY: the caller guarantees `dst` is valid.
    let old_num = unsafe { (*dst).num };
    if size == old_num {
        return;
    } else if size == 0 {
        // SAFETY: as above.
        unsafe { (*dst).num = 0 };
        return;
    }
    // SAFETY: as above.
    unsafe {
        darray_ensure_capacity(element_size, dst, size);
        (*dst).num = size;
        if size > old_num {
            // SAFETY: capacity now covers `size` items.
            ptr::write_bytes(
                ((*dst).array as *mut u8).add(element_size * old_num),
                0,
                element_size * (size - old_num),
            );
        }
    }
}

/// # Safety
///
/// `dst` must be null or point to a valid `darray` of `element_size`-byte
/// items; `array` must be null or valid for reads of `num` items.
pub unsafe fn darray_push_back_array(
    element_size: usize,
    dst: *mut darray,
    array: *const c_void,
    num: usize,
) -> usize {
    if dst.is_null() {
        return 0;
    }
    // SAFETY: `dst` is non-null and valid per the contract.
    let cur = unsafe { (*dst).num };
    if array.is_null() || num == 0 {
        return cur;
    }
    // SAFETY: `dst` is valid; after the resize the buffer holds `cur + num`
    // items, and `array` is valid for `num` items and does not overlap.
    unsafe {
        darray_resize(element_size, dst, cur + num);
        ptr::copy_nonoverlapping(
            array as *const u8,
            ((*dst).array as *mut u8).add(element_size * cur),
            element_size * num,
        );
    }
    cur
}

/// # Safety
///
/// `dst` must point to a valid `darray` of `element_size`-byte items with
/// `idx < num` (the C original asserts this).
pub unsafe fn darray_erase(element_size: usize, dst: *mut darray, idx: usize) {
    // SAFETY: the caller guarantees `dst` is valid.
    let d = unsafe { &mut *dst };
    if idx >= d.num {
        return;
    }
    d.num -= 1;
    if d.num == 0 {
        return;
    }
    let base = d.array as *mut u8;
    // SAFETY: items `idx + 1..=num` are within the buffer (old num = num + 1).
    unsafe {
        ptr::copy(
            base.add(element_size * (idx + 1)),
            base.add(element_size * idx),
            element_size * (d.num - idx),
        )
    };
}

/// # Safety
///
/// `dst` must point to a valid `darray` of `element_size`-byte items.
pub unsafe fn darray_pop_back(element_size: usize, dst: *mut darray) {
    // SAFETY: the caller guarantees `dst` is valid.
    let num = unsafe { (*dst).num };
    if num != 0 {
        // SAFETY: `num - 1 < num`.
        unsafe { darray_erase(element_size, dst, num - 1) };
    }
}
