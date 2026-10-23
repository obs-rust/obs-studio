//! Tier 3: the module locale file shims against the original C.
//!
//! The oracle is `libobs/obs-module-locale.c.h`, unmodified, with its entry
//! points renamed to `oracle_*` (`obs-c-oracle/oracle/module_locale.c`); its
//! `astrcmpi` is the original `util/dstr.c` (the dstr oracle's). Compared:
//!
//! - `obs_module_locale_default_file()` and `obs_module_locale_file()`: the
//!   name, for every locale below (NULL, empty, case, bytes outside ASCII,
//!   '/', "..", `%module%`) and in proptests over any bytes;
//! - `obs_module_locale_is_default()`: the result, for every pair of those
//!   locales and in proptests;
//! - what `obs_module_load_locale()` reads: each side's names, its own
//!   "same locale" decision, and its own `obs_module_paths_find_file()` (the
//!   shim and the oracle's), on one real directory tree, for every data path
//!   × default locale × locale below and in a proptest. Both sides resolve
//!   the second name's double '/' through the same `os_file_exists()`.
//!
//! The tree is a new directory in the temporary directory, removed on drop.
//! The data paths are all inside it, and the locales of the lookups have no
//! '/', so each name is one component below the data path and nothing is
//! looked up outside the tree (#1917). Locales with '/' are compared as
//! names only.
//!
//! No intentional differences.

use core::ffi::{CStr, c_char};
use std::ffi::CString;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use obs_c_oracle::module_locale::{
    oracle_obs_module_locale_default_file, oracle_obs_module_locale_file,
    oracle_obs_module_locale_is_default,
};
use obs_c_oracle::module_paths::oracle_obs_module_paths_find_file;
use obs_core::ffi::module_locale::{
    obs_module_locale_default_file, obs_module_locale_file, obs_module_locale_is_default,
};
use obs_core::ffi::module_paths::obs_module_paths_find_file;
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
            std::env::temp_dir().join(format!("obs-module-locale-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let prefix = root.to_str().unwrap().replace('\\', "/");
        for e in entries {
            let path = root.join(e.trim_start_matches('/'));
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, b"").unwrap();
        }
        Self { root, prefix }
    }

    /// `rel` (which starts with '/') under the root.
    fn at(&self, rel: &str) -> CString {
        CString::new(format!("{}{rel}", self.prefix)).unwrap()
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// The files of the tree. "...ini" is the file of the locale "..", and
/// ".ini" the file of the empty locale.
const ENTRIES: &[&str] = &[
    "/d/locale/en-US.ini",
    "/d/locale/de-DE.ini",
    "/d/locale/EN-us.ini",
    "/d/locale/zh-Hans.ini",
    "/d/locale/.ini",
    "/d/locale/...ini",
    "/d/locale/%module%.ini",
    "/nodef/locale/de-DE.ini",
    "/nodef/locale/fr-FR.ini",
];

/// Data paths, all inside the tree.
const DATA: &[&str] = &["/d", "/d/", "/d//", "/nodef", "/none", "/d/locale"];

/// Locales of the lookups: no '/'.
const LOOKUP_LOCALES: &[Option<&str>] = &[
    None,
    Some(""),
    Some("en-US"),
    Some("EN-us"),
    Some("en-us"),
    Some("de-DE"),
    Some("DE-de"),
    Some("fr-FR"),
    Some("zh-Hans"),
    Some("en"),
    Some(".."),
    Some("%module%"),
];

/// Locales compared as names and as pairs, '/' and odd bytes included.
fn name_locales() -> Vec<Option<Vec<u8>>> {
    let mut out: Vec<Option<Vec<u8>>> = LOOKUP_LOCALES
        .iter()
        .map(|l| l.map(|l| l.as_bytes().to_vec()))
        .collect();
    for l in [
        &b"en-US "[..],
        b"en_US",
        b"../x",
        b"/abs",
        b"a/b",
        b"x.ini",
        b"\\",
        b"@",
        b"`",
        b"[",
        b"{",
        b"\xff",
        b"\x80",
        "é".as_bytes(),
        "É".as_bytes(),
        b"\x7f",
        b"\x01",
    ] {
        out.push(Some(l.to_vec()));
    }
    out
}

fn c(s: Option<&[u8]>) -> Option<CString> {
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

// ------------------------------------------------------ both sides

type Both = (Option<Vec<u8>>, Option<Vec<u8>>);

fn default_file_both(l: &Option<CString>) -> Both {
    // SAFETY: NULL or a NUL-terminated string.
    unsafe {
        (
            take(obs_module_locale_default_file(ptr(l))),
            take(oracle_obs_module_locale_default_file(ptr(l))),
        )
    }
}

fn locale_file_both(l: &Option<CString>) -> Both {
    // SAFETY: NULL or a NUL-terminated string.
    unsafe {
        (
            take(obs_module_locale_file(ptr(l))),
            take(oracle_obs_module_locale_file(ptr(l))),
        )
    }
}

fn is_default_both(a: &Option<CString>, b: &Option<CString>) -> (bool, bool) {
    // SAFETY: NULL or NUL-terminated strings.
    unsafe {
        (
            obs_module_locale_is_default(ptr(a), ptr(b)),
            oracle_obs_module_locale_is_default(ptr(a), ptr(b)),
        )
    }
}

/// The files one side's `obs_module_load_locale()` reads: the default's,
/// then the locale's when it is not the default (`None` when missing), or
/// `None` when the default's is missing.
type Read = Option<(Vec<u8>, Option<Vec<u8>>)>;

struct Side {
    default_file: unsafe extern "C" fn(*const c_char) -> *mut c_char,
    locale_file: unsafe extern "C" fn(*const c_char) -> *mut c_char,
    is_default: unsafe extern "C" fn(*const c_char, *const c_char) -> bool,
    find_file: unsafe extern "C" fn(*const c_char, *const c_char) -> *mut c_char,
}

const SHIM: Side = Side {
    default_file: obs_module_locale_default_file,
    locale_file: obs_module_locale_file,
    is_default: obs_module_locale_is_default,
    find_file: obs_module_paths_find_file,
};

const ORACLE: Side = Side {
    default_file: oracle_obs_module_locale_default_file,
    locale_file: oracle_obs_module_locale_file,
    is_default: oracle_obs_module_locale_is_default,
    find_file: oracle_obs_module_paths_find_file,
};

/// `obs_module_load_locale()` on one side, after its NULL checks.
fn reads(side: &Side, data: &CString, def: &CString, locale: &CString) -> Read {
    let find = |name: *mut c_char| {
        // SAFETY: a NUL-terminated data path and a `bmalloc`'d name, freed
        // after the lookup.
        let found = unsafe { take((side.find_file)(data.as_ptr(), name)) };
        // SAFETY: allocated by `bmalloc`.
        unsafe { bfree(name.cast()) };
        found
    };
    // SAFETY: NUL-terminated strings.
    let first = find(unsafe { (side.default_file)(def.as_ptr()) })?;
    // SAFETY: NUL-terminated strings.
    if unsafe { (side.is_default)(locale.as_ptr(), def.as_ptr()) } {
        return Some((first, None));
    }
    // SAFETY: a NUL-terminated string.
    let second = find(unsafe { (side.locale_file)(locale.as_ptr()) });
    Some((first, second))
}

fn reads_both(tree: &Tree, data: &str, def: &str, locale: &str) -> (Read, Read) {
    let data = tree.at(data);
    let (def, locale) = (CString::new(def).unwrap(), CString::new(locale).unwrap());
    (
        reads(&SHIM, &data, &def, &locale),
        reads(&ORACLE, &data, &def, &locale),
    )
}

fn show(b: &Option<Vec<u8>>) -> String {
    b.as_ref()
        .map_or("NULL".into(), |b| String::from_utf8_lossy(b).into_owned())
}

// ------------------------------------------------------ fixed cases

#[test]
fn names_match_c() {
    for l in name_locales() {
        let l = c(l.as_deref());
        let (rust, oracle) = default_file_both(&l);
        assert_eq!(rust, oracle, "default_file({:?})", l);
        assert!(rust.is_some());
        let (rust, oracle) = locale_file_both(&l);
        assert_eq!(rust, oracle, "locale_file({:?})", l);
        assert!(rust.is_some());
    }
}

#[test]
fn is_default_matches_c() {
    let locales = name_locales();
    let mut same = 0;
    for a in &locales {
        for b in &locales {
            let (a, b) = (c(a.as_deref()), c(b.as_deref()));
            let (rust, oracle) = is_default_both(&a, &b);
            assert_eq!(rust, oracle, "is_default({a:?}, {b:?})");
            same += usize::from(rust);
        }
    }
    // Sanity: some pairs are the default, case included.
    assert!(same > locales.len(), "{same}");
}

#[test]
fn reads_match_c() {
    let tree = Tree::new(ENTRIES);
    let (mut both, mut second, mut doubled) = (0, 0, 0);
    for data in DATA {
        for def in LOOKUP_LOCALES.iter().flatten() {
            for locale in LOOKUP_LOCALES.iter().flatten() {
                let (rust, oracle) = reads_both(&tree, data, def, locale);
                assert_eq!(rust, oracle, "{data} {def:?} {locale:?}");
                if let Some((_, Some(file))) = &rust {
                    second += 1;
                    doubled += usize::from(file.windows(2).any(|w| w == b"//"));
                }
                both += usize::from(rust.is_some());
            }
        }
    }
    // Sanity: defaults found, locales added on top, through the double '/'.
    assert!(both > 50, "{both}");
    assert!(second > 10 && doubled == second, "{second} {doubled}");
}

#[test]
fn reads_follow_the_double_separator() {
    let tree = Tree::new(ENTRIES);
    let (rust, oracle) = reads_both(&tree, "/d", "en-US", "de-DE");
    assert_eq!(rust, oracle);
    let (first, second) = rust.unwrap();
    assert_eq!(
        show(&Some(first)),
        format!("{}/d/locale/en-US.ini", tree.prefix)
    );
    assert_eq!(
        show(&second),
        format!("{}/d//locale/de-DE.ini", tree.prefix)
    );
    // The default in another case: not added.
    assert_eq!(reads_both(&tree, "/d", "en-US", "EN-us").0.unwrap().1, None);
    // A missing default: nothing, though the locale exists.
    assert_eq!(reads_both(&tree, "/nodef", "en-US", "de-DE").0, None);
}

// ------------------------------------------------------ proptests

/// Locale bytes, NUL excluded (C strings).
fn any_locale() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(
        prop_oneof![
            Just(b'/'),
            Just(b'.'),
            Just(b'-'),
            prop::sample::select(b"enUSdeDE@`[{_".to_vec()),
            1u8..=255,
        ],
        0..10,
    )
}

/// Locales of lookups: the tree's names, case changed, without '/'.
fn lookup_locale() -> impl Strategy<Value = Vec<u8>> {
    prop_oneof![
        prop::sample::select(vec![
            "en-US", "de-DE", "zh-Hans", "fr-FR", "..", "", "%module%"
        ])
        .prop_map(|l| l.as_bytes().to_vec()),
        prop::collection::vec(prop::sample::select(b"enUSdeDEzhHans-._%".to_vec()), 0..8),
    ]
    .prop_flat_map(|l| {
        prop::collection::vec(any::<bool>(), l.len()).prop_map(move |flip| {
            l.iter()
                .zip(&flip)
                .map(|(&b, &f)| {
                    if f && b.is_ascii_alphabetic() {
                        b ^ 0x20
                    } else {
                        b
                    }
                })
                .collect()
        })
    })
}

fn opt(v: Vec<u8>, null: bool) -> Option<CString> {
    (!null).then(|| CString::new(v).unwrap())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2048))]

    #[test]
    fn names_match_c_for_any_locale(l in any_locale(), null in any::<bool>()) {
        let l = opt(l, null);
        let (rust, oracle) = default_file_both(&l);
        prop_assert_eq!(rust, oracle);
        let (rust, oracle) = locale_file_both(&l);
        prop_assert_eq!(rust, oracle);
    }

    #[test]
    fn is_default_matches_c_for_any_pair(
        a in any_locale(),
        b in any_locale(),
        upper in any::<bool>(),
        nulls in any::<[bool; 2]>(),
    ) {
        // Half the pairs are one locale in two cases.
        let b = if upper { a.to_ascii_uppercase() } else { b };
        let (a, b) = (opt(a, nulls[0]), opt(b, nulls[1]));
        let (rust, oracle) = is_default_both(&a, &b);
        prop_assert_eq!(rust, oracle);
    }

    #[test]
    fn reads_match_c_for_any_locale(
        data in prop::sample::select(DATA.to_vec()),
        def in lookup_locale(),
        locale in lookup_locale(),
    ) {
        let tree = Tree::new(ENTRIES);
        let (def, locale) = (String::from_utf8(def).unwrap(), String::from_utf8(locale).unwrap());
        let (rust, oracle) = reads_both(&tree, data, &def, &locale);
        prop_assert_eq!(rust, oracle);
    }
}
