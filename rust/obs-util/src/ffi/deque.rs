//! Raw mirror of `struct deque` and the `deque.h` inline functions.
//!
//! `deque.h` is header-inline, so nothing here is exported: these are plain
//! Rust functions with the C signatures that run the safe core
//! ([`crate::deque::Deque`]) in place on a `struct deque` that C may own.
//! The buffer is `bmalloc` memory from the Rust bmem port (re-exported from
//! [`crate::ffi::darray`]), so C's `deque_free` frees what these allocate,
//! and these grow and free what C allocated.

use core::ffi::c_void;
use core::ptr;
use core::slice;

use crate::deque::{Deque, DequeBuf, DequeState};
use crate::ffi::darray::{bfree, brealloc};

/// Mirrors `struct deque` in `libobs/util/deque.h`.
#[repr(C)]
#[allow(non_camel_case_types)]
#[derive(Debug)]
pub struct deque {
    pub data: *mut c_void,
    pub size: usize,
    pub start_pos: usize,
    pub end_pos: usize,
    pub capacity: usize,
}

/// The `bmalloc` buffer of a C deque. Invariant: `data` is null with
/// `len == 0`, or a bmem block of at least `len` bytes.
struct BmemBuf {
    data: *mut c_void,
    len: usize,
}

impl DequeBuf for BmemBuf {
    fn realloc(&mut self, len: usize) {
        // SAFETY: `data` is null or a live bmem block (the invariant).
        self.data = unsafe { brealloc(self.data, len) };
        self.len = len;
    }

    fn free(&mut self) {
        // SAFETY: as above; `bfree(NULL)` is a no-op.
        unsafe { bfree(self.data) };
        self.data = ptr::null_mut();
        self.len = 0;
    }

    fn bytes(&self) -> &[u8] {
        if self.data.is_null() || self.len == 0 {
            return &[];
        }
        // SAFETY: the block holds at least `len` bytes; the buffer is only
        // reached through this `BmemBuf` while it exists.
        unsafe { slice::from_raw_parts(self.data as *const u8, self.len) }
    }

    fn bytes_mut(&mut self) -> &mut [u8] {
        if self.data.is_null() || self.len == 0 {
            return &mut [];
        }
        // SAFETY: as in `bytes`, and `&mut self` makes the borrow unique.
        unsafe { slice::from_raw_parts_mut(self.data as *mut u8, self.len) }
    }
}

/// Runs `f` on the deque at `dq` and stores the state and buffer back, also
/// if `f` panics.
///
/// # Safety
///
/// `dq` must point to a valid `struct deque` whose `data` is null (with
/// `capacity == 0`) or a bmem block of `capacity` bytes, in a state the deque
/// functions can reach; nothing else may access it during the call.
unsafe fn with_deque<R>(dq: *mut deque, f: impl FnOnce(&mut Deque<BmemBuf>) -> R) -> R {
    struct WriteBack {
        dq: *mut deque,
        core: Deque<BmemBuf>,
    }
    impl Drop for WriteBack {
        fn drop(&mut self) {
            let st = self.core.state();
            let data = self.core.buf().data;
            // SAFETY: `dq` is valid for the whole call (caller contract).
            let d = unsafe { &mut *self.dq };
            d.data = data;
            d.size = st.size;
            d.start_pos = st.start_pos;
            d.end_pos = st.end_pos;
            d.capacity = st.capacity;
        }
    }

    // SAFETY: the caller guarantees `dq` is valid.
    let d = unsafe { &*dq };
    let state = DequeState {
        size: d.size,
        start_pos: d.start_pos,
        end_pos: d.end_pos,
        capacity: d.capacity,
    };
    let buf = BmemBuf {
        data: d.data,
        len: if d.data.is_null() { 0 } else { d.capacity },
    };
    let mut guard = WriteBack {
        dq,
        core: Deque::from_parts(state, buf),
    };
    f(&mut guard.core)
}

/// # Safety
///
/// `src` must be valid for reads of `size` bytes (any pointer when `size`
/// is 0) and must not overlap the deque's buffer.
unsafe fn bytes<'a>(src: *const c_void, size: usize) -> &'a [u8] {
    if size == 0 {
        &[]
    } else {
        // SAFETY: the caller guarantees `size` readable bytes.
        unsafe { slice::from_raw_parts(src as *const u8, size) }
    }
}

/// # Safety
///
/// `dst` must be null, or valid for writes of `size` bytes and not overlap
/// the deque's buffer.
unsafe fn bytes_mut<'a>(dst: *mut c_void, size: usize) -> Option<&'a mut [u8]> {
    if dst.is_null() {
        None
    } else if size == 0 {
        Some(&mut [])
    } else {
        // SAFETY: the caller guarantees `size` writable bytes.
        Some(unsafe { slice::from_raw_parts_mut(dst as *mut u8, size) })
    }
}

/// `deque_init`.
///
/// # Safety
///
/// `dq` must be valid for writes.
pub unsafe fn deque_init(dq: *mut deque) {
    // SAFETY: the caller guarantees `dq` is writable; all fields are plain.
    unsafe {
        dq.write(deque {
            data: ptr::null_mut(),
            size: 0,
            start_pos: 0,
            end_pos: 0,
            capacity: 0,
        })
    };
}

/// `deque_free`.
///
/// # Safety
///
/// As [`with_deque`].
pub unsafe fn deque_free(dq: *mut deque) {
    // SAFETY: forwarded caller contract.
    unsafe { with_deque(dq, |d| d.free()) }
}

/// `deque_reserve`.
///
/// # Safety
///
/// As [`with_deque`].
pub unsafe fn deque_reserve(dq: *mut deque, capacity: usize) {
    // SAFETY: forwarded caller contract.
    unsafe { with_deque(dq, |d| d.reserve(capacity)) }
}

/// `deque_upsize`.
///
/// # Safety
///
/// As [`with_deque`].
pub unsafe fn deque_upsize(dq: *mut deque, size: usize) {
    // SAFETY: forwarded caller contract.
    unsafe { with_deque(dq, |d| d.upsize(size)) }
}

/// `deque_place`.
///
/// # Safety
///
/// As [`with_deque`], and `data` as in [`bytes`].
pub unsafe fn deque_place(dq: *mut deque, position: usize, data: *const c_void, size: usize) {
    // SAFETY: forwarded caller contracts.
    unsafe {
        let src = bytes(data, size);
        with_deque(dq, |d| d.place(position, src))
    }
}

/// `deque_push_back`.
///
/// # Safety
///
/// As [`with_deque`], and `data` as in [`bytes`].
pub unsafe fn deque_push_back(dq: *mut deque, data: *const c_void, size: usize) {
    // SAFETY: forwarded caller contracts.
    unsafe {
        let src = bytes(data, size);
        with_deque(dq, |d| d.push_back(src))
    }
}

/// `deque_push_front`.
///
/// # Safety
///
/// As [`with_deque`], and `data` as in [`bytes`].
pub unsafe fn deque_push_front(dq: *mut deque, data: *const c_void, size: usize) {
    // SAFETY: forwarded caller contracts.
    unsafe {
        let src = bytes(data, size);
        with_deque(dq, |d| d.push_front(src))
    }
}

/// `deque_push_back_zero`.
///
/// # Safety
///
/// As [`with_deque`].
pub unsafe fn deque_push_back_zero(dq: *mut deque, size: usize) {
    // SAFETY: forwarded caller contract.
    unsafe { with_deque(dq, |d| d.push_back_zero(size)) }
}

/// `deque_push_front_zero`.
///
/// # Safety
///
/// As [`with_deque`].
pub unsafe fn deque_push_front_zero(dq: *mut deque, size: usize) {
    // SAFETY: forwarded caller contract.
    unsafe { with_deque(dq, |d| d.push_front_zero(size)) }
}

/// `deque_peek_front`; a null `data` only checks `size` (C asserts).
///
/// # Safety
///
/// As [`with_deque`], and `data` as in [`bytes_mut`].
pub unsafe fn deque_peek_front(dq: *mut deque, data: *mut c_void, size: usize) {
    // SAFETY: forwarded caller contracts.
    unsafe {
        let dst = bytes_mut(data, size);
        with_deque(dq, |d| match dst {
            Some(out) => d.peek_front(out),
            None => assert!(size <= d.size(), "deque_peek_front past the size"),
        })
    }
}

/// `deque_peek_back`; a null `data` only checks `size` (C asserts).
///
/// # Safety
///
/// As [`with_deque`], and `data` as in [`bytes_mut`].
pub unsafe fn deque_peek_back(dq: *mut deque, data: *mut c_void, size: usize) {
    // SAFETY: forwarded caller contracts.
    unsafe {
        let dst = bytes_mut(data, size);
        with_deque(dq, |d| match dst {
            Some(out) => d.peek_back(out),
            None => assert!(size <= d.size(), "deque_peek_back past the size"),
        })
    }
}

/// `deque_pop_front`; a null `data` drops the bytes.
///
/// # Safety
///
/// As [`with_deque`], and `data` as in [`bytes_mut`].
pub unsafe fn deque_pop_front(dq: *mut deque, data: *mut c_void, size: usize) {
    // SAFETY: forwarded caller contracts.
    unsafe {
        let dst = bytes_mut(data, size);
        with_deque(dq, |d| match dst {
            Some(out) => d.pop_front(out),
            None => d.discard_front(size),
        })
    }
}

/// `deque_pop_back`; a null `data` drops the bytes.
///
/// # Safety
///
/// As [`with_deque`], and `data` as in [`bytes_mut`].
pub unsafe fn deque_pop_back(dq: *mut deque, data: *mut c_void, size: usize) {
    // SAFETY: forwarded caller contracts.
    unsafe {
        let dst = bytes_mut(data, size);
        with_deque(dq, |d| match dst {
            Some(out) => d.pop_back(out),
            None => d.discard_back(size),
        })
    }
}

/// `deque_data`: a pointer to byte `idx` from the front, or null when
/// `idx >= size`.
///
/// # Safety
///
/// As [`with_deque`].
pub unsafe fn deque_data(dq: *mut deque, idx: usize) -> *mut c_void {
    // SAFETY: forwarded caller contract.
    unsafe {
        with_deque(dq, |d| match d.data_offset(idx) {
            Some(offset) => (d.buf().data as *mut u8).add(offset) as *mut c_void,
            None => ptr::null_mut(),
        })
    }
}
