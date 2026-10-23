//! Safe core of `libobs/obs-module-paths.c.h`: the module search paths.
//!
//! A search path is a binary directory and a data directory, either of which
//! can hold `%module%`. [`find`] reports every module a search path has, as
//! `obs_find_modules2()` and `obs_load_plugins()` do: without `%module%` in
//! the binary directory, every `*<ext>` file in it; with it, every directory
//! where the binary directory ends before its first `%module%`, whose binary
//! is `<name>` plus the host's extension, else `lib<name><ext>`. Each module
//! gets the data path of [`data_dir`]. [`find_file`] is the lookup of
//! `obs_find_module_file()`: a file under a module's data path.
//!
//! The file system is the caller's [`ModuleFs`]: `os_file_exists()` and
//! `os_glob()` in libobs, a fake in the tests. Paths are bytes, not text:
//! they come from the configuration and the file system, so they can hold
//! anything but NUL. `None` is a NULL string, and the C's `struct dstr`
//! steps are followed exactly, NULL against empty included: a copy of an
//! empty string is NULL, and a path cut to nothing is NULL. Nothing here
//! panics, indexes or overflows for any input.
//!
//! Kept as the C has it, characterized and not endorsed (#1876):
//!
//! - a data path is reported whether or not it exists; for a name that
//!   starts with "lib" (ASCII, any case) whose data directory does not
//!   exist, the one without "lib" is reported, whether or not it exists;
//! - only one trailing '/' is taken off a data path, and a data path that
//!   is empty or `"/"` reports no module;
//! - a module directory is looked up under its name up to the last '.',
//!   so one with a '.' in its name is never found;
//! - after `<name>` plus the host's extension, only `lib<name><ext>` is
//!   tried (the C appends ".so" for a "legacy fallback" and then overwrites
//!   it);
//! - [`find_file`] with an empty file returns the data path itself, and with
//!   an empty data path looks the file up as a relative path.
//!
//! Intentional difference: an empty binary directory (or NULL) searches
//! nothing. The C copies it to a NULL `dstr` and passes that to `strstr()`,
//! which crashes.

/// The text a search path holds for the module name.
pub const MODULE_PLACEHOLDER: &[u8] = b"%module%";

/// The prefix a module binary can have that its data directory lacks.
pub const LIB_PREFIX: &[u8] = b"lib";

/// One match of [`ModuleFs::glob`], as `struct os_globent`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobEntry {
    pub path: Vec<u8>,
    pub directory: bool,
}

/// The file system the search runs on.
pub trait ModuleFs {
    /// `os_file_exists()`: a file or directory is at `path`.
    fn file_exists(&self, path: &[u8]) -> bool;
    /// `os_glob(pattern, 0)`: the matches, `None` when it fails (no match
    /// included).
    fn glob(&self, pattern: &[u8]) -> Option<Vec<GlobEntry>>;
}

/// One module [`find`] reports, as `struct obs_module_info2` without the
/// type (the caller's). Valid for the callback only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModuleInfo<'a> {
    /// Never `None` from a real `os_glob()`, whose paths are not empty.
    pub bin_path: Option<&'a [u8]>,
    pub data_path: &'a [u8],
    /// `None` for a file name that is only an extension (".so").
    pub name: Option<&'a [u8]>,
}

/// Same contract as C `obs_module_paths_data_dir`: `data_dir` for the module
/// `name`, every `%module%` replaced by the name and one trailing '/' taken
/// off; for a name that starts with "lib" (any case) whose path does not
/// exist, the path for the name without "lib", whether or not that exists.
/// `None` when the path is NULL (an empty or NULL `data_dir`, or `"/"`).
pub fn data_dir(
    fs: &impl ModuleFs,
    name: Option<&[u8]>,
    data_dir: Option<&[u8]>,
) -> Option<Vec<u8>> {
    todo!()
}

/// Same contract as C `obs_module_paths_find_file`: `data_path`, one '/'
/// when it has none at the end and `file` is not empty, then `file`;
/// `None` when that does not exist. A NULL `file` is empty.
pub fn find_file(
    fs: &impl ModuleFs,
    data_path: Option<&[u8]>,
    file: Option<&[u8]>,
) -> Option<Vec<u8>> {
    todo!()
}

/// The search of C `obs_module_paths_find` up to `os_glob()`: the pattern,
/// and whether its matches are module directories (`%module%` in `bin`)
/// rather than module files. `None` for an empty binary directory
/// (intentional difference: the C crashes).
pub fn search_pattern(bin: &[u8], ext: &[u8]) -> Option<(Vec<u8>, bool)> {
    todo!()
}

/// Same contract as C `obs_module_paths_find`: calls `callback` for every
/// module of the search path `bin`/`data`, in the order of the matches of
/// [`search_pattern`]. `add_extension` is the host's hook
/// (`obs_host_add_module_extension()`), called on `<directory>/<name>` for
/// each module directory. A NULL `data` reports nothing; an empty or NULL
/// `bin` searches nothing (intentional difference: the C crashes).
pub fn find(
    fs: &impl ModuleFs,
    bin: Option<&[u8]>,
    data: Option<&[u8]>,
    ext: &[u8],
    add_extension: &mut dyn FnMut(&mut Vec<u8>),
    callback: &mut dyn FnMut(&ModuleInfo<'_>),
) {
    todo!()
}
