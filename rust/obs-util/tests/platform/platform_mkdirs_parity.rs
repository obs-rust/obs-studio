//! Tier 3: the Rust C ABI shim for `os_mkdirs` behaves exactly like the
//! original `libobs/util/platform-mkdirs.c`, compiled as `oracle_os_mkdirs`.
//!
//! Each case runs the same calls through both sides, in two scratch
//! directories that start with the same contents (one regular file, `f`),
//! then compares the return values and the resulting trees.
//!
//! Not compared: a NULL or empty `dir`, for which the shim returns
//! `MKDIR_ERROR`. Every path here starts with the scratch root, so it is
//! never empty.
//!
//! Reminder: the mutation check (break the core, see this file fail) is done
//! once per port, not on every change.

use core::ffi::{c_char, c_int};
use std::ffi::CString;
use std::path::Path;

use obs_c_oracle::platform_mkdirs as c;
use obs_util::ffi::platform_mkdirs as rs;
use proptest::prelude::*;

use crate::common::ScratchDir;

type Mkdirs = unsafe extern "C" fn(*const c_char) -> c_int;

/// A scratch root holding the regular file `f`.
fn side(tag: &str) -> ScratchDir {
    let dir = ScratchDir::new(tag);
    std::fs::write(dir.join("f"), b"x").unwrap();
    dir
}

/// Every entry under `root`, relative and with `/` separators, and whether
/// it is a directory.
fn tree(root: &Path) -> Vec<(String, bool)> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, bool)>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let rel = path.strip_prefix(root).unwrap();
            let rel = rel
                .components()
                .map(|c| c.as_os_str().to_str().expect("utf-8 name").to_owned())
                .collect::<Vec<_>>()
                .join("/");
            let is_dir = path.is_dir();
            out.push((rel, is_dir));
            if is_dir {
                walk(root, &path, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out.sort();
    out
}

/// `os_mkdirs(root + "/" + rel)` for each `rel` through `f`: the results.
fn run(f: Mkdirs, root: &Path, rels: &[String]) -> Vec<c_int> {
    let root = root.to_str().expect("utf-8 temp path");
    rels.iter()
        .map(|rel| {
            let path = CString::new(format!("{root}/{rel}")).unwrap();
            // SAFETY: `path` is a NUL-terminated string.
            unsafe { f(path.as_ptr()) }
        })
        .collect()
}

/// Runs `rels` on both sides; the results and trees must match.
fn same(tag: &str, rels: &[&str]) {
    let rels: Vec<String> = rels.iter().map(|s| (*s).to_owned()).collect();
    let c_root = side(&format!("{tag}-c"));
    let rs_root = side(&format!("{tag}-rs"));

    let c_ret = run(c::oracle_os_mkdirs, &c_root, &rels);
    let rs_ret = run(rs::os_mkdirs, &rs_root, &rels);
    assert_eq!(rs_ret, c_ret, "results for {rels:?}");
    assert_eq!(tree(&rs_root), tree(&c_root), "trees after {rels:?}");
}

#[test]
fn nested_then_existing() {
    same("mkdirs-par-nested", &["n1/n2/n3", "n1/n2/n3", "n1", ""]);
}

#[test]
fn separators() {
    same(
        "mkdirs-par-seps",
        &["b1\\b2\\b3", "b1\\b2", "m1\\m2/m3", "d1//d2", "x\\\\y"],
    );
}

#[test]
fn trailing_slashes() {
    same(
        "mkdirs-par-trailing",
        &["t3/", "t3/", "t1/t2/", "u1\\u2\\", "v//"],
    );
}

#[test]
fn file_in_the_way() {
    same(
        "mkdirs-par-file",
        &["f", "f/", "f/sub", "f/sub/deeper", "f\\sub"],
    );
}

#[test]
fn non_ascii() {
    same("mkdirs-par-non-ascii", &["\u{fc}/x", "\u{fc}\\y/z"]);
}

#[test]
fn null_returns_error() {
    // SAFETY: the shim accepts NULL.
    assert_eq!(unsafe { rs::os_mkdirs(core::ptr::null()) }, -1);
    let empty = CString::default();
    // SAFETY: `empty` is a NUL-terminated string.
    assert_eq!(unsafe { rs::os_mkdirs(empty.as_ptr()) }, -1);
}

/// A relative path from a few names (including the file `f`) and
/// separators, with an optional trailing separator.
fn rel_path() -> impl Strategy<Value = String> {
    let name = prop_oneof![Just("a"), Just("b"), Just("f"), Just("\u{fc}")];
    let sep = prop_oneof![Just("/"), Just("\\"), Just("//"), Just("/\\")];
    (prop::collection::vec((name, sep), 1..5), any::<bool>()).prop_map(|(parts, trailing)| {
        let mut out = String::new();
        for (i, (name, sep)) in parts.iter().enumerate() {
            if i > 0 {
                out.push_str(sep);
            }
            out.push_str(name);
        }
        if trailing {
            out.push_str(parts[0].1);
        }
        out
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(64)))]

    #[test]
    fn random_paths_match(rels in prop::collection::vec(rel_path(), 1..5)) {
        let rels: Vec<&str> = rels.iter().map(String::as_str).collect();
        same("mkdirs-par-prop", &rels);
    }
}
