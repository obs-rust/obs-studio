//! Tier 1: 1:1 port of `test/cmocka/test_platform_file.c` and of the
//! `platform-file.c` cases in `test/cmocka/test_platform.c`
//! (`test_utf8_*`, `test_fread_utf8_returns_zero_length`,
//! `test_mbs_roundtrip_ascii`, `test_get_file_size_missing`,
//! `test_fopen_seek_tell`), through the C ABI shims. Each test names the
//! cmocka case it mirrors.

use core::ffi::{CStr, c_char, c_void};
use core::ptr::{self, NonNull};
use std::ffi::CString;
use std::path::Path;

use obs_c_oracle as _; // links the test allocator
use obs_util::ffi::bmem::bfree;
use obs_util::ffi::platform_file::*;
use obs_util::platform_file::{CFile, SEEK_CUR, SEEK_END, SEEK_SET};

use crate::common::ScratchDir;

fn c(path: &Path) -> CString {
    CString::new(path.to_str().expect("utf-8 temp path")).unwrap()
}

fn write_str(path: &CStr, s: &str) -> bool {
    // SAFETY: `path` is a C string and `s` is readable for its length.
    unsafe { os_quick_write_utf8_file(path.as_ptr(), s.as_ptr().cast(), s.len(), false) }
}

/// Takes ownership of a `bmalloc`ed C string: its text, or `None` for NULL.
fn take(s: *mut c_char) -> Option<String> {
    if s.is_null() {
        return None;
    }
    // SAFETY: the shims return NUL-terminated `bmalloc`ed strings.
    let text = unsafe { CStr::from_ptr(s) }.to_str().unwrap().to_owned();
    // SAFETY: `s` came from `bmalloc`.
    unsafe { bfree(s.cast()) };
    Some(text)
}

fn read_utf8(path: &CStr) -> Option<String> {
    // SAFETY: `path` is a C string.
    take(unsafe { os_quick_read_utf8_file(path.as_ptr()) })
}

fn assert_file_content(path: &CStr, expected: &str) {
    assert_eq!(read_utf8(path).as_deref(), Some(expected));
}

fn size(path: &CStr) -> i64 {
    // SAFETY: `path` is a C string.
    unsafe { os_get_file_size(path.as_ptr()) }
}

fn fopen(path: &CStr, mode: &CStr) -> *mut c_void {
    // SAFETY: both are C strings.
    unsafe { os_fopen(path.as_ptr(), mode.as_ptr()) }
}

fn fclose(file: *mut c_void) {
    // SAFETY: `file` is an open stream from `os_fopen`, closed once here.
    assert!(unsafe { CFile::from_raw(NonNull::new(file).unwrap()) }.close());
}

/// `fread(buf, 1, n, file)` through the same C library stream.
fn fread(file: *mut c_void, n: usize) -> Vec<u8> {
    // SAFETY: `file` is an open stream the caller still owns.
    let mut f =
        core::mem::ManuallyDrop::new(unsafe { CFile::from_raw(NonNull::new(file).unwrap()) });
    let mut buf = vec![0u8; n];
    let got = f.read(&mut buf);
    buf.truncate(got);
    buf
}

/// `s` as a NUL-terminated `wchar_t` string (UTF-16 on Windows, UTF-32
/// elsewhere).
fn utf8_to_wide(s: &str) -> Vec<WCharT> {
    let mut wide: Vec<WCharT> = if size_of::<WCharT>() == 2 {
        s.encode_utf16().map(|u| u as WCharT).collect()
    } else {
        s.chars().map(|ch| ch as u32 as WCharT).collect()
    };
    wide.push(0);
    wide
}

type WCharT = obs_util::platform_file::WChar;

// test_platform_file.c

#[test]
fn test_fopen_null_path() {
    // SAFETY: NULL paths are checked.
    unsafe {
        assert!(os_fopen(ptr::null(), c"rb".as_ptr()).is_null());
        assert!(os_wfopen(ptr::null(), c"rb".as_ptr()).is_null());
    }
}

#[test]
fn test_wfopen_non_ascii_path() {
    let dir = ScratchDir::new("platform-file-wide");
    let path = dir.join("wide_\u{fc}.txt");
    assert!(write_str(&c(&path), "wide"));

    let wpath = utf8_to_wide(path.to_str().unwrap());
    // SAFETY: `wpath` is NUL-terminated.
    let f = unsafe { os_wfopen(wpath.as_ptr(), c"rb".as_ptr()) };
    assert!(!f.is_null());
    assert_eq!(fread(f, 7), b"wide");
    fclose(f);
}

#[test]
fn test_wfopen_missing() {
    let dir = ScratchDir::new("platform-file-wmissing");
    let wpath = utf8_to_wide(dir.join("missing.txt").to_str().unwrap());
    // SAFETY: `wpath` is NUL-terminated.
    assert!(unsafe { os_wfopen(wpath.as_ptr(), c"rb".as_ptr()) }.is_null());
}

#[test]
fn test_fread_mbs_ascii() {
    let dir = ScratchDir::new("platform-file-mbs");
    let path = c(&dir.join("mbs.txt"));
    // SAFETY: C strings and a readable buffer.
    assert!(unsafe { os_quick_write_mbs_file(path.as_ptr(), c"hello".as_ptr(), 5) });

    let f = fopen(&path, c"rb");
    assert!(!f.is_null());

    // reads from the start even when the position is elsewhere
    // SAFETY: `f` is open.
    assert_eq!(unsafe { os_fseeki64(f, 2, SEEK_SET) }, 0);

    let mut s = ptr::null_mut();
    // SAFETY: `f` is open and `s` is writable.
    assert_eq!(unsafe { os_fread_mbs(f, &mut s) }, 5);
    assert_eq!(take(s).as_deref(), Some("hello"));

    // characterized, not endorsed: the position is left at the end
    // SAFETY: `f` is open.
    assert_eq!(unsafe { os_ftelli64(f) }, 5);
    fclose(f);
}

#[test]
fn test_fread_mbs_empty() {
    let dir = ScratchDir::new("platform-file-mbs-empty");
    let path = c(&dir.join("empty_mbs.txt"));
    let missing = c(&dir.join("missing.txt"));
    // SAFETY: C strings.
    assert!(unsafe { os_quick_write_mbs_file(path.as_ptr(), c"".as_ptr(), 0) });
    assert_eq!(size(&path), 0);

    let f = fopen(&path, c"rb");
    assert!(!f.is_null());
    let mut s = c"untouched".as_ptr().cast_mut();
    // SAFETY: `f` is open and `s` is writable.
    assert_eq!(unsafe { os_fread_mbs(f, &mut s) }, 0);
    assert!(s.is_null());
    fclose(f);

    // SAFETY: C strings.
    unsafe {
        assert_eq!(take(os_quick_read_mbs_file(path.as_ptr())), None);
        assert_eq!(take(os_quick_read_mbs_file(missing.as_ptr())), None);
    }
}

#[test]
fn test_fgetsize_restores_position() {
    let dir = ScratchDir::new("platform-file-size");
    let path = c(&dir.join("size.txt"));
    assert!(write_str(&path, "0123456789"));

    let f = fopen(&path, c"rb");
    assert!(!f.is_null());
    // SAFETY: `f` is open.
    unsafe {
        assert_eq!(os_fseeki64(f, 4, SEEK_SET), 0);
        assert_eq!(os_fgetsize(f), 10);
        assert_eq!(os_ftelli64(f), 4);
    }
    assert_eq!(fread(f, 1), b"4");
    fclose(f);
}

#[test]
fn test_seek_tell_beyond_4gib() {
    let dir = ScratchDir::new("platform-file-seek");
    let path = c(&dir.join("seek.txt"));
    assert!(write_str(&path, "x"));

    let f = fopen(&path, c"rb");
    assert!(!f.is_null());

    // seeking past the end without writing does not grow the file
    let beyond: i64 = 5 * 1024 * 1024 * 1024 + 3;
    // SAFETY: `f` is open.
    unsafe {
        assert_eq!(os_fseeki64(f, beyond, SEEK_SET), 0);
        assert_eq!(os_ftelli64(f), beyond);

        assert_eq!(os_fseeki64(f, -2, SEEK_CUR), 0);
        assert_eq!(os_ftelli64(f), beyond - 2);

        assert_eq!(os_fseeki64(f, 0, SEEK_END), 0);
        assert_eq!(os_ftelli64(f), 1);
    }
    fclose(f);

    assert_eq!(size(&path), 1);
}

/// `test_stat`: Windows only, where `os_stat` is a libobs function (on Unix
/// it is the C library's `stat`).
#[cfg(windows)]
#[test]
fn test_stat() {
    use obs_util::platform::windows::file::Stat;

    const S_IFMT: u16 = 0xF000;
    const S_IFDIR: u16 = 0x4000;
    const S_IFREG: u16 = 0x8000;

    let dir = ScratchDir::new("platform-file-stat");
    let path = c(&dir.join("stat.txt"));
    let missing = c(&dir.join("missing.txt"));
    assert!(write_str(&path, "abcdef"));

    let mut st = Stat::default();
    // SAFETY: C strings and a writable `Stat`.
    unsafe {
        assert_eq!(os_stat(path.as_ptr(), &mut st), 0);
        assert_eq!(st.st_size, 6);
        assert_eq!(st.st_mode & S_IFMT, S_IFREG);

        assert_ne!(os_stat(missing.as_ptr(), &mut st), 0);

        st = Stat::default();
        assert_eq!(os_stat(c(&dir).as_ptr(), &mut st), 0);
        assert_eq!(st.st_mode & S_IFMT, S_IFDIR);
    }
}

#[test]
fn test_utf8_write_empty() {
    let dir = ScratchDir::new("platform-file-empty");
    let path = c(&dir.join("empty_utf8.txt"));

    // len 0 writes nothing; str is not read
    // SAFETY: a C path; `str` is not read for len 0.
    unsafe {
        assert!(os_quick_write_utf8_file(
            path.as_ptr(),
            ptr::null(),
            0,
            false
        ));
        assert_eq!(size(&path), 0);

        assert!(os_quick_write_utf8_file(
            path.as_ptr(),
            ptr::null(),
            0,
            true
        ));
        assert_eq!(size(&path), 3);
    }

    // characterized, not endorsed: a BOM-only file reads as NULL
    assert_eq!(read_utf8(&path), None);
}

#[test]
fn test_utf8_write_safe_ext_without_dot() {
    let dir = ScratchDir::new("platform-file-safe-dot");
    let path = c(&dir.join("safe.txt"));
    let tmp = dir.join("safe.txt.tmp");

    // SAFETY: C strings and readable buffers.
    unsafe {
        assert!(os_quick_write_utf8_file_safe(
            path.as_ptr(),
            c"v1".as_ptr(),
            2,
            false,
            c"tmp".as_ptr(),
            ptr::null()
        ));
    }
    assert_file_content(&path, "v1");

    // the temp file was "<path>.tmp" and has been renamed over the target
    assert!(!tmp.exists());

    // SAFETY: C strings and readable buffers.
    unsafe {
        assert!(!os_quick_write_utf8_file_safe(
            path.as_ptr(),
            c"v2".as_ptr(),
            2,
            false,
            c"".as_ptr(),
            ptr::null()
        ));
        assert!(!os_quick_write_utf8_file_safe(
            path.as_ptr(),
            c"v2".as_ptr(),
            2,
            false,
            ptr::null(),
            ptr::null()
        ));
    }
}

#[test]
fn test_get_file_size_null() {
    // SAFETY: NULL is checked.
    assert_eq!(unsafe { os_get_file_size(ptr::null()) }, -1);
}

// test_platform.c

#[test]
fn test_utf8_write_no_marker() {
    let dir = ScratchDir::new("platform-file-plain");
    let path = c(&dir.join("utf8_plain.txt"));
    // SAFETY: C strings and a readable buffer.
    assert!(unsafe { os_quick_write_utf8_file(path.as_ptr(), c"hello".as_ptr(), 5, false) });
    assert_eq!(size(&path), 5);
    assert_file_content(&path, "hello");
}

#[test]
fn test_utf8_write_marker_adds_bom_and_read_strips() {
    let dir = ScratchDir::new("platform-file-bom");
    let path = c(&dir.join("utf8_bom.txt"));
    // SAFETY: C strings and a readable buffer.
    assert!(unsafe { os_quick_write_utf8_file(path.as_ptr(), c"hello".as_ptr(), 5, true) });
    // BOM adds 3 bytes
    assert_eq!(size(&path), 8);
    // os_fread_utf8 skips a leading EF BB BF
    assert_file_content(&path, "hello");
}

#[test]
fn test_utf8_read_empty_and_bom_only_return_null() {
    let dir = ScratchDir::new("platform-file-empty-bom");
    let empty = c(&dir.join("utf8_empty.txt"));
    let bom_only = c(&dir.join("utf8_bomonly.txt"));

    // characterized, not endorsed: an empty file reads back as NULL, not ""
    // SAFETY: C strings.
    unsafe {
        assert!(os_quick_write_utf8_file(
            empty.as_ptr(),
            c"".as_ptr(),
            0,
            false
        ));
        assert_eq!(size(&empty), 0);
        assert_eq!(read_utf8(&empty), None);

        // a BOM with no payload is also NULL
        assert!(os_quick_write_utf8_file(
            bom_only.as_ptr(),
            c"".as_ptr(),
            0,
            true
        ));
        assert_eq!(size(&bom_only), 3);
        assert_eq!(read_utf8(&bom_only), None);
    }
}

#[test]
fn test_utf8_read_missing_returns_null() {
    let dir = ScratchDir::new("platform-file-missing");
    let missing = c(&dir.join("missing.txt"));
    assert_eq!(read_utf8(&missing), None);
    // SAFETY: a C string.
    assert_eq!(
        take(unsafe { os_quick_read_mbs_file(missing.as_ptr()) }),
        None
    );
}

#[test]
fn test_fread_utf8_returns_zero_length() {
    let dir = ScratchDir::new("platform-file-fread");
    let path = c(&dir.join("fread.txt"));
    assert!(write_str(&path, "abc"));

    let f = fopen(&path, c"rb");
    assert!(!f.is_null());
    let mut s = ptr::null_mut();
    // SAFETY: `f` is open and `s` is writable.
    let len = unsafe { os_fread_utf8(f, &mut s) };
    fclose(f);

    // characterized, not endorsed: the string is filled in but the
    // returned length is always 0
    assert_eq!(len, 0);
    assert_eq!(take(s).as_deref(), Some("abc"));
}

#[test]
fn test_utf8_write_to_bad_path_fails() {
    let dir = ScratchDir::new("platform-file-bad");
    let path = c(&dir.join("no_such_dir").join("x.txt"));
    // SAFETY: C strings and a readable buffer.
    assert!(!unsafe { os_quick_write_utf8_file(path.as_ptr(), c"a".as_ptr(), 1, false) });
}

fn write_safe(path: &CStr, s: &CStr, temp: Option<&CStr>, backup: Option<&CStr>) -> bool {
    let opt = |e: Option<&CStr>| e.map_or(ptr::null(), CStr::as_ptr);
    // SAFETY: C strings or NULL, and a readable buffer.
    unsafe {
        os_quick_write_utf8_file_safe(
            path.as_ptr(),
            s.as_ptr(),
            s.to_bytes().len(),
            false,
            opt(temp),
            opt(backup),
        )
    }
}

#[test]
fn test_utf8_write_safe_with_backup() {
    let dir = ScratchDir::new("platform-file-safe-bak");
    let safe = dir.join("safe.txt");
    let (tmp, bak) = (dir.join("safe.txt.tmp"), dir.join("safe.txt.bak"));
    let path = c(&safe);

    // no existing target: the temp file becomes the target, no backup
    assert!(write_safe(&path, c"one", Some(c"tmp"), Some(c"bak")));
    assert!(safe.exists());
    assert!(!tmp.exists());
    assert!(!bak.exists());
    assert_file_content(&path, "one");

    // existing target: old content moves to the backup; leading '.' in
    // the extension is optional
    assert!(write_safe(&path, c"two", Some(c".tmp"), Some(c".bak")));
    assert!(safe.exists());
    assert!(!tmp.exists());
    assert!(bak.exists());
    assert_file_content(&path, "two");
    assert_file_content(&c(&bak), "one");
}

#[test]
fn test_utf8_write_safe_without_backup() {
    let dir = ScratchDir::new("platform-file-safe-nobak");
    let path = c(&dir.join("safe.txt"));
    let (tmp, bak) = (dir.join("safe.txt.tmp"), dir.join("safe.txt.bak"));

    assert!(write_str(&path, "old"));
    assert!(write_safe(&path, c"new", Some(c"tmp"), None));
    assert_file_content(&path, "new");
    assert!(!tmp.exists());
    assert!(!bak.exists());

    // empty backup extension is treated like none
    assert!(write_safe(&path, c"newer", Some(c"tmp"), Some(c"")));
    assert_file_content(&path, "newer");
    assert!(!bak.exists());
}

#[test]
fn test_utf8_write_safe_invalid_temp_ext() {
    let dir = ScratchDir::new("platform-file-safe-ext");
    let safe = dir.join("safe.txt");
    let path = c(&safe);

    assert!(!write_safe(&path, c"x", None, Some(c"bak")));
    assert!(!write_safe(&path, c"x", Some(c""), Some(c"bak")));
    assert!(!safe.exists());
}

#[test]
fn test_mbs_roundtrip_ascii() {
    let dir = ScratchDir::new("platform-file-mbs-rt");
    let path = c(&dir.join("mbs.txt"));

    // SAFETY: C strings and readable buffers.
    unsafe {
        assert!(os_quick_write_mbs_file(
            path.as_ptr(),
            c"hello world".as_ptr(),
            11
        ));
        assert_eq!(size(&path), 11);
        assert_eq!(
            take(os_quick_read_mbs_file(path.as_ptr())).as_deref(),
            Some("hello world")
        );

        // len 0 means strlen
        assert!(os_quick_write_mbs_file(path.as_ptr(), c"abc".as_ptr(), 0));
        assert_eq!(size(&path), 3);
    }
}

#[test]
fn test_get_file_size_missing() {
    let dir = ScratchDir::new("platform-file-size-missing");
    assert_eq!(size(&c(&dir.join("missing.txt"))), -1);
}

#[test]
fn test_fopen_seek_tell() {
    let dir = ScratchDir::new("platform-file-fopen");
    let path = c(&dir.join("seek.txt"));
    let missing = c(&dir.join("missing.txt"));
    assert!(write_str(&path, "0123456789"));

    // SAFETY: NULL is checked.
    assert!(unsafe { os_fopen(ptr::null(), c"rb".as_ptr()) }.is_null());
    assert!(fopen(&missing, c"rb").is_null());

    let f = fopen(&path, c"rb");
    assert!(!f.is_null());

    // SAFETY: `f` is open.
    unsafe {
        assert_eq!(os_ftelli64(f), 0);
        assert_eq!(os_fseeki64(f, 4, SEEK_SET), 0);
        assert_eq!(os_ftelli64(f), 4);
    }
    assert_eq!(fread(f, 1), b"4");
    // SAFETY: `f` is open.
    unsafe {
        assert_eq!(os_ftelli64(f), 5);

        assert_eq!(os_fseeki64(f, -2, SEEK_END), 0);
        assert_eq!(os_ftelli64(f), 8);

        // os_fgetsize reports the size and restores the position
        assert_eq!(os_fgetsize(f), 10);
        assert_eq!(os_ftelli64(f), 8);
    }
    fclose(f);
}
