//! Safe core of `libobs/obs-module-names.c.h`: module names and files.
//!
//! How a module is named from its binary ([`file_name`], [`module_name`]),
//! where its config file ([`config_path`]) and its manifest
//! ([`manifest_path`]) are, whether a name is on the safe or disabled module
//! list ([`listed`]), and how a core module is found ([`find_core`]).
//! `obs-module.c` keeps the module lists, the config path, `struct
//! obs_module` and the loading, and passes their fields in.
//!
//! Paths and names are bytes, not text: they come from the file system and
//! the configuration, so they can hold anything but NUL. `None` is a NULL
//! string, and the C's `struct dstr` steps are followed exactly, NULL
//! against empty included: a copy of an empty string is NULL, and a name cut
//! to nothing is NULL. The file system is the caller's [`ModuleFs`]. Nothing
//! here panics, indexes or overflows for any input.
//!
//! Kept as the C has it, characterized and not endorsed (#1876):
//!
//! - a module name is its file name without the length of the extension at
//!   its end, whatever those bytes are ("noext" is "no" for ".so");
//! - a file name exactly as long as the extension (".so") has no name;
//! - [`config_path`] and [`manifest_path`] check only the last byte for
//!   '/', and an empty or NULL directory makes them relative;
//! - [`find_core`] reports the data path whether or not it exists.
//!
//! Intentional difference: a file name shorter than the extension has no
//! name. The C's length underflows there and the process crashes.

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
use crate::module_paths::{self, Dstr, ModuleFs, cat, cat_ch, copy, end, exists, is_empty, resize};

/// The manifest file in a module's data path.
pub const MANIFEST_FILE: &[u8] = b"manifest.json";

/// The core module [`find_core`] reports, as `struct obs_module_info2`
/// without the type (the caller's). Valid for the callback only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoreModule<'a> {
    pub bin_path: &'a [u8],
    /// `None` when the data path is NULL (an empty or NULL `data`, or
    /// `"/"`).
    pub data_path: Option<&'a [u8]>,
    pub name: Option<&'a [u8]>,
}

/// Same contract as C `obs_module_names_file_name`: the file name of a
/// binary path, the bytes after its last '/', or the whole path without
/// one.
pub fn file_name(path: &[u8]) -> &[u8] {
    path.iter()
        .rposition(|&b| b == b'/')
        .and_then(|at| path.get(at.saturating_add(1)..))
        .unwrap_or(path)
}

/// Same contract as C `obs_module_names_module_name`: the file name
/// without the length of `ext` at its end. `None` when that leaves
/// nothing, or when the file name is shorter than `ext` (intentional
/// difference: the C underflows).
pub fn module_name(file: Option<&[u8]>, ext: &[u8]) -> Option<Vec<u8>> {
    let mut name = copy(file);
    let len = name.as_ref().map_or(0, Vec::len);
    // The C's `name.len - ext_len` underflows when the name is shorter:
    // cut to nothing instead.
    resize(&mut name, len.saturating_sub(ext.len()));
    name
}

/// Same contract as C `obs_module_names_config_path`: `config_path`, one
/// '/' when it is not empty and has none at the end, `module_name`, '/',
/// `file`.
pub fn config_path(
    config_path: Option<&[u8]>,
    module_name: Option<&[u8]>,
    file: Option<&[u8]>,
) -> Vec<u8> {
    let mut output = copy(config_path);
    if !is_empty(&output) && end(&output) != b'/' {
        cat_ch(&mut output, b'/');
    }
    cat(&mut output, module_name.unwrap_or_default());
    cat_ch(&mut output, b'/');
    cat(&mut output, file.unwrap_or_default());
    output.unwrap_or_default()
}

/// Same contract as C `obs_module_names_manifest_path`: `data_path`, one
/// '/' when it is not empty and has none at the end, `manifest.json`.
pub fn manifest_path(data_path: Option<&[u8]>) -> Vec<u8> {
    let mut path = copy(data_path);
    if !is_empty(&path) && end(&path) != b'/' {
        cat_ch(&mut path, b'/');
    }
    cat(&mut path, MANIFEST_FILE);
    path.unwrap_or_default()
}

/// Same contract as C `obs_module_names_listed`: `name` is one of `names`,
/// byte for byte.
pub fn listed<'a>(names: impl IntoIterator<Item = &'a [u8]>, name: &[u8]) -> bool {
    names.into_iter().any(|n| n == name)
}

/// Same contract as C `obs_module_names_find_core`: when `binary` plus
/// `ext` exists, calls `callback` with that path, the data path of
/// [`crate::module_paths::data_dir`] for `name` and `data` (made first,
/// whether or not the binary exists, and reported whether or not it
/// exists) and `name`, and returns true.
pub fn find_core(
    fs: &impl ModuleFs,
    binary: Option<&[u8]>,
    name: Option<&[u8]>,
    data: Option<&[u8]>,
    ext: &[u8],
    callback: &mut dyn FnMut(&CoreModule<'_>),
) -> bool {
    let mut module_path: Dstr = copy(binary);
    cat(&mut module_path, ext);

    let data_path = module_paths::data_dir(fs, name, data);

    match module_path.as_deref() {
        Some(bin_path) if exists(fs, &module_path) => {
            callback(&CoreModule {
                bin_path,
                data_path: data_path.as_deref(),
                name,
            });
            true
        }
        _ => false,
    }
}
