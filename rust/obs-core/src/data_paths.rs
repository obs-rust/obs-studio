//! Safe core of `libobs/obs-data-paths.c.h`: `obs_add_data_path()` and
//! `obs_remove_data_path()` (#5397), the deprecated wrappers around the
//! process-global list of extra data directories that `obs_find_data_file()`
//! searches, in order, after the libobs data directory.
//!
//! Adding appends a copy of the caller's path (duplicates kept). Removing
//! compares the path with each entry in order, as `dstr_cmp()` does (a NULL
//! entry or path is the empty string, bytes compared as `strcmp()` does),
//! erases the first equal entry and answers true, or answers false when none
//! is equal. The list itself, its strings and their allocations stay in C,
//! behind [`PathList`], so `obs_find_data_file()` keeps reading the same
//! list; this module decides what is added, compared and erased.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::unreachable
)]

/// The list of extra data paths, as the original sees it.
pub trait PathList {
    /// The caller's path, passed through to [`PathList::push_copy`].
    type Path: Copy;
    /// `core_module_paths.num`, read before each comparison.
    fn count(&self) -> usize;
    /// Entry `index`'s bytes without the terminator; an entry stored as
    /// NULL (an empty path) is empty.
    fn entry(&self, index: usize) -> &[u8];
    /// `dstr_init_copy(da_push_back_new(core_module_paths), path)`.
    fn push_copy(&mut self, path: Self::Path);
    /// `dstr_free(&core_module_paths.array[index])`, then
    /// `da_erase(core_module_paths, index)`.
    fn erase(&mut self, index: usize);
}

/// `dstr_cmp()`'s result for an entry and a path, both already mapped from
/// NULL to empty: the sign `strcmp()` gives, as -1, 0 or 1.
#[expect(clippy::todo, reason = "RED: the port lands in the next commit")]
pub fn compare(entry: &[u8], path: &[u8]) -> i32 {
    let _ = (entry, path);
    todo!()
}

/// `obs_add_data_path(path)`: a copy of `path` appended.
#[expect(clippy::todo, reason = "RED: the port lands in the next commit")]
pub fn add<L: PathList>(list: &mut L, path: L::Path) {
    let _ = (list, path);
    todo!()
}

/// `obs_remove_data_path(path)`: the first entry equal to `path` (its bytes,
/// empty for NULL) erased, true; false when no entry is equal.
#[expect(clippy::todo, reason = "RED: the port lands in the next commit")]
pub fn remove<L: PathList>(list: &mut L, path: &[u8]) -> bool {
    let _ = (list, path);
    todo!()
}
