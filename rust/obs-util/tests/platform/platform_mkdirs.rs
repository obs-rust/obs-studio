//! Tier 1: 1:1 port of `test/cmocka/test_platform_mkdirs.c` against the safe
//! core. Each test names the cmocka case it mirrors.

use std::path::Path;

use obs_c_oracle as _; // links the test allocator
use obs_util::platform_mkdirs::{MKDIR_ERROR, MKDIR_EXISTS, MKDIR_SUCCESS, mkdirs};

use crate::common::ScratchDir;

/// `root` joined with `rel` as bytes, with `/` between them.
fn p(root: &Path, rel: &str) -> Vec<u8> {
    format!("{}/{rel}", root.to_str().expect("utf-8 temp path")).into_bytes()
}

fn is_dir(root: &Path, rel: &str) -> bool {
    root.join(rel).is_dir()
}

/// `test_mkdirs_creates_missing_parents`
#[test]
fn mkdirs_creates_missing_parents() {
    let root = ScratchDir::new("mkdirs-parents");

    assert_eq!(mkdirs(&p(&root, "n1/n2/n3")), MKDIR_SUCCESS);
    assert!(is_dir(&root, "n1"));
    assert!(is_dir(&root, "n1/n2"));
    assert!(is_dir(&root, "n1/n2/n3"));

    assert_eq!(mkdirs(&p(&root, "n1/n2/n3")), MKDIR_EXISTS);
    assert_eq!(mkdirs(&p(&root, "n1")), MKDIR_EXISTS);
}

/// `test_mkdirs_single_component`
#[test]
fn mkdirs_single_component() {
    let root = ScratchDir::new("mkdirs-single");

    assert_eq!(mkdirs(&p(&root, "single")), MKDIR_SUCCESS);
    assert!(is_dir(&root, "single"));
    assert_eq!(mkdirs(&p(&root, "single")), MKDIR_EXISTS);
    assert_eq!(
        mkdirs(root.to_str().expect("utf-8 temp path").as_bytes()),
        MKDIR_EXISTS
    );
}

/// `test_mkdirs_some_parents_exist`
#[test]
fn mkdirs_some_parents_exist() {
    let root = ScratchDir::new("mkdirs-some");

    std::fs::create_dir(root.join("p1")).unwrap();
    assert_eq!(mkdirs(&p(&root, "p1/p2/p3")), MKDIR_SUCCESS);
    assert!(is_dir(&root, "p1/p2/p3"));
}

/// `test_mkdirs_backslashes`
#[test]
fn mkdirs_backslashes() {
    let root = ScratchDir::new("mkdirs-backslash");

    let path = p(&root, "b1\\b2\\b3");
    let before = path.clone();
    assert_eq!(mkdirs(&path), MKDIR_SUCCESS);
    assert!(is_dir(&root, "b1/b2/b3"));
    assert_eq!(path, before);

    assert_eq!(mkdirs(&p(&root, "b1\\b2")), MKDIR_EXISTS);
}

/// `test_mkdirs_mixed_separators`
#[test]
fn mkdirs_mixed_separators() {
    let root = ScratchDir::new("mkdirs-mixed");

    assert_eq!(mkdirs(&p(&root, "m1\\m2/m3")), MKDIR_SUCCESS);
    assert!(is_dir(&root, "m1/m2/m3"));
}

/// `test_mkdirs_trailing_slash`
#[test]
fn mkdirs_trailing_slash() {
    let root = ScratchDir::new("mkdirs-trailing");

    assert_eq!(mkdirs(&p(&root, "t3/")), MKDIR_SUCCESS);
    assert!(is_dir(&root, "t3"));
    assert_eq!(mkdirs(&p(&root, "t3/")), MKDIR_EXISTS);

    // Characterized, not endorsed: both directories are created, and the
    // last os_mkdir of "t1/t2/" finds "t1/t2".
    assert_eq!(mkdirs(&p(&root, "t1/t2/")), MKDIR_EXISTS);
    assert!(is_dir(&root, "t1/t2"));
}

/// `test_mkdirs_doubled_slash`
#[test]
fn mkdirs_doubled_slash() {
    let root = ScratchDir::new("mkdirs-doubled");

    assert_eq!(mkdirs(&p(&root, "d1//d2")), MKDIR_SUCCESS);
    assert!(is_dir(&root, "d1/d2"));
}

/// `test_mkdirs_non_ascii`
#[test]
fn mkdirs_non_ascii() {
    let root = ScratchDir::new("mkdirs-non-ascii");

    assert_eq!(mkdirs(&p(&root, "\u{fc}/x")), MKDIR_SUCCESS);
    assert!(is_dir(&root, "\u{fc}/x"));
}

/// `test_mkdirs_file_in_the_way`
#[test]
fn mkdirs_file_in_the_way() {
    let root = ScratchDir::new("mkdirs-file");

    std::fs::write(root.join("file"), b"x").unwrap();

    // Characterized, not endorsed: a file at the path counts as existing.
    assert_eq!(mkdirs(&p(&root, "file")), MKDIR_EXISTS);
    assert!(!is_dir(&root, "file"));

    assert_eq!(mkdirs(&p(&root, "file/sub")), MKDIR_ERROR);
    assert_eq!(mkdirs(&p(&root, "file/sub/deeper")), MKDIR_ERROR);
    assert!(!is_dir(&root, "file/sub"));
}

/// New edge case: the empty path fails.
#[test]
fn mkdirs_empty_is_error() {
    assert_eq!(mkdirs(b""), MKDIR_ERROR);
}

/// New edge case: 32 missing levels are created in one call.
#[test]
fn mkdirs_deep_chain() {
    let root = ScratchDir::new("mkdirs-deep");

    let rel = (0..32)
        .map(|i| format!("d{i}"))
        .collect::<Vec<_>>()
        .join("/");
    assert_eq!(mkdirs(&p(&root, &rel)), MKDIR_SUCCESS);
    assert!(is_dir(&root, &rel));
}
