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

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable
)]

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

/// A `struct dstr` being built: `None` is its NULL array.
pub(crate) type Dstr = Option<Vec<u8>>;

/// `dstr_copy`: NULL for a NULL or empty string.
pub(crate) fn copy(s: Option<&[u8]>) -> Dstr {
    s.filter(|s| !s.is_empty()).map(<[u8]>::to_vec)
}

/// `dstr_end`: the last byte, 0 for an empty or NULL string.
pub(crate) fn end(s: &Dstr) -> u8 {
    s.as_deref().and_then(<[u8]>::last).copied().unwrap_or(0)
}

/// `dstr_is_empty`.
pub(crate) fn is_empty(s: &Dstr) -> bool {
    s.as_deref().is_none_or(<[u8]>::is_empty)
}

/// `dstr_resize(s, len)`: NULL for 0.
pub(crate) fn resize(s: &mut Dstr, len: usize) {
    if len == 0 {
        *s = None;
    } else if let Some(v) = s {
        v.truncate(len);
    }
}

/// `dstr_resize(s, s->len - 1)` after `dstr_end(s) == '/'`, which is never
/// empty.
fn drop_last(s: &mut Dstr) {
    let len = s.as_ref().map_or(0, Vec::len);
    resize(s, len.saturating_sub(1));
}

/// `dstr_cat`: nothing for an empty string, NULL included.
pub(crate) fn cat(s: &mut Dstr, tail: &[u8]) {
    if !tail.is_empty() {
        s.get_or_insert_with(Vec::new).extend_from_slice(tail);
    }
}

/// `dstr_cat_ch`.
pub(crate) fn cat_ch(s: &mut Dstr, ch: u8) {
    s.get_or_insert_with(Vec::new).push(ch);
}

/// Where `needle` first starts in `hay`, as `strstr`.
fn position(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    hay.windows(needle.len()).position(|w| w == needle)
}

/// `dstr_replace(s, "%module%", name)`: every match, left to right, the
/// replacement not searched again; nothing for an empty string.
fn replace_placeholder(s: &mut Dstr, name: &[u8]) {
    let Some(v) = s.as_mut().filter(|v| !v.is_empty()) else {
        return;
    };
    let mut out = Vec::with_capacity(v.len());
    let mut rest: &[u8] = v;
    while let Some(at) = position(rest, MODULE_PLACEHOLDER) {
        let (before, from) = rest.split_at(at);
        out.extend_from_slice(before);
        out.extend_from_slice(name);
        rest = from.get(MODULE_PLACEHOLDER.len()..).unwrap_or_default();
    }
    out.extend_from_slice(rest);
    *v = out;
}

/// `astrcmpi_n(name, "lib", 3) == 0`, in ASCII.
fn has_lib_prefix(name: &[u8]) -> bool {
    name.get(..LIB_PREFIX.len())
        .is_some_and(|start| start.eq_ignore_ascii_case(LIB_PREFIX))
}

pub(crate) fn exists(fs: &impl ModuleFs, path: &Dstr) -> bool {
    // os_file_exists(NULL) is false.
    path.as_deref().is_some_and(|p| fs.file_exists(p))
}

/// `make_data_dir`: `data_dir` with every `%module%` replaced by `name`, one
/// trailing '/' taken off.
fn make_data_dir(data_dir: Option<&[u8]>, name: &[u8]) -> Dstr {
    let mut parsed = copy(data_dir);
    replace_placeholder(&mut parsed, name);
    if end(&parsed) == b'/' {
        drop_last(&mut parsed);
    }
    parsed
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
    let name = name.unwrap_or_default();
    let parsed = make_data_dir(data_dir, name);
    if !exists(fs, &parsed) && has_lib_prefix(name) {
        return make_data_dir(data_dir, name.get(LIB_PREFIX.len()..).unwrap_or_default());
    }
    parsed
}

/// Same contract as C `obs_module_paths_find_file`: `data_path`, one '/'
/// when it has none at the end and `file` is not empty, then `file`;
/// `None` when that does not exist. A NULL `file` is empty.
pub fn find_file(
    fs: &impl ModuleFs,
    data_path: Option<&[u8]>,
    file: Option<&[u8]>,
) -> Option<Vec<u8>> {
    let file = file.unwrap_or_default();
    let mut output = copy(data_path);
    if !is_empty(&output) && end(&output) != b'/' && !file.is_empty() {
        cat_ch(&mut output, b'/');
    }
    cat(&mut output, file);
    if exists(fs, &output) { output } else { None }
}

/// `parse_binary_from_directory`: the binary of the module directory `name`
/// for the binary directory `bin`, `None` when there is none.
fn parse_binary(
    fs: &impl ModuleFs,
    bin: &[u8],
    name: &[u8],
    ext: &[u8],
    add_extension: &mut dyn FnMut(&mut Vec<u8>),
) -> Dstr {
    let mut directory = copy(Some(bin));
    replace_placeholder(&mut directory, name);
    if end(&directory) != b'/' {
        cat_ch(&mut directory, b'/');
    }

    let mut parsed = directory.clone().unwrap_or_default();
    parsed.extend_from_slice(name);
    add_extension(&mut parsed);
    if fs.file_exists(&parsed) {
        return Some(parsed);
    }

    // The C appends ".so" here (its "legacy fallback") and overwrites it
    // right away: the next path tried is lib<name><ext>.
    let mut parsed = directory;
    cat(&mut parsed, LIB_PREFIX);
    cat(&mut parsed, name);
    cat(&mut parsed, ext);
    if exists(fs, &parsed) { parsed } else { None }
}

/// The search of C `obs_module_paths_find` up to `os_glob()`: the pattern,
/// and whether its matches are module directories (`%module%` in `bin`)
/// rather than module files. `None` for an empty binary directory
/// (intentional difference: the C crashes).
pub fn search_pattern(bin: &[u8], ext: &[u8]) -> Option<(Vec<u8>, bool)> {
    let mut search = copy(Some(bin));
    search.as_ref()?;
    let search_directories = match search
        .as_deref()
        .and_then(|s| position(s, MODULE_PLACEHOLDER))
    {
        Some(at) => {
            resize(&mut search, at);
            true
        }
        None => false,
    };
    if !is_empty(&search) && end(&search) != b'/' {
        cat_ch(&mut search, b'/');
    }
    cat_ch(&mut search, b'*');
    if !search_directories {
        cat(&mut search, ext);
    }
    Some((search.unwrap_or_default(), search_directories))
}

/// `process_found_module`.
#[allow(clippy::too_many_arguments)]
fn process_found(
    fs: &impl ModuleFs,
    bin: &[u8],
    data: Option<&[u8]>,
    ext: &[u8],
    add_extension: &mut dyn FnMut(&mut Vec<u8>),
    path: &[u8],
    directory: bool,
    callback: &mut dyn FnMut(&ModuleInfo<'_>),
) {
    let file = path
        .iter()
        .rposition(|&b| b == b'/')
        .and_then(|at| path.get(at.saturating_add(1)..))
        .unwrap_or(path);
    if file == b"." || file == b".." {
        return;
    }

    let mut name = copy(Some(file));
    if let Some(dot) = name
        .as_deref()
        .and_then(|n| n.iter().rposition(|&b| b == b'.'))
    {
        resize(&mut name, dot);
    }

    let (bin_path, bin_found) = if directory {
        let found = parse_binary(
            fs,
            bin,
            name.as_deref().unwrap_or_default(),
            ext,
            add_extension,
        );
        let bin_found = found.is_some();
        (found, bin_found)
    } else {
        (copy(Some(path)), true)
    };

    // The C makes the data path whether or not the binary was found.
    let data_path = self::data_dir(fs, name.as_deref(), data);
    if let (Some(data_path), true) = (data_path, bin_found) {
        callback(&ModuleInfo {
            bin_path: bin_path.as_deref(),
            data_path: &data_path,
            name: name.as_deref(),
        });
    }
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
    let Some(bin) = bin else {
        return;
    };
    let Some((pattern, search_directories)) = search_pattern(bin, ext) else {
        return;
    };
    let Some(matches) = fs.glob(&pattern) else {
        return;
    };
    for entry in matches.iter().filter(|e| e.directory == search_directories) {
        process_found(
            fs,
            bin,
            data,
            ext,
            add_extension,
            &entry.path,
            search_directories,
            callback,
        );
    }
}
