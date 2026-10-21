//! C ABI shims for the `EXPORT`ed functions that stay in
//! `libobs/util/dstr.c`.
//!
//! `struct dstr` is public (the `dstr.h` inline helpers read and write it),
//! so the layout below is the contract. The hot paths (`dstr_copy`, the
//! appends and `dstr_replace`/`dstr_safe_printf`) edit the C buffer in place
//! and grow it with `brealloc` exactly like `dstr_ensure_capacity`, so the
//! existing string is never copied out and back (#1297). The rarer shims
//! read the raw dstr into the safe [`Dstr`] model, run the safe core and
//! write the result back. Either way the buffer only ever comes from the
//! `bmalloc` family, so C can later free or resize it.

// c_char is i8 on most targets and u8 on some (aarch64-pc-windows-msvc,
// aarch64 Linux): the `as u8` / `as *const u8` conversions below are needed on
// the first and no-ops on the second, where clippy calls them unnecessary.
#![allow(clippy::unnecessary_cast)]

use core::ffi::{CStr, c_char, c_int, c_void};
use core::{mem, ptr, slice};

use super::darray::{bfree, bmalloc, brealloc};
use crate::darray::grow_capacity;
use crate::dstr::{self as core_dstr, Dstr, ReplacePlan};

/// Mirrors `struct dstr` in `libobs/util/dstr.h`.
#[repr(C)]
#[allow(non_camel_case_types)]
#[derive(Debug)]
pub struct dstr {
    pub array: *mut c_char,
    pub len: usize,
    pub capacity: usize,
}

/// Mirrors `struct strref` in `libobs/util/lexer.h`. Declared here on
/// purpose; it is not shared with the lexer port.
#[repr(C)]
#[derive(Debug)]
pub struct Strref {
    pub array: *const c_char,
    pub len: usize,
}

/// # Safety
///
/// `d` must point to a valid `dstr` whose `array` is null or holds `len`
/// readable bytes.
unsafe fn read(d: *const dstr) -> Dstr {
    // SAFETY: the caller guarantees `d` is valid.
    let d = unsafe { &*d };
    if d.array.is_null() {
        return Dstr::new();
    }
    let bytes = if d.len == 0 {
        Vec::new()
    } else {
        // SAFETY: a non-null array holds `len` bytes.
        unsafe { slice::from_raw_parts(d.array as *const u8, d.len) }.to_vec()
    };
    Dstr::from_parts(bytes, d.capacity)
}

/// Stores `new` into `d`, leaving the struct exactly as `dstr_free` would
/// when `new` is unallocated.
///
/// # Safety
///
/// `d` must point to a valid `dstr` whose `array` is null or came from
/// `bmalloc`/`brealloc`.
unsafe fn write(d: *mut dstr, new: Dstr) {
    let (bytes, cap) = new.into_parts();
    // SAFETY: the caller guarantees `d` is valid.
    let d = unsafe { &mut *d };
    if cap == 0 {
        // SAFETY: `array` is null or from `bmalloc`.
        unsafe { bfree(d.array as *mut c_void) };
        d.array = ptr::null_mut();
        d.len = 0;
        d.capacity = 0;
        return;
    }
    assert!(cap > bytes.len(), "dstr capacity must exceed its length");
    let reuse = !d.array.is_null() && d.capacity == cap;
    let buf = if reuse {
        d.array
    } else {
        // SAFETY: plain allocation; `cap > 0`.
        unsafe { bmalloc(cap) as *mut c_char }
    };
    // SAFETY: `buf` holds `cap > bytes.len()` bytes.
    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr(), buf as *mut u8, bytes.len());
        *buf.add(bytes.len()) = 0;
    }
    if !reuse {
        // SAFETY: the old array is null or from `bmalloc`.
        unsafe { bfree(d.array as *mut c_void) };
    }
    d.array = buf;
    d.len = bytes.len();
    d.capacity = cap;
}

/// Reads `*d` into the safe model.
///
/// The Cocoa shims are the only callers, and that module is macOS-only.
///
/// # Safety
///
/// `d` must point at a valid `dstr` whose `array` is null or holds `len`
/// readable bytes.
#[cfg(target_os = "macos")]
pub(crate) unsafe fn read_model(d: *const dstr) -> Dstr {
    // SAFETY: the caller guarantees `d` is valid.
    unsafe { read(d) }
}

/// Stores `new` into `*d`.
///
/// The Cocoa shims are the only callers, and that module is macOS-only.
///
/// # Safety
///
/// `d` must point at a valid `dstr` whose `array` is null or came from
/// `bmalloc`/`brealloc`.
#[cfg(target_os = "macos")]
pub(crate) unsafe fn write_model(d: *mut dstr, new: Dstr) {
    // SAFETY: the caller guarantees `d` is valid.
    unsafe { write(d, new) }
}

/// Runs `f` on the safe model of `*d` and stores the result.
///
/// # Safety
///
/// As for [`read`] and [`write`].
unsafe fn with<R>(d: *mut dstr, f: impl FnOnce(&mut Dstr) -> R) -> R {
    // SAFETY: forwarded to the caller.
    let mut model = unsafe { read(d) };
    let r = f(&mut model);
    // SAFETY: forwarded to the caller.
    unsafe { write(d, model) };
    r
}

/// `dstr_ensure_capacity` from `dstr.h`: the same growth rule and the same
/// `brealloc` call, so `capacity` (and the allocator traffic) match C.
///
/// # Safety
///
/// `d` must point to a valid `dstr` whose `array` is null or came from the
/// `bmalloc` family. A grow may move the buffer, invalidating pointers into
/// it.
unsafe fn ensure_capacity(d: *mut dstr, new_size: usize) {
    // SAFETY: the caller guarantees `d` is valid.
    let d = unsafe { &mut *d };
    if new_size <= d.capacity {
        return;
    }
    let new_cap = grow_capacity(d.capacity, new_size);
    // SAFETY: `array` is null or from the `bmalloc` family; `new_cap > 0`.
    d.array = unsafe { brealloc(d.array as *mut c_void, new_cap) } as *mut c_char;
    d.capacity = new_cap;
}

/// `dstr_free`.
///
/// # Safety
///
/// As for [`ensure_capacity`].
unsafe fn free(d: *mut dstr) {
    // SAFETY: the caller guarantees `d` is valid and `array` freeable.
    unsafe {
        bfree((*d).array as *mut c_void);
        (*d).array = ptr::null_mut();
        (*d).len = 0;
        (*d).capacity = 0;
    }
}

/// Writes the `n` bytes at `src` to offset `at` of `d`'s buffer (growing it
/// to `at + n + 1` first), then sets `len = at + n` and the terminator.
/// Shared body of the appends (`at = len`) and `dstr_copy` (`at = 0`).
///
/// `src` may point into `d`'s own buffer (`dstr_cat_dstr(d, d)`, or a
/// pointer into `d->array`): its offset is taken before the grow and the
/// pointer re-derived after it, so a moving `brealloc` cannot leave it
/// dangling, and the copy is a `memmove`.
///
/// # Safety
///
/// `d` as for [`ensure_capacity`], with `at <= len`; `src` must be valid
/// for `n` reads.
unsafe fn write_at(d: *mut dstr, at: usize, src: *const u8, n: usize) {
    // SAFETY: the caller guarantees `d` is valid.
    let (old, old_cap) = unsafe { ((*d).array as *const u8, (*d).capacity) };
    let own = (!old.is_null())
        .then(|| (src as usize).wrapping_sub(old as usize))
        .filter(|&off| off < old_cap);
    // SAFETY: forwarded to the caller.
    unsafe { ensure_capacity(d, at + n + 1) };
    // SAFETY: the buffer now holds `at + n + 1` bytes. An own-buffer source
    // is re-derived from the (possibly moved) buffer at the same offset; it
    // stays in bounds because `brealloc` keeps the old contents. `ptr::copy`
    // allows the ranges to overlap.
    unsafe {
        let buf = (*d).array as *mut u8;
        let src = match own {
            Some(off) => buf.add(off) as *const u8,
            None => src,
        };
        ptr::copy(src, buf.add(at), n);
        *buf.add(at + n) = 0;
        (*d).len = at + n;
    }
}

/// `dstr_replace` on the C buffer in place: no model copy, and a growing
/// replace grows the buffer through [`ensure_capacity`] as C does.
///
/// # Safety
///
/// `d` as for [`ensure_capacity`] with `array` (when non-null) holding
/// `len + 1` bytes; `find` and `replace` are C strings by value.
unsafe fn replace_in_place(d: *mut dstr, find: &[u8], replace: &[u8]) {
    // SAFETY: the caller guarantees `d` is valid.
    let (array, len) = unsafe { ((*d).array as *mut u8, (*d).len) };
    // `dstr_is_empty`.
    // SAFETY: a non-null array with `len > 0` holds at least one byte.
    if array.is_null() || len == 0 || unsafe { *array } == 0 {
        return;
    }
    // SAFETY: a non-null array holds `len` bytes; no other reference to the
    // buffer is live while `s` is.
    let s = unsafe { slice::from_raw_parts(array, len) };
    match core_dstr::replace_plan(s, find, replace) {
        ReplacePlan::None => {}
        ReplacePlan::InPlace { first } => {
            // SAFETY: as above, now mutably; nothing else borrows the buffer.
            let buf = unsafe { slice::from_raw_parts_mut(array, len) };
            let n = core_dstr::replace_in_place(buf, first, find, replace);
            // SAFETY: `n <= len < capacity`, so the terminator fits.
            unsafe {
                *array.add(n) = 0;
                (*d).len = n;
            }
        }
        ReplacePlan::Grow { new_len, first } => {
            // SAFETY: forwarded to the caller; `array` is not used after.
            unsafe { ensure_capacity(d, new_len + 1) };
            // SAFETY: the buffer now holds `new_len + 1` bytes, the first
            // `len` of them the original string (`brealloc` keeps contents).
            unsafe {
                let buf = (*d).array as *mut u8;
                core_dstr::replace_grow(
                    slice::from_raw_parts_mut(buf, new_len),
                    len,
                    first,
                    find,
                    replace,
                );
                *buf.add(new_len) = 0;
                (*d).len = new_len;
            }
        }
    }
}

/// `dstr_copy` on the C buffer.
///
/// # Safety
///
/// `d` as for [`ensure_capacity`]; `array` null or a NUL-terminated string
/// (it may point into `d`'s own buffer).
unsafe fn copy_in_place(d: *mut dstr, array: *const c_char) {
    // SAFETY: forwarded to the caller.
    match unsafe { opt_cstr(array) } {
        Some(s) if !s.is_empty() => {
            // SAFETY: `s` is readable for `s.len()` bytes; `write_at`
            // handles a source inside `d`'s buffer.
            unsafe { write_at(d, 0, s.as_ptr(), s.len()) }
        }
        // SAFETY: forwarded to the caller.
        _ => unsafe { free(d) },
    }
}

/// Whether `p` points into the buffer owned by `d`.
///
/// # Safety
///
/// `d` must be a valid `dstr`.
unsafe fn points_into(d: *mut dstr, p: *const c_char) -> bool {
    // SAFETY: the caller guarantees `d`.
    let (array, capacity) = unsafe { ((*d).array as *const u8, (*d).capacity) };
    if p.is_null() || array.is_null() || capacity == 0 {
        return false;
    }
    (p as *const u8 as usize).wrapping_sub(array as usize) < capacity
}

/// `dstr_safe_printf` in one pass: grow once, write once.
///
/// `dstr_copy` followed by one `dstr_replace` per value rescans and rebuilds
/// the growing buffer for every placeholder, so a 4096-byte value is copied
/// once per pass (#1296). With no `$` in the values nothing can introduce a
/// later placeholder, so the result is the format with `$1`..`$4` substituted
/// left to right.
///
/// # Safety
///
/// `d` must be a valid `dstr`. No argument may point into `d`'s buffer: the
/// buffer is grown before anything is written.
unsafe fn write_safe_printf(d: *mut dstr, format: &[u8], vals: [Option<&[u8]>; 4]) {
    if format.is_empty() {
        // `dstr_copy` of an empty format frees, and the replaces then no-op.
        // SAFETY: forwarded to the caller.
        unsafe { free(d) };
        return;
    }

    let mut new_len = 0usize;
    let mut i = 0;
    while i < format.len() {
        match crate::dstr::placeholder_at(format, i, vals) {
            Some(val) => {
                new_len += val.len();
                i += 2;
            }
            None => {
                new_len += 1;
                i += 1;
            }
        }
    }

    // `dstr_copy` sizes the buffer for the format and a growing replace grows
    // it further, so an empty result from a non-empty format keeps a non-NULL
    // buffer. SAFETY: forwarded to the caller; the arguments do not alias, so a
    // moved buffer cannot invalidate them.
    unsafe { ensure_capacity(d, format.len().max(new_len) + 1) };
    // SAFETY: the buffer now holds `new_len + 1` bytes and `format`/`vals` are
    // readable and outside it.
    unsafe {
        let buf = (*d).array as *mut u8;
        let mut w = 0usize;
        let mut i = 0;
        while i < format.len() {
            match crate::dstr::placeholder_at(format, i, vals) {
                Some(val) => {
                    ptr::copy_nonoverlapping(val.as_ptr(), buf.add(w), val.len());
                    w += val.len();
                    i += 2;
                }
                None => {
                    *buf.add(w) = format[i];
                    w += 1;
                    i += 1;
                }
            }
        }
        debug_assert_eq!(w, new_len);
        *buf.add(new_len) = 0;
        (*d).len = new_len;
    }
}

/// # Safety
///
/// `p` must be null or a NUL-terminated string.
unsafe fn opt_cstr<'a>(p: *const c_char) -> Option<&'a [u8]> {
    if p.is_null() {
        None
    } else {
        // SAFETY: non-null and NUL-terminated per the contract.
        Some(unsafe { CStr::from_ptr(p) }.to_bytes())
    }
}

/// # Safety
///
/// `r` must point to a valid `strref` whose `array` is null or holds `len`
/// readable bytes.
unsafe fn strref_bytes<'a>(r: *const Strref) -> &'a [u8] {
    // SAFETY: the caller guarantees `r` is valid.
    let r = unsafe { &*r };
    if r.array.is_null() || r.len == 0 {
        &[]
    } else {
        // SAFETY: `array` holds `len` bytes per the contract.
        unsafe { slice::from_raw_parts(r.array as *const u8, r.len) }
    }
}

/// Compares nullable C strings without reading farther than the C bounded
/// compare loop would.
///
/// # Safety
///
/// Each non-null pointer must be readable through the first differing byte,
/// a NUL byte, or n bytes, whichever comes first.
unsafe fn compare_cstr_n(a: *const c_char, b: *const c_char, n: usize, fold: bool) -> c_int {
    // SAFETY: the core requests bytes in order and stops at a mismatch, NUL,
    // or n; the caller guarantees those bytes are readable.
    core_dstr::compare_n_by(n, fold, |i| {
        let x = if a.is_null() {
            0
        } else {
            // SAFETY: the caller guarantees readability through this index.
            unsafe { *a.add(i) as u8 }
        };
        let y = if b.is_null() {
            0
        } else {
            // SAFETY: the caller guarantees readability through this index.
            unsafe { *b.add(i) as u8 }
        };
        (x, y)
    })
}

/// # Safety
///
/// `a` and `b` must each be null or NUL-terminated strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn astrcmpi(a: *const c_char, b: *const c_char) -> c_int {
    // SAFETY: forwarded to the caller.
    unsafe { core_dstr::astrcmpi(opt_cstr(a), opt_cstr(b)) }
}

/// # Safety
///
/// Each non-null pointer must be readable through the first differing byte,
/// a NUL byte, or n bytes, whichever comes first.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn astrcmp_n(a: *const c_char, b: *const c_char, n: usize) -> c_int {
    // SAFETY: forwarded to the caller.
    unsafe { compare_cstr_n(a, b, n, false) }
}

/// # Safety
///
/// Each non-null pointer must be readable through the first differing byte,
/// a NUL byte, or n bytes, whichever comes first.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn astrcmpi_n(a: *const c_char, b: *const c_char, n: usize) -> c_int {
    // SAFETY: forwarded to the caller.
    unsafe { compare_cstr_n(a, b, n, true) }
}

/// # Safety
///
/// `str` and `find` must each be null or NUL-terminated strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn astrstri(str: *const c_char, find: *const c_char) -> *mut c_char {
    // SAFETY: forwarded to the caller.
    match unsafe { core_dstr::astrstri(opt_cstr(str), opt_cstr(find)) } {
        // SAFETY: the offset is within the string `str` points to.
        Some(off) => unsafe { str.add(off) as *mut c_char },
        None => ptr::null_mut(),
    }
}

/// # Safety
///
/// `str` must be null or a writable NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn strdepad(str: *mut c_char) -> *mut c_char {
    if str.is_null() {
        return str;
    }
    // SAFETY: `str` is a writable NUL-terminated string.
    unsafe {
        let n = CStr::from_ptr(str).to_bytes().len();
        core_dstr::strdepad(slice::from_raw_parts_mut(str as *mut u8, n + 1));
    }
    str
}

/// # Safety
///
/// `str` must be null or a NUL-terminated string. The result is one
/// `bmalloc` block to be released with [`strlist_free`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn strlist_split(
    str: *const c_char,
    split_ch: c_char,
    include_empty: bool,
) -> *mut *mut c_char {
    // SAFETY: forwarded to the caller.
    let Some(s) = (unsafe { opt_cstr(str) }) else {
        return ptr::null_mut();
    };
    let pieces = core_dstr::strlist_split(s, split_ch as u8, include_empty);
    // Pointer table (NULL terminated) followed by the NUL-terminated pieces.
    let table_size = (pieces.len() + 1) * mem::size_of::<*mut c_char>();
    let total = table_size + pieces.iter().map(|p| p.len() + 1).sum::<usize>();
    // SAFETY: plain allocation; `total >= size_of::<*mut c_char>() > 0`.
    let out = unsafe { bmalloc(total) } as *mut u8;
    let table = out as *mut *mut c_char;
    // SAFETY: `out` holds `total` bytes, which covers the table and every
    // piece with its terminator.
    unsafe {
        let mut offset = out.add(table_size);
        for (i, piece) in pieces.iter().enumerate() {
            *table.add(i) = offset as *mut c_char;
            ptr::copy_nonoverlapping(piece.as_ptr(), offset, piece.len());
            *offset.add(piece.len()) = 0;
            offset = offset.add(piece.len() + 1);
        }
        *table.add(pieces.len()) = ptr::null_mut();
    }
    table
}

/// # Safety
///
/// `strlist` must be null or come from [`strlist_split`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn strlist_free(strlist: *mut *mut c_char) {
    // SAFETY: the block came from `bmalloc` (or is null).
    unsafe { bfree(strlist as *mut c_void) };
}

/// # Safety
///
/// `dst` must be valid for writes; `src` must point to a valid `strref`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dstr_init_copy_strref(dst: *mut dstr, src: *const Strref) {
    // SAFETY: `dst` is valid for writes; `dstr_init` just clears it.
    unsafe {
        (*dst).array = ptr::null_mut();
        (*dst).len = 0;
        (*dst).capacity = 0;
        dstr_copy_strref(dst, src);
    }
}

/// # Safety
///
/// `dst` must point to a valid `dstr`; `array` must be null or a
/// NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dstr_copy(dst: *mut dstr, array: *const c_char) {
    // SAFETY: forwarded to the caller.
    unsafe { copy_in_place(dst, array) };
}

/// # Safety
///
/// `dst` must point to a valid `dstr`; `src` must point to a valid `strref`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dstr_copy_strref(dst: *mut dstr, src: *const Strref) {
    // SAFETY: forwarded to the caller; the core copies `s` before `dst` is
    // rewritten, so `src` may alias `dst`'s buffer.
    unsafe {
        let s = strref_bytes(src);
        with(dst, |d| d.ncopy(s));
    }
}

/// # Safety
///
/// `dst` must point to a valid `dstr`; `array` must be valid for `len`
/// bytes when `len` is non-zero.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dstr_ncopy(dst: *mut dstr, array: *const c_char, len: usize) {
    // SAFETY: forwarded to the caller.
    unsafe {
        let s = if len == 0 {
            &[][..]
        } else {
            slice::from_raw_parts(array as *const u8, len)
        };
        with(dst, |d| d.ncopy(s));
    }
}

/// # Safety
///
/// `dst` and `src` must point to valid `dstr`s.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dstr_ncopy_dstr(dst: *mut dstr, src: *const dstr, len: usize) {
    // SAFETY: forwarded to the caller.
    unsafe {
        let s = read(src);
        with(dst, |d| d.ncopy_dstr(s.as_bytes(), len));
    }
}

/// # Safety
///
/// `dst` and `src` must point to valid `dstr`s.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dstr_cat_dstr(dst: *mut dstr, src: *const dstr) {
    // SAFETY: the caller guarantees `src` is valid. Read its fields before
    // `dst` changes: `src` may be `dst` itself, which `write_at` handles.
    let (array, len) = unsafe { ((*src).array as *const u8, (*src).len) };
    if len == 0 {
        return;
    }
    // SAFETY: a source with `len > 0` holds `len` bytes; `dst` is valid.
    unsafe { write_at(dst, (*dst).len, array, len) };
}

/// # Safety
///
/// `dst` must point to a valid `dstr`; `src` to a valid `strref`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dstr_cat_strref(dst: *mut dstr, src: *const Strref) {
    // SAFETY: the caller guarantees `src` is valid.
    unsafe { dstr_ncat(dst, (*src).array, (*src).len) };
}

/// # Safety
///
/// `dst` must point to a valid `dstr`; `array` must be null or valid for
/// `len` bytes (and for one byte when `len` is zero).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dstr_ncat(dst: *mut dstr, array: *const c_char, len: usize) {
    // SAFETY: a non-null `array` is readable for `len >= 1` bytes.
    if array.is_null() || len == 0 || unsafe { *array } == 0 {
        return;
    }
    // SAFETY: `array` holds `len` bytes (it may point into `dst`'s buffer,
    // which `write_at` handles); `dst` is valid.
    unsafe { write_at(dst, (*dst).len, array as *const u8, len) };
}

/// # Safety
///
/// `dst` and `src` must point to valid `dstr`s.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dstr_ncat_dstr(dst: *mut dstr, src: *const dstr, len: usize) {
    // SAFETY: the caller guarantees `src` is valid; read before `dst`
    // changes, since `src` may be `dst`.
    let (array, src_len) = unsafe { ((*src).array as *const u8, (*src).len) };
    // SAFETY: a non-null array holds at least its terminator.
    if array.is_null() || unsafe { *array } == 0 || len == 0 {
        return;
    }
    // SAFETY: the source holds `src_len >= min(len, src_len)` bytes.
    unsafe { write_at(dst, (*dst).len, array, len.min(src_len)) };
}

/// # Safety
///
/// `dst` must point to a valid `dstr` with `idx <= len`; `array` must be
/// null or a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dstr_insert(dst: *mut dstr, idx: usize, array: *const c_char) {
    // SAFETY: forwarded to the caller.
    unsafe {
        let s = opt_cstr(array).unwrap_or(&[]);
        with(dst, |d| d.insert(idx, s));
    }
}

/// # Safety
///
/// `dst` and `src` must point to valid `dstr`s with `idx <= dst.len`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dstr_insert_dstr(dst: *mut dstr, idx: usize, src: *const dstr) {
    // SAFETY: forwarded to the caller.
    unsafe {
        let s = read(src);
        with(dst, |d| d.insert_dstr(idx, s.as_bytes()));
    }
}

/// # Safety
///
/// `dst` must point to a valid `dstr` with `idx <= len`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dstr_insert_ch(dst: *mut dstr, idx: usize, ch: c_char) {
    // SAFETY: forwarded to the caller.
    unsafe { with(dst, |d| d.insert_ch(idx, ch as u8)) };
}

/// # Safety
///
/// `dst` must point to a valid `dstr` with `idx + count <= len` (unless
/// `count == len`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dstr_remove(dst: *mut dstr, idx: usize, count: usize) {
    // SAFETY: forwarded to the caller.
    unsafe { with(dst, |d| d.remove(idx, count)) };
}

/// # Safety
///
/// `dst` must point to a valid `dstr`; every string argument must be null
/// or NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dstr_safe_printf(
    dst: *mut dstr,
    format: *const c_char,
    val1: *const c_char,
    val2: *const c_char,
    val3: *const c_char,
    val4: *const c_char,
) {
    // SAFETY: forwarded to the caller.
    unsafe {
        // SAFETY: every argument is null or NUL-terminated per the contract.
        let vals = [val1, val2, val3, val4].map(|p| opt_cstr(p));
        let fmt = opt_cstr(format).unwrap_or(b"");
        // C replaces `$1`..`$4` in four passes, so a value that itself holds a
        // later placeholder is substituted by that later pass. Keep those
        // passes when a value holds a `$` (#1296), and also when an argument
        // points into `d`'s own buffer: the one-pass writer grows the buffer
        // first and would have to re-derive such a pointer afterwards.
        let aliased = points_into(dst, format)
            || [val1, val2, val3, val4]
                .into_iter()
                .any(|p| points_into(dst, p));
        // `memchr` is SIMD; `slice::contains` is a byte-at-a-time scan of the
        // whole value, which at 4 KB cost more than the rebuild it avoids.
        if aliased
            || vals
                .iter()
                .flatten()
                .any(|v| memchr::memchr(b'$', v).is_some())
        {
            copy_in_place(dst, format);
            for (find, val) in [c"$1", c"$2", c"$3", c"$4"].into_iter().zip(vals) {
                if let Some(val) = val {
                    replace_in_place(dst, find.to_bytes(), val);
                }
            }
            return;
        }
        write_safe_printf(dst, fmt, vals);
    }
}

/// # Safety
///
/// `str` must point to a valid `dstr`; `find` and `replace` must be
/// NUL-terminated strings (`replace` may be null).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dstr_replace(str: *mut dstr, find: *const c_char, replace: *const c_char) {
    // SAFETY: forwarded to the caller.
    unsafe {
        let find = opt_cstr(find).unwrap_or(&[]);
        let replace = opt_cstr(replace).unwrap_or(&[]);
        replace_in_place(str, find, replace);
    }
}

/// # Safety
///
/// `str` must point to a valid `dstr`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dstr_depad(str: *mut dstr) {
    // SAFETY: forwarded to the caller.
    unsafe { with(str, |d| d.depad()) };
}

/// # Safety
///
/// `dst` and `str` must point to valid `dstr`s (they may be the same) with
/// `pos <= str.len`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dstr_left(dst: *mut dstr, str: *const dstr, pos: usize) {
    // SAFETY: forwarded to the caller.
    unsafe {
        if ptr::eq(dst as *const dstr, str) {
            with(dst, |d| d.left_in_place(pos));
        } else {
            let s = read(str);
            with(dst, |d| d.left(s.as_bytes(), pos));
        }
    }
}

/// # Safety
///
/// `dst` and `str` must point to valid `dstr`s with `start + count <=
/// str.len`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dstr_mid(dst: *mut dstr, str: *const dstr, start: usize, count: usize) {
    // SAFETY: forwarded to the caller.
    unsafe {
        let s = read(str);
        with(dst, |d| d.mid(s.as_bytes(), start, count));
    }
}

/// # Safety
///
/// `dst` and `str` must point to valid `dstr`s with `pos <= str.len`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dstr_right(dst: *mut dstr, str: *const dstr, pos: usize) {
    // SAFETY: forwarded to the caller.
    unsafe {
        let s = read(str);
        with(dst, |d| d.right(s.as_bytes(), pos));
    }
}
