//! Tier 1: the safe core of `libobs/obs-module-names.c.h`.
//!
//! The first five tests mirror the module name and file cases of
//! `test/cmocka/test_core_module_load.c` one to one, with the same names, in
//! the same order, with the same paths and the same expected values:
//! [`find_core`] stands for `find_core_module()` on the paths the core
//! module loader gives each core module, [`file_name`] and [`module_name`]
//! for the names `obs_open_module()` and `obs_create_disabled_module()`
//! give a module, [`config_path`] for `obs_module_get_config_path()`,
//! [`manifest_path`] for the manifest `obs_module_load_metadata()` reads,
//! and [`listed`] for the safe and disabled module lists that
//! `obs_load_plugins()` checks. The root is "/R" where the C test writes
//! "R", and the extension is ".so", as on Linux.
//!
//! Cases marked "characterized, not endorsed" are odd but real (#1876) and
//! kept as the C has them. The cases after the five pin the edges: NULL
//! against empty, the macOS and Windows extensions, what `find_core` asks
//! the file system, the one intentional difference (a file name shorter
//! than the extension), and proptests over any bytes.

use std::cell::RefCell;
use std::collections::BTreeSet;

use obs_core::module_names::{
    CoreModule, MANIFEST_FILE, config_path, file_name, find_core, listed, manifest_path,
    module_name,
};
use obs_core::module_paths::{GlobEntry, ModuleFs};
use proptest::prelude::*;

/// A file system of paths that exist, which logs every `file_exists()`.
#[derive(Default)]
struct FakeFs {
    paths: BTreeSet<Vec<u8>>,
    asked: RefCell<Vec<Vec<u8>>>,
}

impl FakeFs {
    fn with(paths: &[&str]) -> Self {
        Self {
            paths: paths.iter().map(|p| p.as_bytes().to_vec()).collect(),
            asked: RefCell::default(),
        }
    }

    fn asked(&self) -> Vec<String> {
        self.asked
            .borrow()
            .iter()
            .map(|p| String::from_utf8_lossy(p).into_owned())
            .collect()
    }
}

impl ModuleFs for FakeFs {
    fn file_exists(&self, path: &[u8]) -> bool {
        self.asked.borrow_mut().push(path.to_vec());
        self.paths.contains(path)
    }

    fn glob(&self, _pattern: &[u8]) -> Option<Vec<GlobEntry>> {
        panic!("find_core does not glob");
    }
}

/// One module `find_core` reported, owned.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Found {
    bin: String,
    data: Option<String>,
    name: Option<String>,
}

fn s(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

fn find(
    fs: &FakeFs,
    binary: Option<&str>,
    name: Option<&str>,
    data: Option<&str>,
    ext: &str,
) -> (bool, Vec<Found>) {
    let mut found = Vec::new();
    let mut report = |m: &CoreModule<'_>| {
        found.push(Found {
            bin: s(m.bin_path),
            data: m.data_path.map(s),
            name: m.name.map(s),
        });
    };
    let hit = find_core(
        fs,
        binary.map(str::as_bytes),
        name.map(str::as_bytes),
        data.map(str::as_bytes),
        ext.as_bytes(),
        &mut report,
    );
    (hit, found)
}

fn name_of(file: &str) -> Option<String> {
    module_name(Some(file.as_bytes()), b".so").map(|n| s(&n))
}

/// The file name and, as the config path under "/R/cfg" shows it, the name
/// of a module with the binary path.
fn names(path: &str) -> (String, String) {
    let file = file_name(path.as_bytes());
    let name = module_name(Some(file), b".so");
    let config = config_path(Some(b"/R/cfg"), name.as_deref(), Some(b"f"));
    (s(file), s(&config))
}

fn config(dir: Option<&str>, name: Option<&str>, file: Option<&str>) -> String {
    s(&config_path(
        dir.map(str::as_bytes),
        name.map(str::as_bytes),
        file.map(str::as_bytes),
    ))
}

fn manifest(data: Option<&str>) -> String {
    s(&manifest_path(data.map(str::as_bytes)))
}

fn on(list: &[&str], name: &str) -> bool {
    listed(list.iter().map(|n| n.as_bytes()), name.as_bytes())
}

/// What `load_all_callback()` makes of the lists for plugins a, b and bb:
/// 'l' loaded, 'd' disabled. The safe list applies when it is not empty;
/// the disabled list does not apply to core modules.
fn loaded(safe: &[&str], disabled: &[&str], core: bool) -> String {
    ["a", "b", "bb"]
        .iter()
        .map(|name| {
            let is_safe = safe.is_empty() || on(safe, name);
            let is_disabled = !core && !disabled.is_empty() && on(disabled, name);
            if is_safe && !is_disabled { 'l' } else { 'd' }
        })
        .collect()
}

// ------------------------------------------- the cases of the C test

const CORE_NAMES: &[&str] = &[
    "obs-ffmpeg",
    "obs-outputs",
    "linux-capture",
    "obs-browser",
    "rtmp-services",
];

#[test]
fn core_module_paths() {
    // Every core module installed: reported with "<binary>.so", the data
    // path as given (it does not exist), the name.
    let binaries: Vec<String> = CORE_NAMES
        .iter()
        .map(|n| format!("/R/bin/../lib/obs-modules/core/{n}.so"))
        .collect();
    let refs: Vec<&str> = binaries.iter().map(String::as_str).collect();
    let fs = FakeFs::with(&refs);
    for name in CORE_NAMES {
        let binary = format!("/R/bin/../lib/obs-modules/core/{name}");
        let data = format!("/R/bin/../share/obs/obs-modules/core/{name}");
        assert_eq!(s(file_name(binary.as_bytes())), *name);
        let (hit, found) = find(&fs, Some(&binary), Some(name), Some(&data), ".so");
        assert!(hit);
        assert_eq!(
            found,
            [Found {
                bin: format!("{binary}.so"),
                data: Some(data.clone()),
                name: Some((*name).to_owned()),
            }]
        );
    }

    // One not installed (missing_file): not reported.
    let (hit, found) = find(
        &FakeFs::default(),
        Some("/R/bin/../lib/obs-modules/core/obs-ffmpeg"),
        Some("obs-ffmpeg"),
        Some("/R/bin/../share/obs/obs-modules/core/obs-ffmpeg"),
        ".so",
    );
    assert!(!hit);
    assert!(found.is_empty());
}

#[test]
fn module_names() {
    assert_eq!(
        names("/R/n/stub.so"),
        ("stub.so".into(), "/R/cfg/stub/f".into())
    );
    assert_eq!(
        names("/R/n/a.b.so"),
        ("a.b.so".into(), "/R/cfg/a.b/f".into())
    );
    assert_eq!(
        names("/R/n/libz.so"),
        ("libz.so".into(), "/R/cfg/libz/f".into())
    );
    assert_eq!(
        names("/R/n/LIBZ.SO"),
        ("LIBZ.SO".into(), "/R/cfg/LIBZ/f".into())
    );
    // Characterized, not endorsed: the last three bytes go, whatever they
    // are.
    assert_eq!(names("/R/n/noext"), ("noext".into(), "/R/cfg/no/f".into()));
    assert_eq!(names("/R/n/x.dll"), ("x.dll".into(), "/R/cfg/x./f".into()));
    assert_eq!(
        names("/R/n/d.so/x.so"),
        ("x.so".into(), "/R/cfg/x/f".into())
    );
    assert_eq!(
        names("/R/n/a\\b.so"),
        ("a\\b.so".into(), "/R/cfg/a\\b/f".into())
    );
    assert_eq!(
        names("/R/n/sp ace.so"),
        ("sp ace.so".into(), "/R/cfg/sp ace/f".into())
    );
    assert_eq!(
        names("/R//n//two.so"),
        ("two.so".into(), "/R/cfg/two/f".into())
    );
    assert_eq!(
        names("plain.so"),
        ("plain.so".into(), "/R/cfg/plain/f".into())
    );
    assert_eq!(names("abcd"), ("abcd".into(), "/R/cfg/a/f".into()));

    // Looked up by name: the name, not the file name.
    assert_eq!(name_of("stub.so").as_deref(), Some("stub"));
    assert_ne!(name_of("stub.so").as_deref(), Some("stub.so"));

    // A loaded module is named the same way.
    assert_eq!(
        names("/build/test/cmocka/core_module_stub.so"),
        (
            "core_module_stub.so".into(),
            "/R/cfg/core_module_stub/f".into()
        )
    );

    // Characterized, not endorsed: NULL names.
    assert_eq!(names("/R/n/abc"), ("abc".into(), "/R/cfg//f".into()));
    assert_eq!(names("/R/n/.so"), (".so".into(), "/R/cfg//f".into()));
}

#[test]
fn module_config_path() {
    assert_eq!(
        config(Some("/R/cfg"), Some("stub"), Some("f.json")),
        "/R/cfg/stub/f.json"
    );
    assert_eq!(
        config(Some("/R/cfg"), Some("stub"), Some("a/b.ini")),
        "/R/cfg/stub/a/b.ini"
    );
    assert_eq!(
        config(Some("/R/cfg"), Some("stub"), Some("/abs")),
        "/R/cfg/stub//abs"
    );
    assert_eq!(
        config(Some("/R/cfg"), Some("stub"), Some("")),
        "/R/cfg/stub/"
    );
    assert_eq!(config(Some("/R/cfg"), Some("stub"), None), "/R/cfg/stub/");

    assert_eq!(
        config(Some("/R/cfg/"), Some("stub"), Some("f.json")),
        "/R/cfg/stub/f.json"
    );
    // Characterized, not endorsed: only the last byte is checked.
    assert_eq!(
        config(Some("/R/cfg//"), Some("stub"), Some("f.json")),
        "/R/cfg//stub/f.json"
    );
    assert_eq!(
        config(Some("/"), Some("stub"), Some("f.json")),
        "/stub/f.json"
    );
    // Characterized, not endorsed: relative.
    assert_eq!(
        config(Some("rel"), Some("stub"), Some("f.json")),
        "rel/stub/f.json"
    );
    assert_eq!(
        config(Some(""), Some("stub"), Some("f.json")),
        "stub/f.json"
    );
    assert_eq!(config(Some(""), Some("stub"), None), "stub/");
    assert_eq!(config(None, Some("stub"), Some("f.json")), "stub/f.json");

    assert_eq!(
        config(Some("/R/cfg"), Some("core_module_stub"), Some("f.json")),
        "/R/cfg/core_module_stub/f.json"
    );
}

#[test]
fn module_manifest() {
    assert_eq!(manifest(Some("/R/m1")), "/R/m1/manifest.json");
    assert_eq!(manifest(Some("/R/m1/")), "/R/m1/manifest.json");
    assert_eq!(manifest(Some("/R/m1/sub")), "/R/m1/sub/manifest.json");
    assert_eq!(manifest(Some("/R/m2")), "/R/m2/manifest.json");
    assert_eq!(manifest(Some("/R/none")), "/R/none/manifest.json");
    assert_eq!(
        manifest(Some("/R/m1/manifest.json")),
        "/R/m1/manifest.json/manifest.json"
    );
    // Characterized, not endorsed: the working directory's manifest.
    assert_eq!(manifest(Some("")), "manifest.json");
    assert_eq!(manifest(None), "manifest.json");
}

#[test]
fn safe_and_disabled_modules() {
    assert_eq!(loaded(&[], &[], false), "lll");
    assert_eq!(loaded(&["a", "bb"], &[], false), "ldl");
    assert_eq!(loaded(&["A", "b ", ""], &[], false), "ddd");
    assert_eq!(loaded(&[], &["b"], false), "ldl");
    assert_eq!(loaded(&[], &["B", "bbb", "a.so", ""], false), "lll");
    assert_eq!(loaded(&["b", "bb"], &["bb", "bb"], false), "dld");
    assert_eq!(loaded(&[], &[], false), "lll");
    assert_eq!(loaded(&[], &["a"], true), "lll");
    assert_eq!(loaded(&["a"], &[], true), "ldd");
}

// ------------------------------------------------------------ the edges

#[test]
fn file_name_edges() {
    assert_eq!(file_name(b""), b"");
    assert_eq!(file_name(b"/"), b"");
    assert_eq!(file_name(b"//"), b"");
    assert_eq!(file_name(b"a/"), b"");
    assert_eq!(file_name(b"/a"), b"a");
    assert_eq!(file_name(b"a"), b"a");
    assert_eq!(file_name(b"C:\\x\\y.dll"), b"C:\\x\\y.dll");
    assert_eq!(file_name(b"a/b\\c"), b"b\\c");
    assert_eq!(file_name(b"\xff/\xfe.so"), b"\xfe.so");
}

#[test]
fn file_name_points_into_the_path() {
    let path = b"/R/n/stub.so".to_vec();
    let file = file_name(&path);
    assert_eq!(file.as_ptr(), path[5..].as_ptr());
    assert_eq!(file.len(), 7);
    let plain = b"plain.so".to_vec();
    assert_eq!(file_name(&plain).as_ptr(), plain.as_ptr());
}

#[test]
fn module_name_edges() {
    let n = |f: Option<&[u8]>, e: &[u8]| module_name(f, e);
    assert_eq!(n(Some(b"stub.so"), b".so"), Some(b"stub".to_vec()));
    // macOS: no extension, the whole file name.
    assert_eq!(n(Some(b"stub"), b""), Some(b"stub".to_vec()));
    assert_eq!(n(Some(b"stub.so"), b""), Some(b"stub.so".to_vec()));
    // Windows.
    assert_eq!(n(Some(b"stub.dll"), b".dll"), Some(b"stub".to_vec()));
    assert_eq!(n(Some(b"ab.so"), b".dll"), Some(b"a".to_vec()));
    // Cut to nothing: NULL.
    assert_eq!(n(Some(b".so"), b".so"), None);
    assert_eq!(n(Some(b".dll"), b".dll"), None);
    assert_eq!(n(Some(b"xyz"), b".so"), None);
    // A copy of "" is NULL, for any extension.
    assert_eq!(n(Some(b""), b""), None);
    assert_eq!(n(Some(b""), b".so"), None);
    assert_eq!(n(None, b""), None);
    assert_eq!(n(None, b".so"), None);
    // Bytes, not text.
    assert_eq!(n(Some(b"\xff\xfe.so"), b".so"), Some(b"\xff\xfe".to_vec()));
    assert_eq!(n(Some(b"\xc3\xbc.so"), b".so"), Some(b"\xc3\xbc".to_vec()));
    // One byte of a two-byte character.
    assert_eq!(n(Some(b"a\xc3\xbc"), b"xy"), Some(b"a".to_vec()));
}

#[test]
fn shorter_than_the_extension_has_no_name() {
    // Intentional difference: the C's length underflows and the process
    // crashes (characterized as not tested in test_core_module_load).
    assert_eq!(module_name(Some(b"ab"), b".so"), None);
    assert_eq!(module_name(Some(b"a"), b".so"), None);
    assert_eq!(module_name(Some(b"a.so"), b".dylib"), None);
}

#[test]
fn config_path_edges() {
    assert_eq!(config(None, None, None), "/");
    assert_eq!(config(Some(""), Some(""), Some("")), "/");
    assert_eq!(config(Some("c"), None, None), "c//");
    assert_eq!(config(Some("c"), None, Some("f")), "c//f");
    assert_eq!(config(Some("c"), Some(""), Some("f")), "c//f");
    assert_eq!(config(Some("c\\"), Some("m"), Some("f")), "c\\/m/f");
    assert_eq!(config(Some("c"), Some("m/"), Some("f")), "c/m//f");
    assert_eq!(config(Some("c"), Some("m"), Some("f/")), "c/m/f/");
    assert_eq!(
        config_path(Some(b"\xff"), Some(b"\xfe"), Some(b"\xfd")),
        b"\xff/\xfe/\xfd"
    );
}

#[test]
fn manifest_path_edges() {
    assert_eq!(MANIFEST_FILE, b"manifest.json");
    assert_eq!(manifest(Some("/")), "/manifest.json");
    assert_eq!(manifest(Some("//")), "//manifest.json");
    assert_eq!(manifest(Some("d//")), "d//manifest.json");
    assert_eq!(manifest(Some("rel")), "rel/manifest.json");
    assert_eq!(manifest(Some("d\\")), "d\\/manifest.json");
    assert_eq!(manifest_path(Some(b"\xff")), b"\xff/manifest.json");
}

#[test]
fn listed_edges() {
    assert!(!on(&[], "a"));
    assert!(!on(&[], ""));
    assert!(on(&["a"], "a"));
    assert!(on(&["x", "a"], "a"));
    assert!(on(&["a", "a"], "a"));
    assert!(!on(&["a"], "A"));
    assert!(!on(&["a"], "ab"));
    assert!(!on(&["ab"], "a"));
    assert!(!on(&["a "], "a"));
    assert!(on(&[""], ""));
    assert!(!on(&[""], "a"));
    assert!(listed([b"\xff".as_slice()], b"\xff"));
    // Every name is compared, up to the first match.
    let seen = RefCell::new(0usize);
    let names = [b"x".as_slice(), b"a", b"y"];
    let counted = names.iter().map(|n| {
        *seen.borrow_mut() += 1;
        *n
    });
    assert!(listed(counted, b"a"));
    assert_eq!(*seen.borrow(), 2);
}

#[test]
fn find_core_reports_the_binary_with_the_extension() {
    let fs = FakeFs::with(&["/c/m.so"]);
    let (hit, found) = find(&fs, Some("/c/m"), Some("m"), Some("/d/m"), ".so");
    assert!(hit);
    assert_eq!(
        found,
        [Found {
            bin: "/c/m.so".into(),
            data: Some("/d/m".into()),
            name: Some("m".into()),
        }]
    );
    // The data path is made first, then the binary is looked for.
    assert_eq!(fs.asked(), ["/d/m", "/c/m.so"]);
}

#[test]
fn find_core_makes_the_data_path_without_the_binary() {
    let fs = FakeFs::default();
    let (hit, found) = find(&fs, Some("/c/m"), Some("m"), Some("/d/m"), ".so");
    assert!(!hit);
    assert!(found.is_empty());
    assert_eq!(fs.asked(), ["/d/m", "/c/m.so"]);
}

#[test]
fn find_core_data_path_is_the_search_path_one() {
    // Every %module% replaced, one trailing '/' taken off, the "lib"
    // fallback: obs_module_paths_data_dir().
    let fs = FakeFs::with(&["/c/libm.so"]);
    let (_, found) = find(
        &fs,
        Some("/c/libm"),
        Some("libm"),
        Some("/d/%module%/"),
        ".so",
    );
    assert_eq!(found[0].data.as_deref(), Some("/d/m"));
    assert_eq!(fs.asked(), ["/d/libm", "/c/libm.so"]);

    let fs = FakeFs::with(&["/c/libm.so", "/d/libm"]);
    let (_, found) = find(
        &fs,
        Some("/c/libm"),
        Some("libm"),
        Some("/d/%module%"),
        ".so",
    );
    assert_eq!(found[0].data.as_deref(), Some("/d/libm"));

    // NULL data paths are reported as NULL.
    let fs = FakeFs::with(&["/c/m.so"]);
    for data in [None, Some(""), Some("/")] {
        let (hit, found) = find(&fs, Some("/c/m"), Some("m"), data, ".so");
        assert!(hit);
        assert_eq!(found[0].data, None, "data {data:?}");
    }
}

#[test]
fn find_core_extensions() {
    // macOS: the binary path as given (a bundle directory).
    let fs = FakeFs::with(&["/c/m"]);
    assert!(find(&fs, Some("/c/m"), Some("m"), Some("/d"), "").0);
    assert_eq!(fs.asked(), ["/d", "/c/m"]);
    // Windows.
    let fs = FakeFs::with(&["/c/m.dll"]);
    let (hit, found) = find(&fs, Some("/c/m"), Some("m"), Some("/d"), ".dll");
    assert!(hit);
    assert_eq!(found[0].bin, "/c/m.dll");
    // The wrong one.
    let fs = FakeFs::with(&["/c/m.so"]);
    assert!(!find(&fs, Some("/c/m"), Some("m"), Some("/d"), ".dll").0);
}

#[test]
fn find_core_empty_and_null() {
    // An empty or NULL binary is the extension alone (relative).
    let fs = FakeFs::with(&[".so"]);
    for binary in [None, Some("")] {
        let (hit, found) = find(&fs, binary, Some("m"), Some("/d"), ".so");
        assert!(hit, "binary {binary:?}");
        assert_eq!(found[0].bin, ".so");
    }
    // ... and with no extension, a NULL path, which does not exist.
    let fs = FakeFs::with(&[""]);
    let (hit, _) = find(&fs, Some(""), Some("m"), Some("/d"), "");
    assert!(!hit);
    assert_eq!(fs.asked(), ["/d"]);
    // A NULL name is passed on, and makes the data path with "".
    let fs = FakeFs::with(&["/c/m.so"]);
    let (hit, found) = find(&fs, Some("/c/m"), None, Some("/d/%module%x"), ".so");
    assert!(hit);
    assert_eq!(found[0].name, None);
    assert_eq!(found[0].data.as_deref(), Some("/d/x"));
}

#[test]
fn find_core_does_not_take_the_name_from_the_binary() {
    // find_core_module() names the module after the binary; find_core
    // reports the name it gets.
    let fs = FakeFs::with(&["/c/m.so"]);
    let (_, found) = find(&fs, Some("/c/m"), Some("other"), Some("/d/%module%"), ".so");
    assert_eq!(found[0].name.as_deref(), Some("other"));
    assert_eq!(found[0].data.as_deref(), Some("/d/other"));
}

// ------------------------------------------------------------ proptests

fn bytes() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(
        prop_oneof![
            Just(b'/'),
            Just(b'.'),
            Just(b'\\'),
            Just(b'a'),
            Just(b's'),
            Just(b'o'),
            1u8..=255,
        ],
        0..12,
    )
}

/// A module find_core reported: binary path, data path, name.
type Reported = (Vec<u8>, Option<Vec<u8>>, Option<Vec<u8>>);

fn opt(v: Vec<u8>, null: bool) -> Option<Vec<u8>> {
    (!null).then_some(v)
}

/// The rule of config_path and manifest_path: the directory, one '/' when
/// it is not empty and has none at the end, then the rest.
fn joined(dir: Option<&[u8]>, rest: &[u8]) -> Vec<u8> {
    let mut out = dir.unwrap_or_default().to_vec();
    if out.last().is_some_and(|&b| b != b'/') {
        out.push(b'/');
    }
    out.extend_from_slice(rest);
    out
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2048))]

    #[test]
    fn file_name_is_after_the_last_separator(path in bytes()) {
        let file = file_name(&path);
        prop_assert!(!file.contains(&b'/'));
        prop_assert!(path.ends_with(file));
        let cut = path.len() - file.len();
        prop_assert!(cut == 0 || path[cut - 1] == b'/');
    }

    #[test]
    fn module_name_cuts_the_extension_length(file in bytes(), ext in bytes(), null in any::<bool>()) {
        let got = module_name(opt(file.clone(), null).as_deref(), &ext);
        let want = (!null && file.len() > ext.len()).then(|| file[..file.len() - ext.len()].to_vec());
        prop_assert_eq!(got, want);
    }

    #[test]
    fn config_path_joins(dir in bytes(), name in bytes(), file in bytes(), nulls in any::<[bool; 3]>()) {
        let dir = opt(dir, nulls[0]);
        let name = opt(name, nulls[1]);
        let file = opt(file, nulls[2]);
        let mut rest = name.clone().unwrap_or_default();
        rest.push(b'/');
        rest.extend_from_slice(file.as_deref().unwrap_or_default());
        prop_assert_eq!(
            config_path(dir.as_deref(), name.as_deref(), file.as_deref()),
            joined(dir.as_deref(), &rest)
        );
    }

    #[test]
    fn manifest_path_joins(dir in bytes(), null in any::<bool>()) {
        let dir = opt(dir, null);
        prop_assert_eq!(manifest_path(dir.as_deref()), joined(dir.as_deref(), MANIFEST_FILE));
    }

    #[test]
    fn listed_is_any_equal(names in prop::collection::vec(bytes(), 0..5), name in bytes()) {
        prop_assert_eq!(
            listed(names.iter().map(Vec::as_slice), &name),
            names.contains(&name)
        );
    }

    #[test]
    fn find_core_reports_what_exists(
        binary in bytes(),
        name in bytes(),
        data in bytes(),
        ext in prop::sample::select(vec!["", ".so", ".dll"]),
        exists in any::<bool>(),
    ) {
        let mut path = binary.clone();
        path.extend_from_slice(ext.as_bytes());
        let fs = FakeFs { paths: if exists { [path.clone()].into() } else { BTreeSet::new() }, ..FakeFs::default() };
        let mut found: Vec<Reported> = Vec::new();
        let hit = find_core(&fs, Some(&binary), Some(&name), Some(&data), ext.as_bytes(), &mut |m| {
            found.push((m.bin_path.to_vec(), m.data_path.map(<[u8]>::to_vec), m.name.map(<[u8]>::to_vec)));
        });
        let real = exists && !path.is_empty();
        prop_assert_eq!(hit, real);
        prop_assert_eq!(found.len(), usize::from(real));
        if let Some((bin, _, got_name)) = found.first() {
            prop_assert_eq!(bin, &path);
            prop_assert_eq!(got_name.as_deref(), Some(name.as_slice()));
        }
    }
}
