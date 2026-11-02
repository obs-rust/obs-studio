//! Safe core of `libobs/obs-windows-symbol-paths.c.h`: the string rules of
//! `libobs/obs-windows.c.h`'s `reset_win32_symbol_paths()`, which sets
//! DbgHelp's symbol search path after modules load.
//!
//! - [`cut`] and [`dir`]: the directory of a module's binary, its path with
//!   every `/` turned into `\`, cut at the last `\`;
//! - [`listed`]: whether a directory is listed already, by `astrcmpi()`;
//! - [`join_pieces`] and [`join`]: the search path, the listed directories
//!   joined with `;`.
//!
//! The C keeps the DbgHelp loading, `os_get_abs_path_ptr()` for `"."` and for
//! each directory, the push or free of each entry, the wide conversion,
//! `SymInitializeW()` or `SymSetSearchPathW()` and `sym_initialize_called`,
//! each at its point, in the same order (the working directory first, then
//! the modules in load order).
//!
//! The core is OS-free: it takes the C's bytes.
//!
//! Characterized, not endorsed (`test_windows_symbol_paths_steps`), and
//! kept: the C's copy keeps the whole path's length after the cut; a
//! trailing `\` cuts to the path without it; a root file gives the drive
//! alone (`C:`); `astrcmpi()` folds ASCII only, so a directory that differs
//! in a non-ASCII letter's case is listed twice; the join's `;` goes by
//! index, so an empty first entry leaves a leading `;`.
//!
//! One intentional difference from the C: a NULL or empty binary path gives
//! no directory, so the module is skipped, as one with no `\` is. The C
//! calls `strrchr()` on the NULL array `dstr_copy()` leaves for it. No module
//! reaches it today: `obs-module.c` sets `bin_path` only after the module's
//! library opened.

use obs_util::dstr::astrcmpi;

/// The separator `dstr_replace()` writes and `strrchr()` looks for.
const SEPARATOR: u8 = b'\\';

/// A module's binary path as the C cuts it: its copy with every `/` turned
/// into `\`, and where the directory ends in it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cut {
    /// The copy, whole: the C writes its NUL at `end` in place, so the
    /// copy's length stays the path's.
    pub copy: Vec<u8>,
    /// The index of the last `\`, or `None`: the path has no directory.
    pub end: Option<usize>,
}

impl Cut {
    /// The directory: the copy up to its last `\`, if it has one.
    pub fn dir(&self) -> Option<&[u8]> {
        self.end.map(|end| &self.copy[..end])
    }
}

/// The C's cut of a non-empty binary path (a C string's bytes, so no NUL):
/// `dstr_replace(dir, "/", "\\")`, then `strrchr(dir->array, '\\')`.
pub fn cut(bin_path: &[u8]) -> Cut {
    let copy: Vec<u8> = bin_path
        .iter()
        .map(|&b| if b == b'/' { SEPARATOR } else { b })
        .collect();
    let end = copy.iter().rposition(|&b| b == SEPARATOR);
    Cut { copy, end }
}

/// The directory of a module's binary, or `None` when the module adds no
/// directory: no `\` in its path, or (the intentional difference) a NULL or
/// empty path.
pub fn dir(bin_path: Option<&[u8]>) -> Option<Vec<u8>> {
    let path = bin_path.filter(|path| !path.is_empty())?;
    cut(path).dir().map(<[u8]>::to_vec)
}

/// Whether `abspath` is one of `paths` already, compared by `astrcmpi()`,
/// which reads `None` as `""` and folds ASCII case only. The paths are read
/// in order up to the first match, as the C's loop reads them.
pub fn listed<'a>(
    paths: impl IntoIterator<Item = Option<&'a [u8]>>,
    abspath: Option<&[u8]>,
) -> bool {
    paths
        .into_iter()
        .any(|existing| astrcmpi(abspath, existing) == 0)
}

/// What the join appends, in the C's pieces and order: each path but `None`
/// and empty ones, after a `;` unless it is at index 0. The C appends each
/// piece with its own `dstr_cat()`, which decides how the dstr grows, so a
/// shim appends them one by one too.
pub fn join_pieces<'a>(
    paths: impl IntoIterator<Item = Option<&'a [u8]>>,
) -> impl Iterator<Item = &'a [u8]> {
    paths
        .into_iter()
        .enumerate()
        .filter_map(|(i, path)| path.filter(|path| !path.is_empty()).map(|path| (i, path)))
        .flat_map(|(i, path)| {
            [(i != 0).then_some(&b";"[..]), Some(path)]
                .into_iter()
                .flatten()
        })
}

/// What the join appends, whole: [`join_pieces`] one after another. Empty
/// when nothing is appended.
pub fn join<'a>(paths: impl IntoIterator<Item = Option<&'a [u8]>>) -> Vec<u8> {
    join_pieces(paths).flatten().copied().collect()
}
