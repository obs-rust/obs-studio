//! Tier 3: the Rust C ABI shims for the file helpers behave exactly like
//! the original `libobs/util/platform-file.c`, compiled as `oracle_*`.
//!
//! Each case runs the same operation through both sides, in two scratch
//! directories with the same file names, then compares the return values
//! and the resulting files byte for byte. Streams opened by one side are
//! also handed to the other, since C callers mix them with their own stdio.
//! The tests run in the `C` locale, where the `_mbs_` helpers accept ASCII.
//!
//! Reminder: the mutation check (break the core, see this file fail) is done
//! once per port, not on every change.

use core::ffi::{CStr, c_char, c_int, c_void};
use core::ptr;
use std::ffi::CString;
use std::path::{Path, PathBuf};

use obs_c_oracle::platform_file as c;
use obs_util::ffi::bmem::bfree;
use obs_util::ffi::platform_file as rs;
use obs_util::platform_file::{SEEK_CUR, SEEK_END, SEEK_SET};
use proptest::prelude::*;

use crate::common::ScratchDir;

/// One side's scratch directory.
struct Side {
    dir: ScratchDir,
}

impl Side {
    fn new(tag: &str) -> Self {
        Self {
            dir: ScratchDir::new(tag),
        }
    }

    fn path(&self, name: &str) -> CString {
        CString::new(self.dir.join(name).to_str().expect("utf-8 temp path")).unwrap()
    }

    fn file(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }
}

fn pair(tag: &str) -> (Side, Side) {
    (
        Side::new(&format!("{tag}-c")),
        Side::new(&format!("{tag}-rs")),
    )
}

/// The file's bytes, or `None` when it does not exist.
fn contents(path: &Path) -> Option<Vec<u8>> {
    std::fs::read(path).ok()
}

/// Takes a `bmalloc`ed C string: its bytes up to the NUL, or `None`.
fn take(s: *mut c_char) -> Option<Vec<u8>> {
    if s.is_null() {
        return None;
    }
    // SAFETY: both sides return NUL-terminated `bmalloc`ed strings.
    let bytes = unsafe { CStr::from_ptr(s) }.to_bytes().to_vec();
    // SAFETY: `s` came from `bmalloc`.
    unsafe { bfree(s.cast()) };
    Some(bytes)
}

type Fopen = unsafe extern "C" fn(*const c_char, *const c_char) -> *mut c_void;
type FreadStr = unsafe extern "C" fn(*mut c_void, *mut *mut c_char) -> usize;

fn close(file: *mut c_void) {
    let file = core::ptr::NonNull::new(file).expect("open stream");
    // SAFETY: an open stream, closed once.
    assert!(unsafe { obs_util::platform_file::CFile::from_raw(file) }.close());
}

/// `os_fread_*` from `reader` on a stream opened by `opener`: the return
/// value and the string.
fn fread_with(opener: Fopen, reader: FreadStr, path: &CStr) -> Option<(usize, Option<Vec<u8>>)> {
    // SAFETY: C strings.
    let f = unsafe { opener(path.as_ptr(), c"rb".as_ptr()) };
    if f.is_null() {
        return None;
    }
    let mut s = ptr::null_mut();
    // SAFETY: `f` is open and `s` is writable.
    let ret = unsafe { reader(f, &mut s) };
    close(f);
    Some((ret, take(s)))
}

fn write_raw(path: &Path, data: &[u8]) {
    std::fs::write(path, data).unwrap();
}

fn assert_read_same(data: &[u8]) {
    let (cs, rs_) = pair("platform-file-read");
    write_raw(&cs.file("f.txt"), data);
    write_raw(&rs_.file("f.txt"), data);
    let (cp, rp) = (cs.path("f.txt"), rs_.path("f.txt"));

    // SAFETY: C strings.
    unsafe {
        assert_eq!(
            take(c::oracle_os_quick_read_utf8_file(cp.as_ptr())),
            take(rs::os_quick_read_utf8_file(rp.as_ptr())),
            "os_quick_read_utf8_file({data:?})"
        );
        assert_eq!(
            take(c::oracle_os_quick_read_mbs_file(cp.as_ptr())),
            take(rs::os_quick_read_mbs_file(rp.as_ptr())),
            "os_quick_read_mbs_file({data:?})"
        );
        assert_eq!(
            c::oracle_os_get_file_size(cp.as_ptr()),
            rs::os_get_file_size(rp.as_ptr()),
            "os_get_file_size({data:?})"
        );
    }

    // Each reader on a stream from each opener: the streams are the same
    // C library's.
    for opener in [c::oracle_os_fopen as Fopen, rs::os_fopen] {
        let expected = fread_with(opener, c::oracle_os_fread_utf8, &cp);
        assert_eq!(
            expected,
            fread_with(opener, rs::os_fread_utf8, &rp),
            "os_fread_utf8({data:?})"
        );
        let expected = fread_with(opener, c::oracle_os_fread_mbs, &cp);
        assert_eq!(
            expected,
            fread_with(opener, rs::os_fread_mbs, &rp),
            "os_fread_mbs({data:?})"
        );
    }
}

fn assert_write_utf8_same(data: &[u8], marker: bool) {
    let (cs, rs_) = pair("platform-file-write");
    let (cp, rp) = (cs.path("w.txt"), rs_.path("w.txt"));
    // SAFETY: C strings and a readable buffer.
    let (o, r) = unsafe {
        (
            c::oracle_os_quick_write_utf8_file(
                cp.as_ptr(),
                data.as_ptr().cast(),
                data.len(),
                marker,
            ),
            rs::os_quick_write_utf8_file(rp.as_ptr(), data.as_ptr().cast(), data.len(), marker),
        )
    };
    assert_eq!(o, r, "os_quick_write_utf8_file({data:?}, {marker})");
    assert_eq!(
        contents(&cs.file("w.txt")),
        contents(&rs_.file("w.txt")),
        "{data:?}, {marker}"
    );
}

/// `os_quick_write_mbs_file` with `len` (0 means up to the NUL).
fn assert_write_mbs_same(text: &[u8], len: usize) {
    let (cs, rs_) = pair("platform-file-wmbs");
    let (cp, rp) = (cs.path("m.txt"), rs_.path("m.txt"));
    let mut buf = text.to_vec();
    buf.push(0);
    // SAFETY: C strings; `buf` covers `len` bytes and is NUL-terminated.
    let (o, r) = unsafe {
        (
            c::oracle_os_quick_write_mbs_file(cp.as_ptr(), buf.as_ptr().cast(), len),
            rs::os_quick_write_mbs_file(rp.as_ptr(), buf.as_ptr().cast(), len),
        )
    };
    assert_eq!(o, r, "os_quick_write_mbs_file({text:?}, {len})");
    assert_eq!(
        contents(&cs.file("m.txt")),
        contents(&rs_.file("m.txt")),
        "{text:?}, {len}"
    );
}

#[test]
fn reads_match_the_c_file() {
    for data in [
        &b""[..],
        b"a",
        b"ab",
        b"abc",
        b"hello\nworld\n",
        b"\xEF",
        b"\xEF\xBB",
        b"\xEF\xBB\xBF",
        b"\xEF\xBB\xBFx",
        b"\xEF\xBB\xBF\xEF\xBB\xBFtwice",
        b"x\xEF\xBB\xBF",
        b"nul\0inside",
        b"\0",
        b"caf\xC3\xA9",
        b"\xFF\xFE high bytes",
    ] {
        assert_read_same(data);
    }
}

#[test]
fn writes_match_the_c_file() {
    for data in [
        &b""[..],
        b"x",
        b"hello",
        b"nul\0inside",
        b"\xEF\xBB\xBF",
        b"caf\xC3\xA9",
    ] {
        assert_write_utf8_same(data, false);
        assert_write_utf8_same(data, true);
    }
    for (text, len) in [
        (&b""[..], 0),
        (b"hello world", 11),
        (b"abc", 0),
        (b"abcdef", 3),
        (b"nul\0inside", 10),
        (b"caf\xC3\xA9", 0),
    ] {
        assert_write_mbs_same(text, len);
    }
}

#[test]
fn null_and_missing_paths_match() {
    let (cs, rs_) = pair("platform-file-null");
    let (cm, rm) = (cs.path("missing.txt"), rs_.path("missing.txt"));
    // SAFETY: C strings or NULL where C checks it.
    unsafe {
        assert_eq!(
            c::oracle_os_fopen(ptr::null(), c"rb".as_ptr()).is_null(),
            rs::os_fopen(ptr::null(), c"rb".as_ptr()).is_null()
        );
        assert_eq!(
            c::oracle_os_wfopen(ptr::null(), c"rb".as_ptr()).is_null(),
            rs::os_wfopen(ptr::null(), c"rb".as_ptr()).is_null()
        );
        assert!(c::oracle_os_fopen(cm.as_ptr(), c"rb".as_ptr()).is_null());
        assert!(rs::os_fopen(rm.as_ptr(), c"rb".as_ptr()).is_null());
        assert_eq!(
            c::oracle_os_get_file_size(ptr::null()),
            rs::os_get_file_size(ptr::null())
        );
        assert_eq!(
            c::oracle_os_get_file_size(cm.as_ptr()),
            rs::os_get_file_size(rm.as_ptr())
        );
        assert_eq!(
            take(c::oracle_os_quick_read_utf8_file(ptr::null())),
            take(rs::os_quick_read_utf8_file(ptr::null()))
        );
        assert_eq!(
            take(c::oracle_os_quick_read_mbs_file(ptr::null())),
            take(rs::os_quick_read_mbs_file(ptr::null()))
        );
        assert_eq!(
            c::oracle_os_quick_write_mbs_file(ptr::null(), c"x".as_ptr(), 1),
            rs::os_quick_write_mbs_file(ptr::null(), c"x".as_ptr(), 1)
        );
        assert_eq!(
            c::oracle_os_quick_write_utf8_file(ptr::null(), c"x".as_ptr(), 1, false),
            rs::os_quick_write_utf8_file(ptr::null(), c"x".as_ptr(), 1, false)
        );
        // a NULL str writes an empty file
        assert_eq!(
            c::oracle_os_quick_write_mbs_file(cm.as_ptr(), ptr::null(), 5),
            rs::os_quick_write_mbs_file(rm.as_ptr(), ptr::null(), 5)
        );
    }
    assert_eq!(
        contents(&cs.file("missing.txt")),
        contents(&rs_.file("missing.txt"))
    );

    let bad = |s: &Side| s.path("no_such_dir/x.txt");
    // SAFETY: C strings and a readable buffer.
    unsafe {
        assert_eq!(
            c::oracle_os_quick_write_utf8_file(bad(&cs).as_ptr(), c"a".as_ptr(), 1, true),
            rs::os_quick_write_utf8_file(bad(&rs_).as_ptr(), c"a".as_ptr(), 1, true)
        );
    }
}

#[test]
fn seek_tell_and_size_match() {
    let (cs, rs_) = pair("platform-file-seek");
    let data = b"0123456789abcdef";
    write_raw(&cs.file("s.txt"), data);
    write_raw(&rs_.file("s.txt"), data);
    let (cp, rp) = (cs.path("s.txt"), rs_.path("s.txt"));

    let beyond: i64 = 5 * 1024 * 1024 * 1024 + 7;
    let steps: &[(i64, c_int)] = &[
        (0, SEEK_SET),
        (4, SEEK_SET),
        (3, SEEK_CUR),
        (-2, SEEK_CUR),
        (-3, SEEK_END),
        (0, SEEK_END),
        (100, SEEK_SET),
        (beyond, SEEK_SET),
        (-1, SEEK_SET),
        (-100, SEEK_END),
    ];
    for opener in [c::oracle_os_fopen as Fopen, rs::os_fopen] {
        // SAFETY: C strings.
        let (fc, fr) = unsafe {
            (
                opener(cp.as_ptr(), c"rb".as_ptr()),
                opener(rp.as_ptr(), c"rb".as_ptr()),
            )
        };
        assert!(!fc.is_null() && !fr.is_null());
        for &(offset, origin) in steps {
            // SAFETY: both streams are open.
            unsafe {
                assert_eq!(
                    c::oracle_os_fseeki64(fc, offset, origin),
                    rs::os_fseeki64(fr, offset, origin),
                    "os_fseeki64({offset}, {origin})"
                );
                assert_eq!(
                    c::oracle_os_ftelli64(fc),
                    rs::os_ftelli64(fr),
                    "after ({offset}, {origin})"
                );
                assert_eq!(
                    c::oracle_os_fgetsize(fc),
                    rs::os_fgetsize(fr),
                    "after ({offset}, {origin})"
                );
                assert_eq!(
                    c::oracle_os_ftelli64(fc),
                    rs::os_ftelli64(fr),
                    "fgetsize restores"
                );
            }
        }
        close(fc);
        close(fr);
    }
}

/// Every combination of temporary and backup extension, with and without
/// an existing target: the return value and the target, temporary and
/// backup files afterwards.
#[test]
fn safe_write_matches_the_c_file() {
    let exts: [Option<&CStr>; 4] = [None, Some(c""), Some(c"tmp"), Some(c".tmp")];
    let backups: [Option<&CStr>; 4] = [None, Some(c""), Some(c"bak"), Some(c".bak")];
    let opt = |e: Option<&CStr>| e.map_or(ptr::null(), CStr::as_ptr);
    for existing in [false, true] {
        for temp in exts {
            for backup in backups {
                for marker in [false, true] {
                    let (cs, rs_) = pair("platform-file-safe");
                    if existing {
                        write_raw(&cs.file("t.txt"), b"old");
                        write_raw(&rs_.file("t.txt"), b"old");
                    }
                    let (cp, rp) = (cs.path("t.txt"), rs_.path("t.txt"));
                    // SAFETY: C strings or NULL, and a readable buffer.
                    let (o, r) = unsafe {
                        (
                            c::oracle_os_quick_write_utf8_file_safe(
                                cp.as_ptr(),
                                c"new".as_ptr(),
                                3,
                                marker,
                                opt(temp),
                                opt(backup),
                            ),
                            rs::os_quick_write_utf8_file_safe(
                                rp.as_ptr(),
                                c"new".as_ptr(),
                                3,
                                marker,
                                opt(temp),
                                opt(backup),
                            ),
                        )
                    };
                    let case = format!(
                        "existing {existing}, temp {temp:?}, backup {backup:?}, marker {marker}"
                    );
                    assert_eq!(o, r, "{case}");
                    for name in ["t.txt", "t.txt.tmp", "t.txt.bak", "t.txt."] {
                        assert_eq!(
                            contents(&cs.file(name)),
                            contents(&rs_.file(name)),
                            "{name}: {case}"
                        );
                    }
                }
            }
        }
    }
}

/// The safe write fails the same way when the temporary file cannot be
/// written (its directory does not exist).
#[test]
fn safe_write_failure_matches() {
    let (cs, rs_) = pair("platform-file-safe-fail");
    let (cp, rp) = (cs.path("no_such_dir/t.txt"), rs_.path("no_such_dir/t.txt"));
    // SAFETY: C strings and a readable buffer.
    unsafe {
        assert_eq!(
            c::oracle_os_quick_write_utf8_file_safe(
                cp.as_ptr(),
                c"x".as_ptr(),
                1,
                false,
                c"tmp".as_ptr(),
                ptr::null()
            ),
            rs::os_quick_write_utf8_file_safe(
                rp.as_ptr(),
                c"x".as_ptr(),
                1,
                false,
                c"tmp".as_ptr(),
                ptr::null()
            )
        );
    }
}

#[test]
fn wfopen_matches_the_c_file() {
    type WChar = obs_util::platform_file::WChar;
    let wide = |s: &str| -> Vec<WChar> {
        let mut w: Vec<WChar> = if size_of::<WChar>() == 2 {
            s.encode_utf16().map(|u| u as WChar).collect()
        } else {
            s.chars().map(|ch| ch as u32 as WChar).collect()
        };
        w.push(0);
        w
    };
    for name in [
        "plain.txt",
        "caf\u{e9}.txt",
        "\u{65e5}\u{672c}.txt",
        "\u{1f600}.txt",
    ] {
        let (cs, rs_) = pair("platform-file-wfopen");
        write_raw(&cs.file(name), b"wide");
        write_raw(&rs_.file(name), b"wide");
        for (mode, missing) in [(c"rb", false), (c"rb", true), (c"ab", false)] {
            let target = |s: &Side| {
                let p = s.file(if missing { "missing" } else { name });
                wide(p.to_str().unwrap())
            };
            let (wc, wr) = (target(&cs), target(&rs_));
            // SAFETY: NUL-terminated wide strings and a C mode.
            let (fc, fr) = unsafe {
                (
                    c::oracle_os_wfopen(wc.as_ptr().cast(), mode.as_ptr()),
                    rs::os_wfopen(wr.as_ptr(), mode.as_ptr()),
                )
            };
            assert_eq!(
                fc.is_null(),
                fr.is_null(),
                "os_wfopen({name}, {mode:?}, missing {missing})"
            );
            if !fc.is_null() {
                // SAFETY: both streams are open.
                unsafe { assert_eq!(c::oracle_os_fgetsize(fc), rs::os_fgetsize(fr)) };
                close(fc);
                close(fr);
            }
        }
    }
}

#[cfg(windows)]
#[test]
fn stat_matches_the_c_file() {
    let (cs, rs_) = pair("platform-file-stat");
    write_raw(&cs.file("s.txt"), b"abcdef");
    write_raw(&rs_.file("s.txt"), b"abcdef");
    for name in ["s.txt", "missing.txt", ""] {
        let (cp, rp) = (cs.path(name), rs_.path(name));
        let mut sc = c::Stat::default();
        let mut sr = obs_util::platform::windows::file::Stat::default();
        // SAFETY: C strings and writable `Stat`s.
        let (o, r) = unsafe {
            (
                c::oracle_os_stat(cp.as_ptr(), &mut sc),
                rs::os_stat(rp.as_ptr(), &mut sr),
            )
        };
        assert_eq!(o, r, "os_stat({name:?})");
        // The directories differ, so compare what does not depend on them.
        assert_eq!(
            (sc.st_mode, sc.st_size, sc.st_nlink),
            (sr.st_mode, sr.st_size, sr.st_nlink),
            "{name:?}"
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn random_reads_match(bom in any::<bool>(), body in proptest::collection::vec(any::<u8>(), 0..300)) {
        let mut data = if bom { b"\xEF\xBB\xBF".to_vec() } else { Vec::new() };
        data.extend_from_slice(&body);
        assert_read_same(&data);
    }

    #[test]
    fn random_writes_match(body in proptest::collection::vec(any::<u8>(), 0..300), marker in any::<bool>()) {
        assert_write_utf8_same(&body, marker);
    }

    #[test]
    fn random_mbs_writes_match(text in "[ -~\u{e9}\u{65e5}]{0,64}", use_len in any::<bool>()) {
        let bytes = text.as_bytes();
        assert_write_mbs_same(bytes, if use_len { bytes.len() } else { 0 });
    }
}
