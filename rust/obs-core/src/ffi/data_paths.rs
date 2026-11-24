//! `obs_add_data_path()` and `obs_remove_data_path()` (#5397) over the C list
//! of `libobs/obs-data-paths-fields.c.h`, which `obs_find_data_file()` keeps
//! reading. The list, its strings and their allocations stay C behind the
//! private adapters; what is pushed, compared and erased is the safe core's
//! ([`crate::data_paths`]).
use core::ffi::{CStr, c_char};

use crate::data_paths::{PathList, add, remove};

unsafe extern "C" {
    fn obs_data_paths_count() -> usize;
    fn obs_data_paths_entry(index: usize) -> *const c_char;
    fn obs_data_paths_push_copy(path: *const c_char);
    fn obs_data_paths_erase(index: usize);
}

/// The C list. Callers hold no lock, as in the original: another thread's
/// change of the list is outside its contract.
struct CoreList;

/// A string's bytes, empty for NULL, as `dstr_cmp()` reads both sides.
///
/// # Safety
/// `string` is NULL or a terminated string that outlives the result.
unsafe fn bytes<'a>(string: *const c_char) -> &'a [u8] {
    if string.is_null() {
        return &[];
    }
    // SAFETY: the caller's terminated string.
    unsafe { CStr::from_ptr(string) }.to_bytes()
}

impl PathList for CoreList {
    type Path = *const c_char;
    fn count(&self) -> usize {
        // SAFETY: reads the list's length.
        unsafe { obs_data_paths_count() }
    }
    fn entry(&self, index: usize) -> &[u8] {
        // SAFETY: `index` is below the count just read; the entry's string
        // lives until it is erased, which `remove` does only after its last
        // look at it.
        unsafe { bytes(obs_data_paths_entry(index)) }
    }
    fn push_copy(&mut self, path: *const c_char) {
        // SAFETY: the caller's path (or NULL), copied by the adapter.
        unsafe { obs_data_paths_push_copy(path) }
    }
    fn erase(&mut self, index: usize) {
        // SAFETY: `index` is below the count just read.
        unsafe { obs_data_paths_erase(index) }
    }
}

/// # Safety
/// `path` is NULL or a terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_add_data_path(path: *const c_char) {
    add(&mut CoreList, path);
}

/// # Safety
/// `path` is NULL or a terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_remove_data_path(path: *const c_char) -> bool {
    // SAFETY: the caller's path, read during the call only.
    let path = unsafe { bytes(path) };
    remove(&mut CoreList, path)
}
