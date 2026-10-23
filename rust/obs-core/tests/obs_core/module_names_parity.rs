//! Tier 3: the module name and file shims against the original C.
//!
//! The oracle is `libobs/obs-module-names.c.h`, unmodified, with its entry
//! points renamed to `oracle_*` (`obs-c-oracle/oracle/module_names.c`); its
//! core module lookup makes data paths with the oracle's own
//! `obs_module_paths_data_dir()`. Both sides look at the same real
//! directory tree, under a new directory in the temporary directory,
//! through the same `os_file_exists()`, with the same callback, which logs
//! every call. Compared:
//!
//! - `obs_module_names_file_name()`: the offset into the path;
//! - `obs_module_names_module_name()`, `_config_path()` and
//!   `_manifest_path()`: the result, NULL or the bytes;
//! - `obs_module_names_listed()`: the result;
//! - `obs_module_names_find_core()`: the result and every module reported
//!   (binary path, data path, name, NULL included, and the type passed
//!   through), over every binary × name × data path × extension below;
//! - proptests (2048 cases each) over random paths and names built from
//!   the tree's names, separators, dots and `%module%`.
//!
//! Excluded, the intentional differences, tested on the shims alone: a file
//! name shorter than the extension (the C's length underflows), a NULL path
//! or extension, a NULL name or list entry, and a NULL callback (the C
//! dereferences or calls each).

use core::ffi::{CStr, c_char, c_int, c_void};
use std::ffi::CString;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use obs_c_oracle::module_names::{
    oracle_obs_module_names_config_path, oracle_obs_module_names_file_name,
    oracle_obs_module_names_find_core, oracle_obs_module_names_listed,
    oracle_obs_module_names_manifest_path, oracle_obs_module_names_module_name,
};
use obs_c_oracle::module_paths::OracleModuleInfo2;
use obs_core::ffi::module_names::{
    obs_module_names_config_path, obs_module_names_file_name, obs_module_names_find_core,
    obs_module_names_listed, obs_module_names_manifest_path, obs_module_names_module_name,
};
use obs_core::ffi::module_paths::obs_module_info2;
use obs_util::ffi::bmem::bfree;
use proptest::prelude::*;

/// A directory tree of empty files, removed on drop.
struct Tree {
    root: PathBuf,
    /// The root as C writes paths: '/' separators.
    prefix: String,
}

static NEXT: AtomicUsize = AtomicUsize::new(0);

impl Tree {
    fn new(entries: &[&str]) -> Self {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let root =
            std::env::temp_dir().join(format!("obs-module-names-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let prefix = root.to_str().unwrap().replace('\\', "/");
        for e in entries {
            let path = root.join(e.trim_end_matches('/'));
            if e.ends_with('/') {
                std::fs::create_dir_all(&path).unwrap();
            } else {
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                std::fs::write(&path, b"").unwrap();
            }
        }
        Self { root, prefix }
    }

    /// `rel` under the root when it starts with '/', else as given; `None`
    /// stays NULL.
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
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn c(s: Option<&str>) -> Option<CString> {
    s.map(|s| CString::new(s).unwrap())
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

// ------------------------------------------------------ both sides

fn file_name_both(path: &CString) -> (usize, usize) {
    let base = path.as_ptr();
    // SAFETY: a NUL-terminated path; both return a pointer into it.
    unsafe {
        (
            obs_module_names_file_name(base).offset_from(base) as usize,
            oracle_obs_module_names_file_name(base).offset_from(base) as usize,
        )
    }
}

type Both = (Option<Vec<u8>>, Option<Vec<u8>>);

fn module_name_both(file: &Option<CString>, ext: &CString) -> Both {
    // SAFETY: NULL or NUL-terminated strings.
    unsafe {
        (
            take(obs_module_names_module_name(ptr(file), ext.as_ptr())),
            take(oracle_obs_module_names_module_name(ptr(file), ext.as_ptr())),
        )
    }
}

fn config_path_both(
    config: &Option<CString>,
    name: &Option<CString>,
    file: &Option<CString>,
) -> Both {
    // SAFETY: NULL or NUL-terminated strings.
    unsafe {
        (
            take(obs_module_names_config_path(
                ptr(config),
                ptr(name),
                ptr(file),
            )),
            take(oracle_obs_module_names_config_path(
                ptr(config),
                ptr(name),
                ptr(file),
            )),
        )
    }
}

fn manifest_path_both(data: &Option<CString>) -> Both {
    // SAFETY: NULL or NUL-terminated strings.
    unsafe {
        (
            take(obs_module_names_manifest_path(ptr(data))),
            take(oracle_obs_module_names_manifest_path(ptr(data))),
        )
    }
}

fn listed_both(names: &[CString], name: &CString) -> (bool, bool) {
    let mut ptrs: Vec<*mut c_char> = names.iter().map(|n| n.as_ptr().cast_mut()).collect();
    let list = if ptrs.is_empty() {
        // The C reads nothing for a count of 0; a NULL array is never
        // passed by obs-module.c (a DARRAY's array is NULL when empty).
        core::ptr::null()
    } else {
        ptrs.as_mut_ptr().cast_const()
    };
    // SAFETY: `count` valid strings, a valid name.
    unsafe {
        (
            obs_module_names_listed(list, ptrs.len(), name.as_ptr()),
            oracle_obs_module_names_listed(list, ptrs.len(), name.as_ptr()),
        )
    }
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

type Outcome = (bool, Vec<Record>);

fn find_core_both(
    binary: &Option<CString>,
    name: &Option<CString>,
    data: &Option<CString>,
    ty: c_int,
    ext: &CString,
) -> (Outcome, Outcome) {
    let mut shim: Vec<Record> = Vec::new();
    let mut oracle: Vec<Record> = Vec::new();
    // SAFETY: valid strings and callbacks; the vectors outlive the calls.
    unsafe {
        let s = obs_module_names_find_core(
            ptr(binary),
            ptr(name),
            ptr(data),
            ty,
            ext.as_ptr(),
            Some(record_shim),
            (&raw mut shim).cast(),
        );
        let o = oracle_obs_module_names_find_core(
            ptr(binary),
            ptr(name),
            ptr(data),
            ty,
            ext.as_ptr(),
            Some(record_oracle),
            (&raw mut oracle).cast(),
        );
        ((s, shim), (o, oracle))
    }
}

// ------------------------------------------------------------ inputs

const ENTRIES: &[&str] = &[
    "core/obs-ffmpeg.so",
    "core/obs-ffmpeg.dll",
    "core/linux-capture.so",
    "core/libobs-x.so",
    "core/bundle/",
    "core/a.b.so",
    "core/\u{fc}.so",
    "core/sp ace.so",
    "data/obs-ffmpeg/",
    "data/obs-x/",
    "data/libobs-x/manifest.json",
    "data/linux-capture/locale/",
    ".so",
];

const PATHS: &[&str] = &[
    "",
    "/",
    "//",
    "a",
    "a/",
    "/a",
    "/R/n/stub.so",
    "/R//n//two.so",
    "plain.so",
    "C:\\x\\y.dll",
    "a/b\\c",
    "/R/n/d.so/x.so",
    "/R/n/.so",
    "\u{fc}/\u{fc}.so",
];

const FILES: &[Option<&str>] = &[
    None,
    Some(""),
    Some("stub.so"),
    Some("a.b.so"),
    Some("LIBZ.SO"),
    Some("noext"),
    Some("x.dll"),
    Some("abcd"),
    Some("abc"),
    Some(".so"),
    Some(".dll"),
    Some("lib.so"),
    Some("\u{fc}.so"),
    Some("sp ace.so"),
    Some("core_module_stub.so"),
];

const EXTS: &[&str] = &[".so", ".dll", ""];

const CONFIGS: &[Option<&str>] = &[
    None,
    Some(""),
    Some("/"),
    Some("//"),
    Some("/R/cfg"),
    Some("/R/cfg/"),
    Some("/R/cfg//"),
    Some("rel"),
    Some("c\\"),
];

const NAMES: &[Option<&str>] = &[
    None,
    Some(""),
    Some("stub"),
    Some("m/"),
    Some("/m"),
    Some("lib"),
    Some("libobs-x"),
    Some("obs-ffmpeg"),
    Some("obs-x"),
    Some("%module%"),
    Some("\u{fc}"),
];

const CONFIG_FILES: &[Option<&str>] = &[
    None,
    Some(""),
    Some("f.json"),
    Some("a/b.ini"),
    Some("/abs"),
    Some("f/"),
];

const BINARIES: &[Option<&str>] = &[
    None,
    Some(""),
    Some("/core/obs-ffmpeg"),
    Some("/core/linux-capture"),
    Some("/core/libobs-x"),
    Some("/core/bundle"),
    Some("/core/a.b"),
    Some("/core/missing"),
    Some("/core/"),
    Some("/core/\u{fc}"),
    Some("/core/sp ace"),
    Some("/nowhere/obs-ffmpeg"),
];

const DATAS: &[Option<&str>] = &[
    None,
    Some(""),
    Some("/"),
    Some("/data/obs-ffmpeg"),
    Some("/data/obs-ffmpeg/"),
    Some("/data/%module%"),
    Some("/data/%module%/"),
    Some("/data/%module%//"),
    Some("/data/lib%module%"),
    Some("/nodata/%module%"),
    Some("%module%"),
];

const TYPES: &[c_int] = &[0, 1, 2, 3, -1, 7, c_int::MAX];

fn tree() -> Tree {
    Tree::new(ENTRIES)
}

// ------------------------------------------------------------- the tests

#[test]
fn file_name_matches_c() {
    for path in PATHS {
        let path = CString::new(*path).unwrap();
        let (shim, oracle) = file_name_both(&path);
        assert_eq!(shim, oracle, "path {path:?}");
    }
}

#[test]
fn module_name_matches_c() {
    let mut n = 0;
    for file in FILES {
        for ext in EXTS {
            // Excluded: shorter than the extension (intentional difference).
            if file.map_or(0, str::len) < ext.len() {
                continue;
            }
            let (f, e) = (c(*file), CString::new(*ext).unwrap());
            let (shim, oracle) = module_name_both(&f, &e);
            assert_eq!(shim, oracle, "file {file:?} ext {ext:?}");
            n += 1;
        }
    }
    assert!(n > 30, "only {n} compared");
}

#[test]
fn config_path_matches_c() {
    let mut n = 0;
    for config in CONFIGS {
        for name in NAMES {
            for file in CONFIG_FILES {
                let (shim, oracle) = config_path_both(&c(*config), &c(*name), &c(*file));
                assert_eq!(
                    shim, oracle,
                    "config {config:?} name {name:?} file {file:?}"
                );
                n += 1;
            }
        }
    }
    assert_eq!(n, CONFIGS.len() * NAMES.len() * CONFIG_FILES.len());
}

#[test]
fn manifest_path_matches_c() {
    for data in CONFIGS.iter().chain(DATAS) {
        let (shim, oracle) = manifest_path_both(&c(*data));
        assert_eq!(shim, oracle, "data {data:?}");
    }
}

#[test]
fn listed_matches_c() {
    let lists: &[&[&str]] = &[
        &[],
        &["a"],
        &["a", "bb"],
        &["A", "b ", ""],
        &["B", "bbb", "a.so", ""],
        &["bb", "bb"],
        &["\u{fc}"],
    ];
    let names = ["a", "b", "bb", "", "A", "a.so", "\u{fc}", "bbb"];
    for list in lists {
        let list: Vec<CString> = list.iter().map(|n| CString::new(*n).unwrap()).collect();
        for name in names {
            let name = CString::new(name).unwrap();
            let (shim, oracle) = listed_both(&list, &name);
            assert_eq!(shim, oracle, "list {list:?} name {name:?}");
        }
    }
}

#[test]
fn find_core_matches_c_over_every_core_module() {
    let tree = tree();
    let mut compared = 0usize;
    let mut found = 0usize;
    let mut i = 0usize;
    for binary in BINARIES {
        let binary = tree.at(*binary);
        for name in NAMES {
            let name = c(*name);
            for data in DATAS {
                let data = tree.at(*data);
                for ext in EXTS {
                    let ext = CString::new(*ext).unwrap();
                    let ty = TYPES[i % TYPES.len()];
                    i += 1;
                    let (shim, oracle) = find_core_both(&binary, &name, &data, ty, &ext);
                    assert_eq!(
                        shim, oracle,
                        "binary {binary:?} name {name:?} data {data:?} ext {ext:?}"
                    );
                    compared += 1;
                    found += usize::from(shim.0);
                }
            }
        }
    }
    assert_eq!(
        compared,
        BINARIES.len() * NAMES.len() * DATAS.len() * EXTS.len()
    );
    // A good share of the lookups must find a binary, or the tree is broken.
    // The share depends on the host's extensions (968 of 4356 on Windows, more
    // on Linux), so the check is a fraction rather than a fixed count.
    assert!(found * 5 > compared, "only {found} of {compared} found");
}

// ------------------------------------------- the intentional differences

#[test]
fn shorter_than_the_extension_has_no_name() {
    for (file, ext) in [("ab", ".so"), ("a", ".so"), ("", ".so"), ("a.so", ".dll")] {
        let (file, ext) = (c(Some(file)), CString::new(ext).unwrap());
        // SAFETY: NUL-terminated strings; the oracle is not called.
        assert_eq!(
            take(unsafe { obs_module_names_module_name(ptr(&file), ext.as_ptr()) }),
            None
        );
    }
    // A NULL file is NULL for any extension.
    // SAFETY: as above.
    assert_eq!(
        take(unsafe { obs_module_names_module_name(core::ptr::null(), c".so".as_ptr()) }),
        None
    );
}

#[test]
fn null_path_and_extension() {
    // SAFETY: NULL arguments the C would dereference.
    unsafe {
        assert!(obs_module_names_file_name(core::ptr::null()).is_null());
        assert_eq!(
            take(obs_module_names_module_name(
                c"stub.so".as_ptr(),
                core::ptr::null()
            )),
            Some(b"stub.so".to_vec())
        );
    }
}

#[test]
fn null_names_are_on_no_list() {
    let a = CString::new("a").unwrap();
    let mut list = [core::ptr::null_mut(), a.as_ptr().cast_mut()];
    // SAFETY: NULL entries and names the C would pass to strcmp().
    unsafe {
        assert!(obs_module_names_listed(list.as_mut_ptr(), 2, a.as_ptr()));
        assert!(!obs_module_names_listed(
            list.as_mut_ptr(),
            2,
            core::ptr::null()
        ));
        assert!(!obs_module_names_listed(list.as_mut_ptr(), 1, a.as_ptr()));
        assert!(!obs_module_names_listed(core::ptr::null(), 3, a.as_ptr()));
    }
}

#[test]
fn null_callback_finds_nothing() {
    let tree = tree();
    let binary = tree.at(Some("/core/obs-ffmpeg"));
    let data = tree.at(Some("/data/%module%"));
    // SAFETY: valid strings; a NULL callback (the C would call it).
    let found = unsafe {
        obs_module_names_find_core(
            ptr(&binary),
            c"obs-ffmpeg".as_ptr(),
            ptr(&data),
            0,
            c".so".as_ptr(),
            None,
            core::ptr::null_mut(),
        )
    };
    assert!(!found);
}

// ------------------------------------------------------------ proptests

fn piece() -> impl Strategy<Value = &'static str> {
    prop::sample::select(vec![
        "/",
        "/",
        "//",
        "%module%",
        ".",
        "..",
        "lib",
        "LIB",
        "a",
        "b",
        ".so",
        ".dll",
        "so",
        "core",
        "data",
        "obs-ffmpeg",
        "obs-x",
        "libobs-x",
        "bundle",
        "\\",
        "\u{fc}",
        " ",
    ])
}

fn text() -> impl Strategy<Value = String> {
    prop::collection::vec(piece(), 0..8).prop_map(|p| p.concat())
}

fn maybe() -> impl Strategy<Value = Option<String>> {
    prop_oneof![1 => Just(None), 6 => text().prop_map(Some)]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2048))]

    #[test]
    fn file_name_matches_c_for_any_path(path in text()) {
        let path = CString::new(path).unwrap();
        let (shim, oracle) = file_name_both(&path);
        prop_assert_eq!(shim, oracle);
    }

    #[test]
    fn module_name_matches_c_for_any_file(file in maybe(), ext in prop::sample::select(EXTS.to_vec())) {
        prop_assume!(file.as_ref().map_or(0, String::len) >= ext.len());
        let (shim, oracle) = module_name_both(&c(file.as_deref()), &CString::new(ext).unwrap());
        prop_assert_eq!(shim, oracle);
    }

    #[test]
    fn config_path_matches_c_for_any_parts(config in maybe(), name in maybe(), file in maybe()) {
        let (shim, oracle) = config_path_both(&c(config.as_deref()), &c(name.as_deref()), &c(file.as_deref()));
        prop_assert_eq!(shim, oracle);
    }

    #[test]
    fn manifest_path_matches_c_for_any_path(data in maybe()) {
        let (shim, oracle) = manifest_path_both(&c(data.as_deref()));
        prop_assert_eq!(shim, oracle);
    }

    #[test]
    fn listed_matches_c_for_any_list(list in prop::collection::vec(text(), 0..5), name in text()) {
        let list: Vec<CString> = list.into_iter().map(|n| CString::new(n).unwrap()).collect();
        let (shim, oracle) = listed_both(&list, &CString::new(name).unwrap());
        prop_assert_eq!(shim, oracle);
    }

    #[test]
    fn find_core_matches_c_for_any_path(
        binary in text(),
        name in maybe(),
        data in text(),
        ext in prop::sample::select(EXTS.to_vec()),
    ) {
        // Rooted: a relative path would look in the working directory.
        let tree = TREE.with(|t| t.prefix.clone());
        let binary = Some(CString::new(format!("{tree}/{binary}")).unwrap());
        let data = Some(CString::new(format!("{tree}/{data}")).unwrap());
        let (shim, oracle) = find_core_both(&binary, &c(name.as_deref()), &data, 0, &CString::new(ext).unwrap());
        prop_assert_eq!(shim, oracle);
    }
}

thread_local! {
    /// One tree per proptest thread.
    static TREE: Tree = tree();
}
