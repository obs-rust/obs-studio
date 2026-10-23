//! Tier 1: the safe core of `libobs/obs-module-paths.c.h`.
//!
//! The first nine tests mirror the module search path cases of
//! `test/cmocka/test_core_module_load.c` one to one, with the same names, in
//! the same order, with the same layouts and the same expected values, on a
//! fake file system: [`find`] stands for `obs_find_modules2()` over the
//! paths a case adds (`obs_add_module_path()` drops NULL paths before they
//! get here), [`find_file`] for `obs_find_module_file()` on a module with
//! that data path, and [`data_dir`] for the data path a module gets. The
//! root is "/R" where the C test writes "R".
//!
//! Cases marked "characterized, not endorsed" are odd but real (#1876) and
//! kept as the C has them. The cases after the nine pin the edges of the
//! safe API: the search pattern, NULL against empty, the host's extension
//! hook, the macOS and Windows extensions, what the glob returns, names
//! with '.' and `%module%` in them, the one intentional difference (an
//! empty binary directory), and proptests over any bytes.

use std::cell::RefCell;
use std::collections::BTreeSet;

use obs_core::module_paths::{
    GlobEntry, LIB_PREFIX, MODULE_PLACEHOLDER, ModuleFs, ModuleInfo, data_dir, find, find_file,
    search_pattern,
};
use proptest::prelude::*;

/// A file system of empty files and directories. Paths are kept without
/// repeated or trailing '/', as the kernel resolves them.
#[derive(Default)]
struct FakeFs {
    files: BTreeSet<Vec<u8>>,
    dirs: BTreeSet<Vec<u8>>,
    /// Every pattern `glob()` got, in order.
    globs: RefCell<Vec<Vec<u8>>>,
    /// Every path `file_exists()` got, in order.
    asked: RefCell<Vec<Vec<u8>>>,
    /// `glob()` fails (`os_glob` returns non-zero) for every pattern.
    glob_fails: bool,
    /// Extra matches every `glob()` returns first ("." and "..").
    extra: Vec<GlobEntry>,
}

/// `path` with repeated '/' as one and no trailing '/', and whether it had
/// one (then only a directory matches).
fn normalize(path: &[u8]) -> (Vec<u8>, bool) {
    let trailing = path.len() > 1 && path.ends_with(b"/");
    let mut out = Vec::new();
    for (i, part) in path.split(|&b| b == b'/').enumerate() {
        if part.is_empty() {
            if i == 0 {
                out.push(b'/');
            }
            continue;
        }
        if !out.is_empty() && !out.ends_with(b"/") {
            out.push(b'/');
        }
        out.extend_from_slice(part);
    }
    (out, trailing)
}

impl FakeFs {
    fn with(paths: &[&str]) -> Self {
        let mut fs = Self::default();
        for p in paths {
            fs.add(p);
        }
        fs
    }

    /// A path ending in '/' is a directory, else a file; parents are
    /// directories.
    fn add(&mut self, path: &str) {
        let (norm, dir) = normalize(path.as_bytes());
        let mut parent = norm.clone();
        while let Some(at) = parent.iter().rposition(|&b| b == b'/') {
            parent.truncate(at);
            if parent.is_empty() {
                break;
            }
            self.dirs.insert(parent.clone());
        }
        if dir {
            self.dirs.insert(norm);
        } else {
            self.files.insert(norm);
        }
    }
}

impl ModuleFs for FakeFs {
    fn file_exists(&self, path: &[u8]) -> bool {
        self.asked.borrow_mut().push(path.to_vec());
        if path.is_empty() {
            return false;
        }
        let (norm, dir_only) = normalize(path);
        self.dirs.contains(&norm) || (!dir_only && self.files.contains(&norm))
    }

    /// Patterns of the form `<dir>/*<suffix>` (or `*<suffix>`, relative):
    /// the entries of `<dir>` whose name ends with the suffix and does not
    /// start with '.', sorted, as POSIX `glob()`.
    fn glob(&self, pattern: &[u8]) -> Option<Vec<GlobEntry>> {
        self.globs.borrow_mut().push(pattern.to_vec());
        if self.glob_fails {
            return None;
        }
        let star = pattern.iter().position(|&b| b == b'*')?;
        let (dir, suffix) = (&pattern[..star], &pattern[star + 1..]);
        let (base, _) = normalize(dir);
        let mut out = self.extra.clone();
        for (set, directory) in [(&self.files, false), (&self.dirs, true)] {
            for p in set {
                let Some(at) = p.iter().rposition(|&b| b == b'/') else {
                    if base.is_empty() && p.ends_with(suffix) && !p.starts_with(b".") {
                        out.push(GlobEntry {
                            path: p.clone(),
                            directory,
                        });
                    }
                    continue;
                };
                let (parent, name) = (&p[..at.max(1)], &p[at + 1..]);
                let parent = if at == 0 { b"/".as_slice() } else { parent };
                if parent == base.as_slice() && name.ends_with(suffix) && !name.starts_with(b".") {
                    let mut path = dir.to_vec();
                    path.extend_from_slice(name);
                    out.push(GlobEntry { path, directory });
                }
            }
        }
        let extra = self.extra.len();
        out[extra..].sort_by(|a, b| a.path.cmp(&b.path));
        (!out.is_empty()).then_some(out)
    }
}

/// One reported module, owned.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Found {
    bin: Option<Vec<u8>>,
    data: Vec<u8>,
    name: Option<Vec<u8>>,
}

impl Found {
    fn new(bin: &str, data: &str, name: &str) -> Self {
        Self {
            bin: Some(bin.as_bytes().to_vec()),
            data: data.as_bytes().to_vec(),
            name: Some(name.as_bytes().to_vec()),
        }
    }
}

/// The libobs extension hook on Linux and Windows: `ext` appended.
fn append(ext: &'static [u8]) -> impl FnMut(&mut Vec<u8>) {
    move |p: &mut Vec<u8>| p.extend_from_slice(ext)
}

/// [`find`] with the Linux extension, ".so", and its hook.
fn find_so(fs: &FakeFs, bin: &str, data: Option<&str>) -> Vec<Found> {
    find_with(
        fs,
        Some(bin.as_bytes()),
        data.map(str::as_bytes),
        b".so",
        &mut append(b".so"),
    )
}

fn find_with(
    fs: &FakeFs,
    bin: Option<&[u8]>,
    data: Option<&[u8]>,
    ext: &[u8],
    add: &mut dyn FnMut(&mut Vec<u8>),
) -> Vec<Found> {
    let mut found = Vec::new();
    find(fs, bin, data, ext, add, &mut |info: &ModuleInfo<'_>| {
        found.push(Found {
            bin: info.bin_path.map(<[u8]>::to_vec),
            data: info.data_path.to_vec(),
            name: info.name.map(<[u8]>::to_vec),
        });
    });
    found
}

fn file(fs: &FakeFs, data_path: Option<&str>, f: Option<&str>) -> Option<String> {
    find_file(fs, data_path.map(str::as_bytes), f.map(str::as_bytes))
        .map(|p| String::from_utf8(p).unwrap())
}

fn dir(fs: &FakeFs, name: Option<&str>, data: Option<&str>) -> Option<String> {
    data_dir(fs, name.map(str::as_bytes), data.map(str::as_bytes))
        .map(|p| String::from_utf8(p).unwrap())
}

// --------------------------------------------------- the C cases, 1:1

#[test]
fn search_flat_directory() {
    let fs = FakeFs::with(&[
        "/R/plugins/a.so",
        "/R/plugins/d.so",
        "/R/plugins/libb.so",
        "/R/plugins/libe.so",
        "/R/plugins/libf.so",
        "/R/plugins/c.txt",
        "/R/plugins/sub/",
        "/R/plugins/dir.so/",
        "/R/data/a/",
        "/R/data/b/",
        "/R/data/libf/",
    ]);
    let found = find_so(&fs, "/R/plugins", Some("/R/data/%module%"));
    assert_eq!(
        found,
        [
            Found::new("/R/plugins/a.so", "/R/data/a", "a"),
            // Characterized, not endorsed: reported though data/d does not
            // exist.
            Found::new("/R/plugins/d.so", "/R/data/d", "d"),
            // data/libb does not exist: data/b.
            Found::new("/R/plugins/libb.so", "/R/data/b", "libb"),
            // Characterized, not endorsed: neither exists, still the one
            // without "lib".
            Found::new("/R/plugins/libe.so", "/R/data/e", "libe"),
            // data/libf exists: kept.
            Found::new("/R/plugins/libf.so", "/R/data/libf", "libf"),
        ]
    );
}

#[test]
fn search_trailing_separators() {
    let fs = FakeFs::with(&["/R/plugins/a.so", "/R/p2/x.so", "/R/p3/LIBy.so"]);
    let mut found = find_so(&fs, "/R/plugins/", Some("/R/data/%module%/"));
    found.extend(find_so(&fs, "/R/p2", Some("/R/d2/%module%//")));
    found.extend(find_so(&fs, "/R/p3", Some("/R/d3/%module%/")));
    assert_eq!(
        found,
        [
            Found::new("/R/plugins/a.so", "/R/data/a", "a"),
            // Characterized, not endorsed: one '/' is taken off, not two.
            Found::new("/R/p2/x.so", "/R/d2/x/", "x"),
            // "lib" in any case.
            Found::new("/R/p3/LIBy.so", "/R/d3/y", "LIBy"),
        ]
    );
}

#[test]
fn search_module_directories() {
    let fs = FakeFs::with(&[
        "/R/mods/x/bin/64bit/x.so",
        "/R/mods/x/data/",
        "/R/mods/y/bin/64bit/liby.so",
        "/R/mods/z/bin/64bit/",
        "/R/mods/w.so",
        "/R/mods/p.q/bin/64bit/p.so",
        "/R/mods/p.q/bin/64bit/p.q.so",
        "/R/mods/libl/bin/64bit/libl.so",
        "/R/mods/v/bin/64bit/v.so/",
    ]);
    let found = find_so(
        &fs,
        "/R/mods/%module%/bin/64bit",
        Some("/R/mods/%module%/data"),
    );
    // Characterized, not endorsed: p.q is looked up as "p" (mods/p/...).
    // w.so is a file, z has no binary.
    assert_eq!(
        found,
        [
            // mods/libl/data does not exist: mods/l/data.
            Found::new("/R/mods/libl/bin/64bit/libl.so", "/R/mods/l/data", "libl"),
            // Characterized, not endorsed: a directory named v.so passes
            // for the binary.
            Found::new("/R/mods/v/bin/64bit/v.so", "/R/mods/v/data", "v"),
            Found::new("/R/mods/x/bin/64bit/x.so", "/R/mods/x/data", "x"),
            Found::new("/R/mods/y/bin/64bit/liby.so", "/R/mods/y/data", "y"),
        ]
    );
}

#[test]
fn search_repeated_placeholder() {
    let fs = FakeFs::with(&["/R/plugins/a.so", "/R/mods/x/x/x.so"]);
    let mut found = find_so(&fs, "/R/plugins", Some("/R/data/%module%/%module%"));
    found.extend(find_so(&fs, "/R/plugins", Some("/R/shared")));
    found.extend(find_so(
        &fs,
        "/R/mods/%module%/%module%",
        Some("/R/md/%module%"),
    ));
    assert_eq!(
        found,
        [
            Found::new("/R/plugins/a.so", "/R/data/a/a", "a"),
            Found::new("/R/plugins/a.so", "/R/shared", "a"),
            Found::new("/R/mods/x/x/x.so", "/R/md/x", "x"),
        ]
    );
}

#[test]
fn search_missing_and_empty() {
    let fs = FakeFs::with(&["/R/plugins/a.so", "/R/plugins/libb.so"]);
    assert_eq!(find_so(&fs, "/R/nowhere", Some("/R/data/%module%")), []);
    assert_eq!(
        find_so(&fs, "/R/mods/%module%/bin", Some("/R/data/%module%")),
        []
    );
    // obs_add_module_path() drops a NULL bin; a NULL data reports nothing.
    assert_eq!(find_so(&fs, "/R/plugins", None), []);
    // Characterized, not endorsed: an empty data path, or "/", reports no
    // module.
    assert_eq!(find_so(&fs, "/R/plugins", Some("")), []);
    assert_eq!(find_so(&fs, "/R/plugins", Some("/")), []);

    assert_eq!(
        find_so(&fs, "/R/plugins", Some("/R/data/%module%")),
        [
            Found::new("/R/plugins/a.so", "/R/data/a", "a"),
            Found::new("/R/plugins/libb.so", "/R/data/b", "libb"),
        ]
    );
}

#[test]
fn search_duplicate_paths() {
    let fs = FakeFs::with(&["/R/plugins/a.so"]);
    // Characterized, not endorsed: a path added twice is searched twice.
    let mut found = find_so(&fs, "/R/plugins", Some("/R/data/%module%"));
    found.extend(find_so(&fs, "/R/plugins", Some("/R/data/%module%")));
    let a = Found::new("/R/plugins/a.so", "/R/data/a", "a");
    assert_eq!(found, [a.clone(), a]);
}

#[test]
fn find_module_file() {
    let fs = FakeFs::with(&["/R/data/stub/locale/en-US.ini", "/R/data/stub/sub/"]);
    let stub = Some("/R/data/stub");
    assert_eq!(
        file(&fs, stub, Some("locale/en-US.ini")).as_deref(),
        Some("/R/data/stub/locale/en-US.ini")
    );
    assert_eq!(file(&fs, stub, Some("locale/xx-XX.ini")), None);
    assert_eq!(
        file(&fs, stub, Some("sub")).as_deref(),
        Some("/R/data/stub/sub")
    );
    assert_eq!(
        file(&fs, stub, Some("sub/")).as_deref(),
        Some("/R/data/stub/sub/")
    );
    // Characterized, not endorsed: a NULL or empty file is the data path.
    assert_eq!(file(&fs, stub, None).as_deref(), Some("/R/data/stub"));
    assert_eq!(file(&fs, stub, Some("")).as_deref(), Some("/R/data/stub"));
    // Characterized, not endorsed: one more '/' before a leading '/'.
    assert_eq!(
        file(&fs, stub, Some("/locale/en-US.ini")).as_deref(),
        Some("/R/data/stub//locale/en-US.ini")
    );
    // A NULL module is obs-module.c's (it returns NULL before the lookup).

    let slash = Some("/R/data/stub/");
    assert_eq!(
        file(&fs, slash, Some("locale/en-US.ini")).as_deref(),
        Some("/R/data/stub/locale/en-US.ini")
    );
    assert_eq!(file(&fs, slash, None).as_deref(), Some("/R/data/stub/"));

    let missing = Some("/R/nodata");
    assert_eq!(file(&fs, missing, Some("locale/en-US.ini")), None);
    assert_eq!(file(&fs, missing, None), None);
}

#[test]
fn find_module_file_relative() {
    // The working directory: relative paths.
    let fs = FakeFs::with(&["rel.txt"]);
    // Characterized, not endorsed: an empty or NULL data path looks the file
    // up relative to the working directory.
    assert_eq!(
        file(&fs, Some(""), Some("rel.txt")).as_deref(),
        Some("rel.txt")
    );
    assert_eq!(file(&fs, None, Some("rel.txt")).as_deref(), Some("rel.txt"));
    assert_eq!(file(&fs, Some(""), Some("none.txt")), None);
    assert_eq!(file(&fs, Some(""), None), None);
}

#[test]
fn load_plugins_data_path() {
    let fs = FakeFs::with(&[
        "/R/plugs/pathstub/bin/64bit/pathstub.so",
        "/R/plugs/pathstub/data/locale/",
        "/R/plugs/libpathlib/bin/64bit/libpathlib.so",
        "/R/plugs/pathlib/data/",
    ]);
    let found = find_so(
        &fs,
        "/R/plugs/%module%/bin/64bit",
        Some("/R/plugs/%module%/data"),
    );
    assert_eq!(
        found,
        [
            Found::new(
                "/R/plugs/libpathlib/bin/64bit/libpathlib.so",
                "/R/plugs/pathlib/data",
                "libpathlib"
            ),
            Found::new(
                "/R/plugs/pathstub/bin/64bit/pathstub.so",
                "/R/plugs/pathstub/data",
                "pathstub"
            ),
        ]
    );
    assert_eq!(
        file(&fs, Some("/R/plugs/pathstub/data"), Some("locale")).as_deref(),
        Some("/R/plugs/pathstub/data/locale")
    );
}

// ------------------------------------------------------------ the edges

#[test]
fn search_pattern_of_a_binary_directory() {
    let p = |bin: &str, ext: &str| {
        search_pattern(bin.as_bytes(), ext.as_bytes())
            .map(|(p, d)| (String::from_utf8(p).unwrap(), d))
    };
    assert_eq!(
        p("/R/plugins", ".so"),
        Some(("/R/plugins/*.so".into(), false))
    );
    assert_eq!(
        p("/R/plugins/", ".so"),
        Some(("/R/plugins/*.so".into(), false))
    );
    assert_eq!(
        p("/R/plugins//", ".dll"),
        Some(("/R/plugins//*.dll".into(), false))
    );
    // macOS: no extension.
    assert_eq!(p("/R/plugins", ""), Some(("/R/plugins/*".into(), false)));
    // Up to the first %module%; the extension is not added.
    assert_eq!(
        p("/R/mods/%module%/bin", ".so"),
        Some(("/R/mods/*".into(), true))
    );
    assert_eq!(
        p("/R/mods%module%/bin", ".so"),
        Some(("/R/mods/*".into(), true))
    );
    assert_eq!(
        p("/R/%module%/%module%", ".so"),
        Some(("/R/*".into(), true))
    );
    // %module% first: a relative "*" (the cut path is NULL).
    assert_eq!(p("%module%/bin", ".so"), Some(("*".into(), true)));
    assert_eq!(p("/", ".so"), Some(("/*.so".into(), false)));
    assert_eq!(p("rel", ".so"), Some(("rel/*.so".into(), false)));
    // Intentional difference: the C crashes on an empty one.
    assert_eq!(p("", ".so"), None);
}

#[test]
fn search_globs_once_per_path() {
    let fs = FakeFs::with(&["/R/plugins/a.so"]);
    find_so(&fs, "/R/plugins", Some("/R/data/%module%"));
    find_so(&fs, "/R/mods/%module%/bin", Some("/R/data/%module%"));
    assert_eq!(
        *fs.globs.borrow(),
        [b"/R/plugins/*.so".to_vec(), b"/R/mods/*".to_vec()]
    );
}

#[test]
fn empty_binary_directory_searches_nothing() {
    // Intentional difference: the C passes a NULL dstr to strstr().
    let fs = FakeFs::with(&["a.so"]);
    assert_eq!(
        find_with(&fs, Some(b""), Some(b"d"), b".so", &mut append(b".so")),
        []
    );
    assert_eq!(
        find_with(&fs, None, Some(b"d"), b".so", &mut append(b".so")),
        []
    );
    assert!(fs.globs.borrow().is_empty());
}

#[test]
fn failed_glob_reports_nothing() {
    let mut fs = FakeFs::with(&["/R/plugins/a.so"]);
    fs.glob_fails = true;
    assert_eq!(find_so(&fs, "/R/plugins", Some("/R/data/%module%")), []);
    assert_eq!(fs.globs.borrow().len(), 1);
}

#[test]
fn dot_entries_are_skipped() {
    // "/R/mods/.so" would pass for the binary of "." (a NULL name) if it
    // were not skipped.
    let mut fs = FakeFs::with(&["/R/mods/x/x.so", "/R/mods/.so"]);
    fs.extra = vec![
        GlobEntry {
            path: b"/R/mods/.".to_vec(),
            directory: true,
        },
        GlobEntry {
            path: b"/R/mods/..".to_vec(),
            directory: true,
        },
        GlobEntry {
            path: b".".to_vec(),
            directory: true,
        },
    ];
    assert_eq!(
        find_so(&fs, "/R/mods/%module%", Some("/R/d/%module%")),
        [Found::new("/R/mods/x/x.so", "/R/d/x", "x")]
    );
    // And as files of a flat search.
    let fs = FakeFs {
        extra: vec![
            GlobEntry {
                path: b"/R/p/.".to_vec(),
                directory: false,
            },
            GlobEntry {
                path: b"/R/p/..".to_vec(),
                directory: false,
            },
        ],
        ..FakeFs::default()
    };
    assert_eq!(find_so(&fs, "/R/p", Some("/R/d/%module%")), []);
}

#[test]
fn files_and_directories_are_told_apart_by_the_glob() {
    // The glob's directory flag decides, not the name.
    let mut fs = FakeFs::with(&["/R/p/"]);
    fs.extra = vec![
        GlobEntry {
            path: b"/R/p/f.so".to_vec(),
            directory: false,
        },
        GlobEntry {
            path: b"/R/p/g.so".to_vec(),
            directory: true,
        },
    ];
    assert_eq!(
        find_so(&fs, "/R/p", Some("/R/d/%module%")),
        [Found::new("/R/p/f.so", "/R/d/f", "f")]
    );
}

#[test]
fn names_end_at_the_last_dot() {
    let fs = FakeFs {
        extra: vec![
            GlobEntry {
                path: b"/R/p/a.b.so".to_vec(),
                directory: false,
            },
            GlobEntry {
                path: b"/R/p/noext".to_vec(),
                directory: false,
            },
            // Only an extension: a NULL name, the data path for "".
            GlobEntry {
                path: b"/R/p/.so".to_vec(),
                directory: false,
            },
            // No '/': the whole path is the file name.
            GlobEntry {
                path: b"bare.so".to_vec(),
                directory: false,
            },
        ],
        ..FakeFs::default()
    };
    let found = find_so(&fs, "/R/p", Some("/R/d/%module%"));
    assert_eq!(
        found,
        [
            Found::new("/R/p/a.b.so", "/R/d/a.b", "a.b"),
            Found::new("/R/p/noext", "/R/d/noext", "noext"),
            Found {
                bin: Some(b"/R/p/.so".to_vec()),
                data: b"/R/d".to_vec(),
                name: None,
            },
            Found::new("bare.so", "/R/d/bare", "bare"),
        ]
    );
}

#[test]
fn a_name_with_the_placeholder_is_not_replaced_again() {
    let fs = FakeFs {
        extra: vec![GlobEntry {
            path: b"/R/p/%module%x.so".to_vec(),
            directory: false,
        }],
        ..FakeFs::default()
    };
    let found = find_so(&fs, "/R/p", Some("/R/d/%module%/%module%"));
    assert_eq!(
        found,
        [Found::new(
            "/R/p/%module%x.so",
            "/R/d/%module%x/%module%x",
            "%module%x"
        )]
    );
}

#[test]
fn module_directory_with_the_placeholder_in_its_name() {
    // A directory named "%module%": its binary directory has the name once.
    let fs = FakeFs::with(&["/R/m/%module%/b/%module%.so"]);
    assert_eq!(
        find_so(&fs, "/R/m/%module%/b", Some("/R/d/%module%")),
        [Found::new(
            "/R/m/%module%/b/%module%.so",
            "/R/d/%module%",
            "%module%"
        )]
    );
}

#[test]
fn host_hook_gets_the_directory_and_name() {
    let fs = FakeFs::with(&["/R/m/x/b/x.so", "/R/m/y/b/liby.so", "/R/m/z/"]);
    let mut seen = Vec::new();
    let found = find_with(
        &fs,
        Some(b"/R/m/%module%/b"),
        Some(b"/R/d/%module%"),
        b".so",
        &mut |p: &mut Vec<u8>| {
            seen.push(p.clone());
            p.extend_from_slice(b".so");
        },
    );
    assert_eq!(
        found,
        [
            Found::new("/R/m/x/b/x.so", "/R/d/x", "x"),
            Found::new("/R/m/y/b/liby.so", "/R/d/y", "y"),
        ]
    );
    assert_eq!(
        seen,
        [
            b"/R/m/x/b/x".to_vec(),
            b"/R/m/y/b/y".to_vec(),
            b"/R/m/z/b/z".to_vec()
        ]
    );
}

#[test]
fn hook_is_not_called_for_module_files() {
    let fs = FakeFs::with(&["/R/p/a.so"]);
    let mut calls = 0;
    let found = find_with(
        &fs,
        Some(b"/R/p"),
        Some(b"/R/d/%module%"),
        b".so",
        &mut |_: &mut Vec<u8>| {
            calls += 1;
        },
    );
    assert_eq!(found.len(), 1);
    assert_eq!(calls, 0);
}

#[test]
fn macos_bundles_and_the_cocoa_hook() {
    // macOS: get_module_extension() is "" and the hook adds ".so" only when
    // the bundle path does not exist (obs-cocoa.m).
    let fs = FakeFs::with(&[
        "/R/m/bundle/b/bundle/",
        "/R/m/legacy/b/legacy.so",
        "/R/m/libl/b/liblibl",
    ]);
    let mut cocoa = |p: &mut Vec<u8>| {
        if !fs.file_exists(p) {
            p.extend_from_slice(b".so");
        }
    };
    let found = find_with(
        &fs,
        Some(b"/R/m/%module%/b"),
        Some(b"/R/d/%module%"),
        b"",
        &mut cocoa,
    );
    assert_eq!(
        found,
        [
            Found::new("/R/m/bundle/b/bundle", "/R/d/bundle", "bundle"),
            Found::new("/R/m/legacy/b/legacy.so", "/R/d/legacy", "legacy"),
            // lib<name> with no extension.
            Found::new("/R/m/libl/b/liblibl", "/R/d/l", "libl"),
        ]
    );
    // Flat: every entry, "*" with no extension.
    let fs = FakeFs::with(&["/R/p/a", "/R/p/b.plugin"]);
    let found = find_with(
        &fs,
        Some(b"/R/p"),
        Some(b"/R/d/%module%"),
        b"",
        &mut |_: &mut Vec<u8>| {},
    );
    assert_eq!(
        found,
        [
            Found::new("/R/p/a", "/R/d/a", "a"),
            Found::new("/R/p/b.plugin", "/R/d/b", "b")
        ]
    );
}

#[test]
fn windows_extension_and_no_so_fallback() {
    // Characterized, not endorsed: the C's ".so" "legacy fallback" is
    // overwritten before it is tried, so x/x.so is not found with ".dll".
    // Nor u/u.dll.so, which the C builds (u.dll, then ".so") and drops.
    let fs = FakeFs::with(&[
        "/R/m/x/b/x.so",
        "/R/m/y/b/liby.dll",
        "/R/m/w/b/w.dll",
        "/R/m/u/b/u.dll.so",
    ]);
    let found = find_with(
        &fs,
        Some(b"/R/m/%module%/b"),
        Some(b"/R/d/%module%"),
        b".dll",
        &mut append(b".dll"),
    );
    assert_eq!(
        found,
        [
            Found::new("/R/m/w/b/w.dll", "/R/d/w", "w"),
            Found::new("/R/m/y/b/liby.dll", "/R/d/y", "y"),
        ]
    );
}

#[test]
fn hook_that_empties_the_path() {
    // Then <name> does not exist and lib<name><ext> is tried.
    let fs = FakeFs::with(&["/R/m/x/b/libx.so"]);
    let found = find_with(
        &fs,
        Some(b"/R/m/%module%/b"),
        Some(b"/R/d/%module%"),
        b".so",
        &mut |p: &mut Vec<u8>| {
            p.clear();
        },
    );
    assert_eq!(found, [Found::new("/R/m/x/b/libx.so", "/R/d/x", "x")]);
}

#[test]
fn data_dir_null_and_empty() {
    let fs = FakeFs::with(&["/R/d/"]);
    assert_eq!(dir(&fs, Some("a"), None), None);
    assert_eq!(dir(&fs, Some("a"), Some("")), None);
    assert_eq!(dir(&fs, Some("a"), Some("/")), None);
    assert_eq!(dir(&fs, Some("liba"), Some("/")), None);
    // Only the placeholder, and the name is "": "" (not NULL).
    assert_eq!(dir(&fs, None, Some("%module%")).as_deref(), Some(""));
    assert_eq!(dir(&fs, Some(""), Some("%module%")).as_deref(), Some(""));
    // "/" after the name: cut to nothing, NULL.
    assert_eq!(dir(&fs, Some(""), Some("%module%/")), None);
    assert_eq!(
        dir(&fs, None, Some("/R/d/%module%")).as_deref(),
        Some("/R/d")
    );
    assert_eq!(
        dir(&fs, Some("a"), Some("/R/d/%module%")).as_deref(),
        Some("/R/d/a")
    );
    assert_eq!(
        dir(&fs, Some("a"), Some("rel/%module%")).as_deref(),
        Some("rel/a")
    );
}

#[test]
fn data_dir_lib_prefix() {
    let fs = FakeFs::with(&["/R/d/libkept/", "/R/d/"]);
    // Exactly "lib": the name without it is "".
    assert_eq!(
        dir(&fs, Some("lib"), Some("/R/d/%module%")).as_deref(),
        Some("/R/d")
    );
    assert_eq!(
        dir(&fs, Some("LiBx"), Some("/R/d/%module%")).as_deref(),
        Some("/R/d/x")
    );
    assert_eq!(
        dir(&fs, Some("libkept"), Some("/R/d/%module%")).as_deref(),
        Some("/R/d/libkept")
    );
    assert_eq!(
        dir(&fs, Some("li"), Some("/R/d/%module%")).as_deref(),
        Some("/R/d/li")
    );
    assert_eq!(
        dir(&fs, Some("xlib"), Some("/R/d/%module%")).as_deref(),
        Some("/R/d/xlib")
    );
    // Not "lib" in another script's case: ASCII only.
    assert_eq!(
        dir(&fs, Some("l\u{130}b"), Some("/R/d/%module%")).as_deref(),
        Some("/R/d/l\u{130}b")
    );
    // Once: "liblibx" becomes "libx", not "x".
    assert_eq!(
        dir(&fs, Some("liblibx"), Some("/R/d/%module%")).as_deref(),
        Some("/R/d/libx")
    );
    // No %module%: the same path either way.
    assert_eq!(
        dir(&fs, Some("libz"), Some("/R/shared/")).as_deref(),
        Some("/R/shared")
    );
}

#[test]
fn data_dir_asks_once_unless_lib() {
    let fs = FakeFs::with(&["/R/d/"]);
    dir(&fs, Some("a"), Some("/R/d/%module%"));
    dir(&fs, Some("libb"), Some("/R/d/%module%"));
    dir(&fs, Some("a"), Some(""));
    // A NULL path is not asked about (os_file_exists(NULL) is false).
    assert_eq!(
        *fs.asked.borrow(),
        [b"/R/d/a".to_vec(), b"/R/d/libb".to_vec()]
    );
}

#[test]
fn find_file_separators() {
    let fs = FakeFs::with(&["/R/d/f", "/R/d/"]);
    assert_eq!(
        file(&fs, Some("/R/d"), Some("f")).as_deref(),
        Some("/R/d/f")
    );
    assert_eq!(
        file(&fs, Some("/R/d/"), Some("f")).as_deref(),
        Some("/R/d/f")
    );
    assert_eq!(
        file(&fs, Some("/R/d//"), Some("f")).as_deref(),
        Some("/R/d//f")
    );
    assert_eq!(
        file(&fs, Some("/R/d"), Some("/f")).as_deref(),
        Some("/R/d//f")
    );
    assert_eq!(
        file(&fs, Some("/"), Some("R/d/f")).as_deref(),
        Some("/R/d/f")
    );
    assert_eq!(file(&fs, Some("/R/d"), Some("f/")), None);
    assert_eq!(file(&fs, None, None), None);
    assert_eq!(file(&fs, Some(""), Some("")), None);
}

#[test]
fn constants() {
    assert_eq!(MODULE_PLACEHOLDER, b"%module%");
    assert_eq!(LIB_PREFIX, b"lib");
}

// ------------------------------------------------------------- proptests

/// The data path as `make_data_dir()` writes it out: every placeholder
/// replaced, then one trailing '/' off. NULL for an empty pattern and for
/// one that the '/' leaves empty; "" when the replacement alone is empty.
fn written_data_dir(pattern: &[u8], name: &[u8]) -> Option<Vec<u8>> {
    if pattern.is_empty() {
        return None;
    }
    let mut out = Vec::new();
    let mut rest = pattern;
    while let Some(at) = rest.windows(8).position(|w| w == b"%module%") {
        out.extend_from_slice(&rest[..at]);
        out.extend_from_slice(name);
        rest = &rest[at + 8..];
    }
    out.extend_from_slice(rest);
    if out.ends_with(b"/") {
        out.pop();
        if out.is_empty() {
            return None;
        }
    }
    Some(out)
}

fn path_bytes() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(
        prop_oneof![
            Just(b'/'),
            Just(b'%'),
            Just(b'.'),
            Just(b'l'),
            Just(b'i'),
            Just(b'b'),
            Just(b'L'),
            1u8..=255,
        ],
        0..24,
    )
    .prop_map(|mut v| {
        // Sometimes the placeholder, anywhere.
        if v.len() % 3 == 0 {
            let at = v.len() / 2;
            v.splice(at..at, MODULE_PLACEHOLDER.iter().copied());
        }
        v
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2048))]

    #[test]
    fn data_dir_is_the_written_rule(
        pattern in path_bytes(),
        name in path_bytes(),
        exists_first in any::<bool>(),
    ) {
        let first = written_data_dir(&pattern, &name);
        let mut fs = FakeFs::default();
        if let Some(p) = first.as_ref().filter(|p| exists_first && !p.is_empty()) {
            fs.dirs.insert(normalize(p).0);
        }
        let got = data_dir(&fs, Some(&name), Some(&pattern));
        let lib = name.len() >= 3 && name[..3].eq_ignore_ascii_case(b"lib");
        let found = first.as_ref().is_some_and(|p| fs.file_exists(p));
        let want = if !found && lib { written_data_dir(&pattern, &name[3..]) } else { first };
        prop_assert_eq!(got, want);
    }

    #[test]
    fn find_file_is_the_written_rule(
        data in path_bytes(),
        f in path_bytes(),
        exists in any::<bool>(),
    ) {
        let mut want = data.clone();
        if !data.is_empty() && !data.ends_with(b"/") && !f.is_empty() {
            want.push(b'/');
        }
        want.extend_from_slice(&f);
        let mut fs = FakeFs::default();
        if exists && !want.is_empty() {
            fs.files.insert(want.clone());
        }
        let got = find_file(&fs, Some(&data), Some(&f));
        // The fake normalizes, so look the same path up the same way.
        let found = !want.is_empty() && fs.file_exists(&want);
        prop_assert_eq!(got, found.then_some(want));
    }

    #[test]
    fn flat_search_reports_every_file(
        names in prop::collection::btree_set("[a-z]{1,6}", 0..8),
    ) {
        let mut fs = FakeFs::default();
        for n in &names {
            fs.add(&format!("/R/p/{n}.so"));
        }
        let found = find_so(&fs, "/R/p", Some("/R/d/%module%"));
        let want: Vec<Found> = names
            .iter()
            .map(|n| {
                let data = n.strip_prefix("lib").unwrap_or(n);
                let data = if data.is_empty() { "/R/d".to_owned() } else { format!("/R/d/{data}") };
                Found::new(&format!("/R/p/{n}.so"), &data, n)
            })
            .collect();
        prop_assert_eq!(found, want);
    }
}
