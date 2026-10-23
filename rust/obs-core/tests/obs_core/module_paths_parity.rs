//! Tier 3: the module search path shims against the original C.
//!
//! The oracle is `libobs/obs-module-paths.c.h`, unmodified, with its entry
//! points renamed to `oracle_*` (`obs-c-oracle/oracle/module_paths.c`). Both
//! sides search the same real directory tree, nested `ABOVE_ROOT` levels deep
//! in a new directory in the temporary directory (so that no `..` of a random
//! path reaches a directory another process can change), through the same
//! `os_file_exists()` and `os_glob()`,
//! with the same host hooks and callbacks, which log every call. Compared:
//!
//! - `obs_module_paths_find()`: every module reported (binary path, data
//!   path, name, NULL included, and the type passed through), in order, and
//!   every call of the host's extension hook with the path it got, over
//!   every binary directory × data path × extension × hook below;
//! - `obs_module_paths_data_dir()` and `obs_module_paths_find_file()`: the
//!   result, NULL or the bytes, over names, data paths and files below;
//! - proptests (2048 cases each) over random paths built from the tree's
//!   names, separators and `%module%`.
//!
//! Excluded, the intentional differences, tested on the shims alone: an
//! empty or NULL binary directory, a NULL callback and a NULL hook (the C
//! crashes on each).

use core::ffi::{CStr, c_char, c_int, c_void};
use std::cell::RefCell;
use std::ffi::CString;
use std::mem::{align_of, offset_of, size_of};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use obs_c_oracle::module_paths::{
    OracleAddExtension, OracleModuleInfo2, oracle_obs_module_info2_layout,
    oracle_obs_module_paths_data_dir, oracle_obs_module_paths_find,
    oracle_obs_module_paths_find_file,
};
use obs_core::ffi::module_paths::{
    obs_module_add_extension_t, obs_module_info2, obs_module_paths_data_dir, obs_module_paths_find,
    obs_module_paths_find_file,
};
use obs_util::ffi::bmem::bfree;
use obs_util::ffi::dstr::{dstr, dstr_ncat};
#[cfg(unix)]
use obs_util::ffi::platform_nix::os_file_exists;
#[cfg(windows)]
use obs_util::ffi::platform_windows::os_file_exists;
use proptest::prelude::*;

/// A directory tree of empty files, removed on drop. The entries are under
/// `root`, which is `ABOVE_ROOT` levels of `up/` below `base`, a new directory
/// in the temporary directory: every directory a search reaches by climbing
/// up to `ABOVE_ROOT` levels above `root` is the test's own.
struct Tree {
    base: PathBuf,
    root: PathBuf,
    /// The root as C writes paths: '/' separators.
    prefix: String,
}

static NEXT: AtomicUsize = AtomicUsize::new(0);

impl Tree {
    fn new(entries: &[&str]) -> Self {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let base =
            std::env::temp_dir().join(format!("obs-module-paths-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let root = (0..ABOVE_ROOT).fold(base.clone(), |p, _| p.join("up"));
        std::fs::create_dir_all(&root).unwrap();
        let prefix = root.to_str().unwrap().replace('\\', "/");
        let tree = Self { base, root, prefix };
        for e in entries {
            tree.add(e);
        }
        tree
    }

    fn add(&self, entry: &str) {
        let path = self.root.join(entry.trim_end_matches('/'));
        if entry.ends_with('/') {
            std::fs::create_dir_all(&path).unwrap();
        } else {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, b"").unwrap();
        }
    }

    /// `rel` under the root, as C writes it; `None` stays NULL, and a path
    /// that does not start with '/' is used as given.
    fn at(&self, rel: Option<&str>) -> Option<CString> {
        rel.map(|r| {
            let full = if r.starts_with('/') {
                format!("{}{r}", self.prefix)
            } else {
                r.to_owned()
            };
            CString::new(full).unwrap()
        })
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.base);
    }
}

fn ptr(s: &Option<CString>) -> *const c_char {
    s.as_ref().map_or(core::ptr::null(), |s| s.as_ptr())
}

/// The bytes of a `bmalloc`'d result, freed.
fn take(p: *mut c_char) -> Option<Vec<u8>> {
    if p.is_null() {
        return None;
    }
    // SAFETY: a NUL-terminated `bmalloc`'d string from a shim or the oracle.
    let out = unsafe { CStr::from_ptr(p) }.to_bytes().to_vec();
    // SAFETY: allocated by `bmalloc`.
    unsafe { bfree(p.cast()) };
    Some(out)
}

fn bytes(p: *const c_char) -> Option<Vec<u8>> {
    // SAFETY: NULL or a NUL-terminated string valid for the callback.
    (!p.is_null()).then(|| unsafe { CStr::from_ptr(p) }.to_bytes().to_vec())
}

// ------------------------------------------------- hooks and callbacks

thread_local! {
    /// Every path the host hook got.
    static HOOK_LOG: RefCell<Vec<Vec<u8>>> = const { RefCell::new(Vec::new()) };
}

fn log_hook(d: *mut dstr) {
    // SAFETY: the hook gets a valid `dstr`.
    let got = unsafe { bytes((*d).array) }.unwrap_or_default();
    HOOK_LOG.with(|l| l.borrow_mut().push(got));
}

unsafe extern "C" fn hook_so(d: *mut dstr) {
    log_hook(d);
    // SAFETY: a valid `dstr` and a 3-byte string.
    unsafe { dstr_ncat(d, c".so".as_ptr(), 3) };
}

unsafe extern "C" fn hook_dll(d: *mut dstr) {
    log_hook(d);
    // SAFETY: a valid `dstr` and a 4-byte string.
    unsafe { dstr_ncat(d, c".dll".as_ptr(), 4) };
}

unsafe extern "C" fn hook_none(d: *mut dstr) {
    log_hook(d);
}

/// `obs_host_add_module_extension()` of obs-cocoa.m: ".so" when the bundle
/// path does not exist.
unsafe extern "C" fn hook_cocoa(d: *mut dstr) {
    log_hook(d);
    // SAFETY: a valid `dstr`, NUL-terminated.
    unsafe {
        if !os_file_exists((*d).array) {
            dstr_ncat(d, c".so".as_ptr(), 3);
        }
    }
}

type Hook = unsafe extern "C" fn(*mut dstr);

const HOOKS: [(&str, Hook); 4] = [
    ("so", hook_so),
    ("dll", hook_dll),
    ("none", hook_none),
    ("cocoa", hook_cocoa),
];

fn oracle_hook(hook: Hook) -> OracleAddExtension {
    // SAFETY: the same C ABI; `struct dstr *` is passed as `void *`.
    Some(unsafe { core::mem::transmute::<Hook, unsafe extern "C" fn(*mut c_void)>(hook) })
}

/// One module a callback got.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Record {
    bin: Option<Vec<u8>>,
    data: Option<Vec<u8>>,
    name: Option<Vec<u8>>,
    module_type: c_int,
}

unsafe extern "C" fn record_shim(param: *mut c_void, info: *const obs_module_info2) {
    // SAFETY: `param` is the test's `Vec<Record>`, `info` valid for the call.
    unsafe {
        let info = &*info;
        (*param.cast::<Vec<Record>>()).push(Record {
            bin: bytes(info.bin_path),
            data: bytes(info.data_path),
            name: bytes(info.name),
            module_type: info.module_type,
        });
    }
}

unsafe extern "C" fn record_oracle(param: *mut c_void, info: *const OracleModuleInfo2) {
    // SAFETY: as `record_shim`.
    unsafe {
        let info = &*info;
        (*param.cast::<Vec<Record>>()).push(Record {
            bin: bytes(info.bin_path),
            data: bytes(info.data_path),
            name: bytes(info.name),
            module_type: info.module_type,
        });
    }
}

/// What one search did: the modules and the hook's calls.
type Outcome = (Vec<Record>, Vec<Vec<u8>>);

fn find_shim(
    bin: &Option<CString>,
    data: &Option<CString>,
    ty: c_int,
    ext: &Option<CString>,
    hook: Hook,
) -> Outcome {
    HOOK_LOG.with(|l| l.borrow_mut().clear());
    let mut out: Vec<Record> = Vec::new();
    let hook: obs_module_add_extension_t = Some(hook);
    // SAFETY: valid strings, hook and callback; `out` outlives the call.
    unsafe {
        obs_module_paths_find(
            ptr(bin),
            ptr(data),
            ty,
            ptr(ext),
            hook,
            Some(record_shim),
            (&raw mut out).cast(),
        );
    }
    (out, HOOK_LOG.with(|l| l.take()))
}

fn find_oracle(
    bin: &Option<CString>,
    data: &Option<CString>,
    ty: c_int,
    ext: &Option<CString>,
    hook: Hook,
) -> Outcome {
    HOOK_LOG.with(|l| l.borrow_mut().clear());
    let mut out: Vec<Record> = Vec::new();
    // SAFETY: as `find_shim`.
    unsafe {
        oracle_obs_module_paths_find(
            ptr(bin),
            ptr(data),
            ty,
            ptr(ext),
            oracle_hook(hook),
            Some(record_oracle),
            (&raw mut out).cast(),
        );
    }
    (out, HOOK_LOG.with(|l| l.take()))
}

fn data_dir_both(
    name: &Option<CString>,
    data: &Option<CString>,
) -> (Option<Vec<u8>>, Option<Vec<u8>>) {
    // SAFETY: NULL or NUL-terminated strings.
    unsafe {
        (
            take(obs_module_paths_data_dir(ptr(name), ptr(data))),
            take(oracle_obs_module_paths_data_dir(ptr(name), ptr(data))),
        )
    }
}

fn find_file_both(
    data: &Option<CString>,
    file: &Option<CString>,
) -> (Option<Vec<u8>>, Option<Vec<u8>>) {
    // SAFETY: NULL or NUL-terminated strings.
    unsafe {
        (
            take(obs_module_paths_find_file(ptr(data), ptr(file))),
            take(oracle_obs_module_paths_find_file(ptr(data), ptr(file))),
        )
    }
}

// ------------------------------------------------------------ the tree

const ENTRIES: &[&str] = &[
    "plugins/a.so",
    "plugins/d.so",
    "plugins/libb.so",
    "plugins/libe.so",
    "plugins/libf.so",
    "plugins/LIBg.so",
    "plugins/c.txt",
    "plugins/sub/",
    "plugins/dir.so/",
    "plugins/a.dll",
    "plugins/liba.dll",
    "plugins/noext",
    "plugins/x.y.so",
    "plugins/sp ace.so",
    "plugins/\u{fc}.so",
    "plugins/lib.so",
    "data/a/",
    "data/b/",
    "data/libf/",
    "data/g/",
    "data/stub/locale/en-US.ini",
    "data/stub/sub/",
    "data/stub/%module%/f",
    "mods/x/bin/64bit/x.so",
    "mods/x/bin/64bit/x.dll",
    "mods/x/data/",
    "mods/x/x/x.so",
    "mods/y/bin/64bit/liby.so",
    "mods/y/bin/64bit/liby.dll",
    "mods/z/bin/64bit/",
    "mods/w.so",
    "mods/p.q/bin/64bit/p.so",
    "mods/libl/bin/64bit/libl.so",
    "mods/v/bin/64bit/v.so/",
    "mods/bundle/bin/64bit/bundle/",
    "mods/legacy/bin/64bit/legacy.so",
    "mods/%module%/bin/64bit/%module%.so",
    "mods/n/bin/n/n.so",
    "shared/",
];

#[cfg(target_os = "linux")]
fn tree() -> Tree {
    let tree = Tree::new(ENTRIES);
    // A name that is not UTF-8 (Linux file systems take any bytes).
    use std::os::unix::ffi::OsStrExt;
    let odd = tree
        .root
        .join("plugins")
        .join(std::ffi::OsStr::from_bytes(b"\xff\xfe.so"));
    std::fs::write(odd, b"").unwrap();
    tree
}

#[cfg(not(target_os = "linux"))]
fn tree() -> Tree {
    Tree::new(ENTRIES)
}

const BINS: &[&str] = &[
    "/plugins",
    "/plugins/",
    "/plugins//",
    "/mods/%module%/bin/64bit",
    "/mods/%module%/bin/64bit/",
    "/mods/%module%",
    "/mods%module%",
    "/mods/%module%/%module%",
    "/mods/%module%/bin/%module%",
    "/nowhere",
    "/nowhere/%module%/bin",
    "/plugins/a.so",
    "/",
];

const DATAS: &[Option<&str>] = &[
    None,
    Some(""),
    Some("/"),
    Some("//"),
    Some("/data/%module%"),
    Some("/data/%module%/"),
    Some("/data/%module%//"),
    Some("/data/%module%/%module%"),
    Some("/shared"),
    Some("/shared/"),
    Some("/mods/%module%/data"),
    Some("/data/lib%module%"),
    Some("/data"),
    Some("%module%"),
    Some("%module%/"),
    Some("rel/%module%"),
];

const EXTS: &[Option<&str>] = &[Some(".so"), Some(".dll"), Some(""), None];

const TYPES: &[c_int] = &[0, 1, 2, 3, -1, 7, c_int::MAX];

// ------------------------------------------------------------- the tests

#[test]
fn obs_module_info2_layout_matches_c() {
    // SAFETY: a pure query.
    let c = |which| unsafe { oracle_obs_module_info2_layout(which) };
    assert_eq!(size_of::<obs_module_info2>(), c(0));
    assert_eq!(align_of::<obs_module_info2>(), c(1));
    assert_eq!(offset_of!(obs_module_info2, bin_path), c(2));
    assert_eq!(offset_of!(obs_module_info2, data_path), c(3));
    assert_eq!(offset_of!(obs_module_info2, name), c(4));
    assert_eq!(offset_of!(obs_module_info2, module_type), c(5));
    assert_eq!(size_of::<c_int>(), c(6));
    assert_eq!(size_of::<OracleModuleInfo2>(), c(0));
}

#[test]
fn find_matches_c_over_every_search_path() {
    let tree = tree();
    let mut compared = 0usize;
    let mut reported = 0usize;
    let mut i = 0usize;
    for bin in BINS {
        let bin = tree.at(Some(bin));
        for data in DATAS {
            let data = tree.at(*data);
            for ext in EXTS {
                let ext = ext.map(|e| CString::new(e).unwrap());
                for (hook_name, hook) in HOOKS {
                    let ty = TYPES[i % TYPES.len()];
                    i += 1;
                    let shim = find_shim(&bin, &data, ty, &ext, hook);
                    let oracle = find_oracle(&bin, &data, ty, &ext, hook);
                    assert_eq!(
                        shim, oracle,
                        "bin {bin:?} data {data:?} ext {ext:?} hook {hook_name}"
                    );
                    compared += 1;
                    reported += shim.0.len();
                }
            }
        }
    }
    assert_eq!(
        compared,
        BINS.len() * DATAS.len() * EXTS.len() * HOOKS.len()
    );
    // The tree is not empty for the search: most paths find modules.
    assert!(reported > 2000, "only {reported} modules reported");
}

#[test]
fn find_reports_what_the_c_test_pins() {
    // A sanity anchor for the comparison above: the shims and the oracle
    // agree, and both find the modules test_core_module_load pins.
    let tree = tree();
    let ext = Some(CString::new(".so").unwrap());
    let (found, hooks) = find_shim(
        &tree.at(Some("/mods/%module%/bin/64bit")),
        &tree.at(Some("/mods/%module%/data")),
        2,
        &ext,
        hook_so,
    );
    let names: Vec<_> = found.iter().map(|r| r.name.clone().unwrap()).collect();
    assert!(names.contains(&b"x".to_vec()) && names.contains(&b"libl".to_vec()));
    assert!(found.iter().all(|r| r.module_type == 2));
    assert!(!hooks.is_empty());
}

#[test]
fn data_dir_matches_c() {
    let tree = tree();
    let names: &[Option<&str>] = &[
        None,
        Some(""),
        Some("a"),
        Some("b"),
        Some("g"),
        Some("lib"),
        Some("libb"),
        Some("LIBb"),
        Some("LiBf"),
        Some("libf"),
        Some("liblibf"),
        Some("libstub"),
        Some("stub"),
        Some("li"),
        Some("%module%"),
        Some("x/y"),
        Some(".."),
        Some("a.b"),
        Some("\u{fc}"),
        Some("sp ace"),
    ];
    let mut n = 0;
    for name in names {
        let name = name.map(|s| CString::new(s).unwrap());
        for data in DATAS {
            let data = tree.at(*data);
            let (shim, oracle) = data_dir_both(&name, &data);
            assert_eq!(shim, oracle, "name {name:?} data {data:?}");
            n += 1;
        }
    }
    assert_eq!(n, names.len() * DATAS.len());
}

#[test]
fn find_file_matches_c() {
    let tree = tree();
    let datas: &[Option<&str>] = &[
        None,
        Some(""),
        Some("/"),
        Some("/data/stub"),
        Some("/data/stub/"),
        Some("/data/stub//"),
        Some("/data/stub/locale"),
        Some("/nodata"),
        Some("/data/%module%"),
        Some("/data/stub/%module%"),
    ];
    let files: &[Option<&str>] = &[
        None,
        Some(""),
        Some("locale/en-US.ini"),
        Some("/locale/en-US.ini"),
        Some("locale//en-US.ini"),
        Some("locale/xx-XX.ini"),
        Some("sub"),
        Some("sub/"),
        Some("../stub/locale/en-US.ini"),
        Some("en-US.ini"),
        Some("f"),
        Some("%module%/f"),
        Some("/"),
    ];
    let mut found = 0;
    for data in datas {
        let data = tree.at(*data);
        for file in files {
            let file = file.map(|s| CString::new(s).unwrap());
            let (shim, oracle) = find_file_both(&data, &file);
            assert_eq!(shim, oracle, "data {data:?} file {file:?}");
            found += usize::from(shim.is_some());
        }
    }
    assert!(found >= 10, "only {found} found");
}

// ------------------------------------------- the intentional differences

#[test]
fn empty_or_null_binary_directory_searches_nothing() {
    // The C passes a NULL dstr to strstr() for both: excluded from the
    // comparison, the shims report nothing and do not call the hook.
    let tree = tree();
    let data = tree.at(Some("/data/%module%"));
    let ext = Some(CString::new(".so").unwrap());
    for bin in [None, Some(CString::new("").unwrap())] {
        let (found, hooks) = find_shim(&bin, &data, 2, &ext, hook_so);
        assert!(found.is_empty() && hooks.is_empty());
    }
}

#[test]
fn null_callback_searches_nothing() {
    let tree = tree();
    let bin = tree.at(Some("/mods/%module%/bin/64bit"));
    let data = tree.at(Some("/data/%module%"));
    HOOK_LOG.with(|l| l.borrow_mut().clear());
    // SAFETY: valid strings; a NULL callback (the C would call it).
    unsafe {
        obs_module_paths_find(
            ptr(&bin),
            ptr(&data),
            2,
            c".so".as_ptr(),
            Some(hook_so),
            None,
            core::ptr::null_mut(),
        );
    }
    assert!(HOOK_LOG.with(|l| l.borrow().is_empty()));
}

#[test]
fn null_hook_leaves_the_path() {
    // The C would call NULL. The shim tries <dir>/<name> as it is, then
    // lib<name><ext>: bundle/ (a directory) and liby.so are found, x.so is
    // not (its hook would have added ".so").
    let tree = tree();
    let bin = tree.at(Some("/mods/%module%/bin/64bit"));
    let data = tree.at(Some("/data/%module%"));
    let mut out: Vec<Record> = Vec::new();
    // SAFETY: valid strings and callback; a NULL hook.
    unsafe {
        obs_module_paths_find(
            ptr(&bin),
            ptr(&data),
            2,
            c".so".as_ptr(),
            None,
            Some(record_shim),
            (&raw mut out).cast(),
        );
    }
    let names: Vec<_> = out.iter().map(|r| r.name.clone().unwrap()).collect();
    assert!(names.contains(&b"bundle".to_vec()));
    assert!(names.contains(&b"y".to_vec()));
    assert!(!names.contains(&b"x".to_vec()));
}

// ------------------------------------------------------------ proptests

fn piece() -> impl Strategy<Value = &'static str> {
    prop::sample::select(PIECES)
}

/// Pieces random paths are built from: the tree's names, separators, dots
/// and the placeholder.
const PIECES: &[&str] = &[
    "/",
    "/",
    "//",
    "%module%",
    "%module",
    "module%",
    ".",
    "..",
    "lib",
    "LIB",
    "a",
    "b",
    "f",
    "g",
    "x",
    "y",
    "stub",
    "data",
    "plugins",
    "mods",
    "bin",
    "64bit",
    "locale",
    "en-US.ini",
    ".so",
    ".dll",
    "sub",
    "shared",
    "p.q",
    "\u{fc}",
    " ",
];

/// The most pieces in a random path.
const MAX_PIECES: usize = 9;

/// How many levels above the tree's root a random path can climb. A piece
/// has at most two dots and no piece has both a dot and a '/' (checked by
/// `climbing_above_the_root_stays_in_the_tree`), so each `..` takes at least
/// one piece and the next one a separator piece before it (the first one gets
/// the '/' after the root): k of them take at least 2k - 1 pieces.
const ABOVE_ROOT: usize = MAX_PIECES.div_ceil(2);

fn path() -> impl Strategy<Value = String> {
    prop::collection::vec(piece(), 0..=MAX_PIECES).prop_map(|p| p.concat())
}

/// How many levels above the root `<root>/<rel>` climbs, by its components,
/// at the deepest point: over the path with each `%module%` replaced by a
/// module's name (what the search stats), and over the part before the first
/// `%module%` (the directory the search lists).
fn levels_above_root(rel: &str) -> usize {
    fn climb(p: &str) -> usize {
        let (mut depth, mut lowest) = (0isize, 0isize);
        for c in p.split('/') {
            match c {
                "" | "." => {}
                ".." => {
                    depth -= 1;
                    lowest = lowest.min(depth);
                }
                _ => depth += 1,
            }
        }
        lowest.unsigned_abs()
    }
    let listed = rel.split("%module%").next().unwrap_or_default();
    climb(&rel.replace("%module%", "name")).max(climb(listed))
}

#[test]
fn climbing_above_the_root_stays_in_the_tree() {
    // What ABOVE_ROOT relies on.
    for p in PIECES {
        assert!(p.matches('.').count() <= 2, "{p:?}");
        assert!(!(p.contains('.') && p.contains('/')), "{p:?}");
    }
    assert_eq!(levels_above_root(""), 0);
    assert_eq!(levels_above_root("plugins/../data"), 0);
    assert_eq!(levels_above_root(".."), 1);
    assert_eq!(levels_above_root("../"), 1);
    assert_eq!(levels_above_root("...."), 0);
    assert_eq!(levels_above_root("mods/../../.."), 2);
    // The search lists <root>/.. for this one.
    assert_eq!(levels_above_root("..%module%"), 1);
    assert_eq!(levels_above_root("%module%/../.."), 1);
    // The most a path of MAX_PIECES pieces climbs.
    let most = [".."; ABOVE_ROOT].join("/");
    assert!(most.matches("..").count() + most.matches('/').count() <= MAX_PIECES);
    assert_eq!(levels_above_root(&most), ABOVE_ROOT);
    // That many levels up is the tree's own directory, and holds only the
    // way back down.
    let tree = tree();
    let top = (0..ABOVE_ROOT).fold(tree.root.clone(), |p, _| p.join(".."));
    assert_eq!(
        top.canonicalize().unwrap(),
        tree.base.canonicalize().unwrap()
    );
    let mut dir = tree.base.clone();
    for _ in 0..ABOVE_ROOT {
        let names: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, ["up"]);
        dir.push("up");
    }
    assert_eq!(dir, tree.root);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2048))]

    #[test]
    fn data_dir_matches_c_for_any_path(name in path(), data in path(), rooted in any::<bool>()) {
        let tree = TREE.with(|t| t.prefix.clone());
        let data = if rooted { format!("{tree}/{data}") } else { data };
        let name = Some(CString::new(name).unwrap());
        let data = Some(CString::new(data).unwrap());
        let (shim, oracle) = data_dir_both(&name, &data);
        prop_assert_eq!(shim, oracle);
    }

    #[test]
    fn find_file_matches_c_for_any_path(data in path(), file in path(), rooted in any::<bool>()) {
        let tree = TREE.with(|t| t.prefix.clone());
        let data = if rooted { format!("{tree}/{data}") } else { data };
        let data = Some(CString::new(data).unwrap());
        let file = Some(CString::new(file).unwrap());
        let (shim, oracle) = find_file_both(&data, &file);
        prop_assert_eq!(shim, oracle);
    }

    #[test]
    fn find_matches_c_for_any_path(bin in path(), data in path(), ext in 0..EXTS.len(), hook in 0..HOOKS.len()) {
        // Rooted only: a relative search would glob the working directory.
        // Neither path leaves the tree (a search of the temporary directory
        // would race other processes).
        prop_assert!(levels_above_root(&bin) <= ABOVE_ROOT, "bin {bin:?}");
        prop_assert!(levels_above_root(&data) <= ABOVE_ROOT, "data {data:?}");
        let tree = TREE.with(|t| t.prefix.clone());
        let bin = Some(CString::new(format!("{tree}/{bin}")).unwrap());
        let data = Some(CString::new(format!("{tree}/{data}")).unwrap());
        let ext = EXTS[ext].map(|e| CString::new(e).unwrap());
        let (_, hook) = HOOKS[hook];
        prop_assert_eq!(find_shim(&bin, &data, 3, &ext, hook), find_oracle(&bin, &data, 3, &ext, hook));
    }
}

thread_local! {
    /// One tree per proptest thread.
    static TREE: Tree = tree();
}
