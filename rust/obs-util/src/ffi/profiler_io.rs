//! C ABI shims for `libobs/util/profiler-io.c`: `profiler_print`,
//! `profiler_print_time_between_calls`, `profiler_snapshot_dump_csv`,
//! `profiler_snapshot_dump_csv_gz`, and the hidden `profile_print_func`.
//!
//! The snapshot is read in place through the `#[repr(C)]` mirrors in
//! [`super::profiler_snapshot`]; the logic is [`crate::profiler_io`]. Lines
//! go to `blog(LOG_INFO, "%s", line)`, as in the C.

use core::ffi::{CStr, c_char, c_int, c_uint};
use core::ptr;
use core::slice;

use super::dstr::dstr;
use super::profiler::profile_snapshot_create;
use super::profiler_snapshot::{
    Darray, profile_snapshot_free, profiler_snapshot, profiler_snapshot_entry,
};
use crate::base::LOG_INFO;
use crate::platform::path_from_bytes;
use crate::profiler_io::{self as io, Entry, Report, TimeEntry};

unsafe extern "C" {
    fn blog(log_level: c_int, format: *const c_char, ...);
}

/// `profile_entry_print_func`.
pub type EntryPrintFunc = Option<
    unsafe extern "C" fn(
        entry: *mut profiler_snapshot_entry,
        indent_buffer: *mut dstr,
        output_buffer: *mut dstr,
        indent: c_uint,
        active: u64,
        parent_calls: u64,
    ),
>;

/// A snapshot entry read in place. Only made from entries of a valid
/// snapshot (the shims' callers guarantee it), so its arrays hold `num`
/// valid elements and its name is NULL or a C string.
#[repr(transparent)]
struct CEntry(profiler_snapshot_entry);

/// The `num` elements of a C darray (none when `array` is NULL).
///
/// # Safety
///
/// `array` holds `num` valid `T`s that outlive the result.
unsafe fn darray_slice<T>(d: &Darray<T>) -> &[T] {
    if d.array.is_null() || d.num == 0 {
        &[]
    } else {
        // SAFETY: see the function contract.
        unsafe { slice::from_raw_parts(d.array, d.num) }
    }
}

impl CEntry {
    fn times_of(d: &Darray<super::profiler_snapshot::profiler_time_entry>) -> &[TimeEntry] {
        // SAFETY: the entry is from a valid snapshot (see `CEntry`), and
        // `profiler_time_entry` and `TimeEntry` are both `#[repr(C)]`
        // `{ u64 time_delta; u64 count; }`.
        unsafe {
            let s = darray_slice(d);
            slice::from_raw_parts(s.as_ptr().cast::<TimeEntry>(), s.len())
        }
    }
}

impl Entry for CEntry {
    fn id(&self) -> usize {
        ptr::from_ref(self).addr()
    }
    fn name(&self) -> &[u8] {
        if self.0.name.is_null() {
            return &[];
        }
        // SAFETY: a non-NULL name is a C string that outlives the snapshot.
        unsafe { CStr::from_ptr(self.0.name) }.to_bytes()
    }
    fn name_id(&self) -> usize {
        self.0.name.addr()
    }
    fn times(&self) -> &[TimeEntry] {
        Self::times_of(&self.0.times)
    }
    fn min_time(&self) -> u64 {
        self.0.min_time
    }
    fn max_time(&self) -> u64 {
        self.0.max_time
    }
    fn overall_count(&self) -> u64 {
        self.0.overall_count
    }
    fn times_between_calls(&self) -> &[TimeEntry] {
        Self::times_of(&self.0.times_between_calls)
    }
    fn expected_time_between_calls(&self) -> u64 {
        self.0.expected_time_between_calls
    }
    fn min_time_between_calls(&self) -> u64 {
        self.0.min_time_between_calls
    }
    fn max_time_between_calls(&self) -> u64 {
        self.0.max_time_between_calls
    }
    fn overall_between_calls_count(&self) -> u64 {
        self.0.overall_between_calls_count
    }
    fn children(&self) -> &[Self] {
        // SAFETY: the children darray holds `num` valid entries;
        // `CEntry` is `#[repr(transparent)]` over the C struct.
        unsafe {
            let s = darray_slice(&self.0.children);
            slice::from_raw_parts(s.as_ptr().cast::<CEntry>(), s.len())
        }
    }
}

/// The roots of `snap` (none for NULL).
///
/// # Safety
///
/// `snap` is NULL or a valid snapshot that outlives the result.
unsafe fn roots<'a>(snap: *const profiler_snapshot) -> &'a [CEntry] {
    if snap.is_null() {
        return &[];
    }
    // SAFETY: see the function contract; `CEntry` is transparent.
    unsafe {
        let s = darray_slice(&(*snap).roots);
        slice::from_raw_parts(s.as_ptr().cast::<CEntry>(), s.len())
    }
}

/// `blog(LOG_INFO, "%s", line)`, the line cut at a NUL as `%s` would.
fn log_line(line: &[u8]) {
    let line = line.split(|&b| b == 0).next().unwrap_or_default();
    let mut c = Vec::with_capacity(line.len() + 1);
    c.extend_from_slice(line);
    c.push(0);
    // SAFETY: the format takes one C string, which `c` is.
    unsafe { blog(LOG_INFO, c"%s".as_ptr(), c.as_ptr().cast::<c_char>()) };
}

/// Runs `f` on `snap`, or on a fresh snapshot of the profiler (freed after)
/// when `snap` is NULL.
///
/// # Safety
///
/// `snap` is NULL or a valid snapshot.
unsafe fn with_snapshot(snap: *mut profiler_snapshot, f: impl FnOnce(*mut profiler_snapshot)) {
    let free_snapshot = snap.is_null();
    // SAFETY: `profile_snapshot_create` returns a snapshot that is freed
    // below, exactly once.
    let snap = if free_snapshot {
        unsafe { profile_snapshot_create() }
    } else {
        snap
    };
    f(snap);
    if free_snapshot {
        // SAFETY: created above and not freed yet.
        unsafe { profile_snapshot_free(snap) };
    }
}

/// `profile_print_func`: logs `intro`, calls `print` on every root with
/// two scratch `dstr`s, logs the footer. Not `EXPORT` in C; hidden here.
///
/// # Safety
///
/// `intro` is a C string, `print` a valid printer (called only when there
/// are roots), and `snap` NULL or a valid snapshot.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn profile_print_func(
    intro: *const c_char,
    print: EntryPrintFunc,
    snap: *mut profiler_snapshot,
) {
    let mut indent_buffer = dstr {
        array: ptr::null_mut(),
        len: 0,
        capacity: 0,
    };
    let mut output_buffer = dstr {
        array: ptr::null_mut(),
        len: 0,
        capacity: 0,
    };

    // SAFETY: the caller's contract covers `snap`, `intro` and `print`.
    unsafe {
        with_snapshot(snap, |snap| {
            blog(LOG_INFO, c"%s".as_ptr(), intro);
            if !snap.is_null() {
                let roots = &mut (*snap).roots;
                for i in 0..roots.num {
                    if let Some(print) = print {
                        print(
                            roots.array.add(i),
                            &mut indent_buffer,
                            &mut output_buffer,
                            0,
                            0,
                            0,
                        );
                    }
                }
            }
            log_line(io::FOOTER);
        });
        super::bmem::bfree(output_buffer.array.cast());
        super::bmem::bfree(indent_buffer.array.cast());
    }
}

/// # Safety
///
/// `snap` is NULL or a valid snapshot.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn profiler_print(snap: *mut profiler_snapshot) {
    // SAFETY: the caller's contract covers `snap`.
    unsafe {
        with_snapshot(snap, |snap| {
            io::print(roots(snap), Report::Results, &mut |l| log_line(l));
        });
    }
}

/// # Safety
///
/// `snap` is NULL or a valid snapshot.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn profiler_print_time_between_calls(snap: *mut profiler_snapshot) {
    // SAFETY: the caller's contract covers `snap`.
    unsafe {
        with_snapshot(snap, |snap| {
            io::print(roots(snap), Report::TimeBetweenCalls, &mut |l| log_line(l));
        });
    }
}

/// The UTF-8 file name, or `None` for NULL.
///
/// # Safety
///
/// `filename` is NULL or a C string.
unsafe fn file_name(filename: *const c_char) -> Option<std::path::PathBuf> {
    if filename.is_null() {
        return None;
    }
    // SAFETY: see the function contract.
    Some(path_from_bytes(
        unsafe { CStr::from_ptr(filename) }.to_bytes(),
    ))
}

/// # Safety
///
/// `snap` is NULL or a valid snapshot; `filename` NULL or a C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn profiler_snapshot_dump_csv(
    snap: *const profiler_snapshot,
    filename: *const c_char,
) -> bool {
    // SAFETY: the caller's contract covers both pointers.
    unsafe { file_name(filename).is_some_and(|p| io::dump_csv_file(roots(snap), &p)) }
}

/// # Safety
///
/// `snap` is NULL or a valid snapshot; `filename` NULL or a C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn profiler_snapshot_dump_csv_gz(
    snap: *const profiler_snapshot,
    filename: *const c_char,
) -> bool {
    // SAFETY: the caller's contract covers both pointers.
    unsafe { file_name(filename).is_some_and(|p| io::dump_csv_gz_file(roots(snap), &p)) }
}
