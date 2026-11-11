//! C ABI for `libobs/obs-missing-files.c.h`.
//!
//! The list and each file are `bmalloc` blocks, so `bnum_allocs` counts the
//! same objects the C file does. The pointer array is the `bmalloc` darray.
//! `add_file` does not take a reference. `append` does, and it re-reads
//! `num` each step, so appending a list to itself does not return.
//! `destroy` frees a file while its ref is still 1. `release(NULL)` returns.
//! A null list, a null file (except `release`), and a bad index are not
//! checked, matching the C file.
//!
//! `obs_source_get_name` is supplied by obs-source. The missing-file replacement
//! callback stays in `obs-source.c.h`; Cargo tests supply that callback symbol.

use std::ffi::{CStr, c_char, c_int, c_void};
use std::ptr;
use std::sync::atomic::Ordering;

use obs_util::bmem::CLongAtomic;
use obs_util::ffi::bmem::{bfree, bmalloc, bmemdup};
use obs_util::ffi::darray::{darray, darray_ensure_capacity, darray_free};

use crate::files::{calls_replace, copies_source_name, frees_source_name};

pub type MissingFileCb = Option<unsafe extern "C" fn(*mut c_void, *const c_char, *mut c_void)>;

unsafe extern "C" {
    fn obs_source_get_name(source: *const c_void) -> *const c_char;
    fn obs_source_replace_missing_file(
        cb: MissingFileCb,
        source: *mut c_void,
        new_path: *const c_char,
        data: *mut c_void,
    );
}

struct File {
    refcount: CLongAtomic,
    file_path: *mut c_char,
    callback: MissingFileCb,
    src_type: c_int,
    src: *mut c_void,
    src_name: *mut c_char,
    data: *mut c_void,
}

struct Files {
    files: darray,
}

/// `bzalloc`: `bmalloc` plus a zero of the block. A null result is returned.
unsafe fn bzalloc(size: usize) -> *mut c_void {
    // SAFETY: plain allocation. Size is the list or file struct, never 0.
    let mem = unsafe { bmalloc(size) };
    if !mem.is_null() {
        // SAFETY: `bmalloc` returned `size` bytes.
        unsafe { ptr::write_bytes(mem.cast::<u8>(), 0, size) };
    }
    mem
}

/// `bstrdup`: null stays null. Otherwise `bmemdup` of the bytes plus a NUL,
/// then `dup[n] = 0`, as `bstrdup_n` does.
unsafe fn bstrdup(text: *const c_char) -> *mut c_char {
    if text.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: `text` is a NUL-terminated C string.
    let n = unsafe { CStr::from_ptr(text) }.to_bytes().len();
    // SAFETY: `text` has `n` bytes and a trailing NUL. `bmemdup` copies `n + 1`.
    let dup = unsafe { bmemdup(text.cast(), n + 1) }.cast::<c_char>();
    // SAFETY: `dup` has `n + 1` bytes.
    unsafe { *dup.add(n) = 0 };
    dup
}

/// `da_insert` at `num`, which is `darray_push_back`: increment `num`, then
/// ensure that capacity, then write the pointer.
unsafe fn push_file(list: *mut Files, file: *mut File) {
    // SAFETY: `list` is a live list from `obs_missing_files_create`.
    let da = unsafe { &raw mut (*list).files };
    let n = unsafe { (*da).num };
    unsafe { (*da).num = n + 1 };
    // SAFETY: `da` is that list's darray. The element is one pointer.
    unsafe { darray_ensure_capacity(size_of::<*mut File>(), da, n + 1) };
    // SAFETY: capacity now covers index `n`.
    unsafe { ptr::write((*da).array.cast::<*mut File>().add(n), file) };
}

/// # Safety
/// Same contract as C `obs_missing_files_create`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_missing_files_create() -> *mut c_void {
    // SAFETY: one zeroed list. An empty darray does not allocate.
    unsafe { bzalloc(size_of::<Files>()) }
}

/// # Safety
/// Same contract as C `obs_missing_files_destroy`. `files` is a live list.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_missing_files_destroy(files: *mut c_void) {
    let list = files.cast::<Files>();
    // SAFETY: `files` is a live list. Each slot is released, then the buffer.
    unsafe {
        let n = (*list).files.num;
        for i in 0..n {
            let file = *(*list).files.array.cast::<*mut File>().add(i);
            obs_missing_file_release(file.cast());
        }
        darray_free(&raw mut (*list).files);
        bfree(files);
    }
}

/// # Safety
/// Same contract as C `obs_missing_files_add_file`. Does not take a reference.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_missing_files_add_file(files: *mut c_void, file: *mut c_void) {
    // SAFETY: `files` is a live list. `file` is stored as a pointer.
    unsafe { push_file(files.cast(), file.cast()) };
}

/// # Safety
/// Same contract as C `obs_missing_files_count`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_missing_files_count(files: *mut c_void) -> usize {
    // SAFETY: `files` is a live list.
    unsafe { (*files.cast::<Files>()).files.num }
}

/// # Safety
/// Same contract as C `obs_missing_files_get_file`. `idx` is in range.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_missing_files_get_file(files: *mut c_void, idx: c_int) -> *mut c_void {
    // SAFETY: `files` is a live list and `idx` is in range, as in C.
    unsafe {
        (*(*files.cast::<Files>())
            .files
            .array
            .cast::<*mut File>()
            .add(idx as usize))
        .cast()
    }
}

/// # Safety
/// Same contract as C `obs_missing_files_append`. `dst` and `src` are live
/// lists. A list appended to itself does not return.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_missing_files_append(dst: *mut c_void, src: *mut c_void) {
    let src = src.cast::<Files>();
    let mut i = 0;
    // SAFETY: `src` is a live list. `num` is re-read so a growing src is seen.
    while i < unsafe { (*src).files.num } {
        // SAFETY: `i` is in range at the start of this step.
        let file = unsafe { *(*src).files.array.cast::<*mut File>().add(i) };
        // SAFETY: `dst` is a live list. `file` is the slot's pointer.
        unsafe { obs_missing_files_add_file(dst, file.cast()) };
        // SAFETY: `file` is a live file. C increments even for the new slot.
        unsafe { (*file).refcount.fetch_add(1, Ordering::SeqCst) };
        i += 1;
    }
}

/// # Safety
/// Same contract as C `obs_missing_file_create`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_missing_file_create(
    path: *const c_char,
    callback: MissingFileCb,
    src_type: c_int,
    src: *mut c_void,
    data: *mut c_void,
) -> *mut c_void {
    // SAFETY: one file struct, then the path copy, then the source name.
    let mem = unsafe { bzalloc(size_of::<File>()) }.cast::<File>();
    let file_path = unsafe { bstrdup(path) };
    let src_name = if copies_source_name(src_type) {
        // SAFETY: `src` is null or a source. The real function, or the cargo
        // test stand-in, returns a C string or null.
        unsafe { bstrdup(obs_source_get_name(src)) }
    } else {
        ptr::null_mut()
    };
    // SAFETY: `mem` is the block just allocated.
    unsafe {
        ptr::write(
            mem,
            File {
                refcount: CLongAtomic::new(1),
                file_path,
                callback,
                src_type,
                src,
                src_name,
                data,
            },
        );
    }
    mem.cast()
}

/// # Safety
/// Same contract as C `obs_missing_file_release`. Null returns.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_missing_file_release(file: *mut c_void) {
    if file.is_null() {
        return;
    }
    let file = file.cast::<File>();
    // `os_atomic_dec_long` returns the new value. `fetch_sub` returns the old
    // one, so the file is destroyed when that old value is 1.
    // SAFETY: `file` is a live file.
    if unsafe { (*file).refcount.fetch_sub(1, Ordering::SeqCst) } == 1 {
        // SAFETY: the ref just hit 0.
        unsafe { obs_missing_file_destroy(file.cast()) };
    }
}

/// # Safety
/// Same contract as C `obs_missing_file_destroy`. Frees the file whatever
/// its ref is.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_missing_file_destroy(file: *mut c_void) {
    let file = file.cast::<File>();
    // SAFETY: `file` is a live file. The name is freed only for a source.
    unsafe {
        let src_type = (*file).src_type;
        let src_name = (*file).src_name;
        let path = (*file).file_path;
        if frees_source_name(src_type) {
            bfree(src_name.cast());
        }
        bfree(path.cast());
        bfree(file.cast());
    }
}

/// # Safety
/// Same contract as C `obs_missing_file_issue_callback`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_missing_file_issue_callback(
    file: *mut c_void,
    new_path: *const c_char,
) {
    let file = file.cast::<File>();
    // SAFETY: `file` is a live file.
    let (src_type, callback, src, data) = unsafe {
        (
            (*file).src_type,
            (*file).callback,
            (*file).src,
            (*file).data,
        )
    };
    if calls_replace(src_type) {
        // SAFETY: replace is the C function, or the cargo test stand-in.
        unsafe { obs_source_replace_missing_file(callback, src, new_path, data) };
    }
}

/// # Safety
/// Same contract as C `obs_missing_file_get_path`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_missing_file_get_path(file: *mut c_void) -> *const c_char {
    // SAFETY: `file` is a live file.
    unsafe { (*file.cast::<File>()).file_path }
}

/// # Safety
/// Same contract as C `obs_missing_file_get_source_name`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_missing_file_get_source_name(file: *mut c_void) -> *const c_char {
    // SAFETY: `file` is a live file.
    unsafe { (*file.cast::<File>()).src_name }
}
