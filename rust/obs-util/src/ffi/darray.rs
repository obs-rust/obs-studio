//! Raw mirror of `struct darray` and the `darray.h` inline functions.
//!
//! `darray.h` is header-inline, so nothing here is exported: these are plain
//! Rust functions that reproduce the header byte for byte (including the
//! `bmalloc`/`bfree` allocations) for the shims that use C darrays in place:
//! the array serializer, and the output interleaver (#1594) with insert,
//! erase range, pop front and push back new. C and Rust can hand one array
//! back and forth, since both allocate through bmem. The allocator is the
//! Rust port in [`crate::ffi::bmem`], re-exported here.

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
    // SAFETY: `dst` is valid; the buffer then holds `cur + num` items, and
    // `array` is valid for `num` items and does not overlap. The header
    // resizes here, which zeroes the new items before the copy overwrites
    // every one of them.
    unsafe {
        darray_ensure_capacity(element_size, dst, cur + num);
        (*dst).num = cur + num;
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

/// `darray_item`: the address of item `idx`.
///
/// # Safety
///
/// `d.array` must hold at least `idx` items (one past the end is allowed).
unsafe fn item(element_size: usize, d: &darray, idx: usize) -> *mut u8 {
    // SAFETY: the caller keeps the offset within (or one past) the buffer.
    unsafe { (d.array as *mut u8).add(element_size * idx) }
}

/// # Safety
///
/// `dst` must point to a valid `darray` of `element_size`-byte items, and
/// `item` must be valid for reads of one item.
pub unsafe fn darray_push_back(
    element_size: usize,
    dst: *mut darray,
    item: *const c_void,
) -> usize {
    // SAFETY: the caller guarantees `dst` is valid. As in C, `num` grows
    // before the capacity check; darray_ensure_capacity does not read it.
    unsafe {
        (*dst).num += 1;
        darray_ensure_capacity(element_size, dst, (*dst).num);
        let d = &*dst;
        // SAFETY: the buffer now holds `num` items; `item` is readable. A
        // move, not a copy, so an `item` inside the buffer is no UB here.
        ptr::copy(
            item as *const u8,
            self::item(element_size, d, d.num - 1),
            element_size,
        );
        d.num - 1
    }
}

/// `darray_push_back_new`, and the GCC/Clang `da_push_back_new` macro, which
/// does the same steps: one more item, zeroed, after any growth. Returns it.
///
/// # Safety
///
/// `dst` must point to a valid `darray` of `element_size`-byte items.
pub unsafe fn darray_push_back_new(element_size: usize, dst: *mut darray) -> *mut c_void {
    // SAFETY: the caller guarantees `dst` is valid; the buffer then holds
    // `num` items, so the last one is in bounds.
    unsafe {
        (*dst).num += 1;
        darray_ensure_capacity(element_size, dst, (*dst).num);
        let last = item(element_size, &*dst, (*dst).num - 1);
        ptr::write_bytes(last, 0, element_size);
        last.cast()
    }
}

/// # Safety
///
/// `dst` must point to a valid `darray` of `element_size`-byte items, and
/// `item` must be valid for reads of one item.
///
/// # Panics
///
/// When `idx > num`, where C asserts.
pub unsafe fn darray_insert(
    element_size: usize,
    dst: *mut darray,
    idx: usize,
    item: *const c_void,
) {
    // SAFETY: the caller guarantees `dst` is valid.
    let num = unsafe { (*dst).num };
    assert!(idx <= num, "darray_insert: idx > num");
    if idx == num {
        // SAFETY: as above, and `item` is readable.
        unsafe { darray_push_back(element_size, dst, item) };
        return;
    }
    let move_count = num - idx;
    // SAFETY: after the growth the buffer holds `num + 1` items, so items
    // `idx..num` move up by one within it; then `item` fills `idx`.
    unsafe {
        (*dst).num = num + 1;
        darray_ensure_capacity(element_size, dst, num + 1);
        let d = &*dst;
        let new_item = self::item(element_size, d, idx);
        ptr::copy(
            new_item,
            self::item(element_size, d, idx + 1),
            move_count * element_size,
        );
        ptr::copy(item as *const u8, new_item, element_size);
    }
}

/// # Safety
///
/// `dst` must point to a valid `darray` of `element_size`-byte items.
///
/// # Panics
///
/// Unless `start < end <= num`, where C asserts.
pub unsafe fn darray_erase_range(element_size: usize, dst: *mut darray, start: usize, end: usize) {
    // SAFETY: the caller guarantees `dst` is valid.
    let num = unsafe { (*dst).num };
    assert!(start <= num, "darray_erase_range: start > num");
    assert!(end <= num, "darray_erase_range: end > num");
    assert!(end > start, "darray_erase_range: end <= start");

    let count = end - start;
    if count == 1 {
        // SAFETY: `start < num`.
        unsafe { darray_erase(element_size, dst, start) };
        return;
    } else if count == num {
        // SAFETY: as above.
        unsafe { (*dst).num = 0 };
        return;
    }

    let move_count = num - end;
    // SAFETY: items `end..num` are in the buffer and move down to `start`.
    // The slots past the new `num` keep their old bytes, as in C.
    unsafe {
        if move_count != 0 {
            let d = &*dst;
            ptr::copy(
                item(element_size, d, end),
                item(element_size, d, start),
                move_count * element_size,
            );
        }
        (*dst).num -= count;
    }
}

/// `darray_pop_front`. C asserts `num != 0` and then checks it, so an empty
/// array is a no-op, as `darray_pop_back` is.
///
/// # Safety
///
/// `dst` must point to a valid `darray` of `element_size`-byte items.
pub unsafe fn darray_pop_front(element_size: usize, dst: *mut darray) {
    // SAFETY: the caller guarantees `dst` is valid; `0 < num`.
    unsafe {
        if (*dst).num != 0 {
            darray_erase(element_size, dst, 0);
        }
    }
}
