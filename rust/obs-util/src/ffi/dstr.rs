//! C ABI shims for the `EXPORT`ed functions that stay in
//! `libobs/util/dstr.c`.
//!
//! `struct dstr` is public (the `dstr.h` inline helpers read and write it),
//! so the layout below is the contract. Every shim reads the raw dstr into
//! the safe [`Dstr`] model, runs the safe core and writes the result back,
//! allocating only through `bmalloc`/`bfree` so C can later free or resize
//! the buffer.

use core::ffi::{CStr, c_char, c_int, c_void};
use core::{mem, ptr, slice};

use super::darray::{bfree, bmalloc};
use crate::dstr::{self as core_dstr, Dstr};

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
/// `a` and `b` must each be null or NUL-terminated strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn astrcmp_n(a: *const c_char, b: *const c_char, n: usize) -> c_int {
    // SAFETY: forwarded to the caller.
    unsafe { core_dstr::astrcmp_n(opt_cstr(a), opt_cstr(b), n) }
}

/// # Safety
///
/// `a` and `b` must each be null or NUL-terminated strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn astrcmpi_n(a: *const c_char, b: *const c_char, n: usize) -> c_int {
    // SAFETY: forwarded to the caller.
    unsafe { core_dstr::astrcmpi_n(opt_cstr(a), opt_cstr(b), n) }
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
    unsafe {
        let s = opt_cstr(array).unwrap_or(&[]);
        with(dst, |d| d.copy(s));
    }
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
    // SAFETY: forwarded to the caller.
    unsafe {
        let s = read(src);
        with(dst, |d| d.cat_dstr(s.as_bytes()));
    }
}

/// # Safety
///
/// `dst` must point to a valid `dstr`; `src` to a valid `strref`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dstr_cat_strref(dst: *mut dstr, src: *const Strref) {
    // SAFETY: forwarded to the caller.
    unsafe {
        let s = strref_bytes(src);
        with(dst, |d| d.ncat(s));
    }
}

/// # Safety
///
/// `dst` must point to a valid `dstr`; `array` must be null or valid for
/// `len` bytes (and for one byte when `len` is zero).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dstr_ncat(dst: *mut dstr, array: *const c_char, len: usize) {
    if array.is_null() || len == 0 {
        return;
    }
    // SAFETY: forwarded to the caller.
    unsafe {
        let s = slice::from_raw_parts(array as *const u8, len);
        with(dst, |d| d.ncat(s));
    }
}

/// # Safety
///
/// `dst` and `src` must point to valid `dstr`s.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dstr_ncat_dstr(dst: *mut dstr, src: *const dstr, len: usize) {
    // SAFETY: forwarded to the caller.
    unsafe {
        let s = read(src);
        with(dst, |d| d.ncat_dstr(s.as_bytes(), len));
    }
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
        let vals = [
            opt_cstr(val1),
            opt_cstr(val2),
            opt_cstr(val3),
            opt_cstr(val4),
        ];
        let format = opt_cstr(format);
        with(dst, |d| d.safe_printf(format, vals));
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
        with(str, |d| d.replace(find, replace));
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
