//! Tier 3: the Rust-backed `blog` / `blogva` path matches `libobs/util/base.c`.
//!
//! `blog` and `bcrash` are still C (`libobs/util/base-variadic.c`) because
//! stable Rust cannot define a variadic `extern "C"` function. The handler
//! slot is Rust. No intentional difference on one thread. Concurrent
//! `base_set_log_handler` is mutex-ordered in Rust and a plain store in C.
//!
//! `bcrash` itself is not called here: it exits the process. The once-flag
//! is what `test_base.c` pins, and that is compared below.

use std::ffi::{CString, c_char, c_int, c_void};
use std::ptr;

use obs_c_oracle::base as c;
use obs_util::base::{
    LOG_DEBUG, LOG_ERROR, LOG_INFO, LOG_LINE_CAP, LOG_WARNING, LogStream, default_log_format,
};
use obs_util::ffi::base as rs;
use proptest::prelude::*;

/// The log handler is process-global; profiler_io_parity.rs replaces it too.
use crate::common::LOG_HANDLER_LOCK as GATE;

#[repr(C)]
struct Rec {
    calls: c_int,
    level: c_int,
    message: [c_char; 256],
}

impl Rec {
    fn new() -> Self {
        Self {
            calls: 0,
            level: 0,
            message: [0; 256],
        }
    }

    fn text(&self) -> String {
        let bytes: Vec<u8> = self.message.iter().map(|b| *b as u8).collect();
        let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
        String::from_utf8_lossy(&bytes[..end]).into_owned()
    }
}

unsafe extern "C" {
    fn base_test_record_ptr() -> *mut c_void;
    fn base_test_flush();
    fn base_test_rust_blog_i(level: c_int, fmt: *const c_char, v: c_int);
    fn base_test_oracle_blog_i(level: c_int, fmt: *const c_char, v: c_int);
    fn base_test_rust_blog_s(level: c_int, fmt: *const c_char, s: *const c_char);
    fn base_test_oracle_blog_s(level: c_int, fmt: *const c_char, s: *const c_char);
    fn base_test_rust_blog_ss(level: c_int, fmt: *const c_char, a: *const c_char, b: *const c_char);
    fn base_test_oracle_blog_ss(
        level: c_int,
        fmt: *const c_char,
        a: *const c_char,
        b: *const c_char,
    );
    fn base_test_rust_blog_0(level: c_int, fmt: *const c_char);
    fn base_test_oracle_blog_0(level: c_int, fmt: *const c_char);
    fn base_test_rust_blogva_si(level: c_int, fmt: *const c_char, s: *const c_char, v: c_int);
    fn base_test_oracle_blogva_si(level: c_int, fmt: *const c_char, s: *const c_char, v: c_int);
}

fn lock() -> std::sync::MutexGuard<'static, ()> {
    GATE.lock().unwrap_or_else(|err| err.into_inner())
}

struct Saved {
    rust_handler: *mut c_void,
    rust_param: *mut c_void,
    oracle_handler: *mut c_void,
    oracle_param: *mut c_void,
}

impl Saved {
    fn capture() -> Self {
        let mut saved = Self {
            rust_handler: ptr::null_mut(),
            rust_param: ptr::null_mut(),
            oracle_handler: ptr::null_mut(),
            oracle_param: ptr::null_mut(),
        };
        // SAFETY: the out-pointers are live locals.
        unsafe {
            rs::base_get_log_handler(&mut saved.rust_handler, &mut saved.rust_param);
            c::oracle_base_get_log_handler(&mut saved.oracle_handler, &mut saved.oracle_param);
        }
        saved
    }
}

impl Drop for Saved {
    fn drop(&mut self) {
        // SAFETY: the pointers were read from the same setters.
        unsafe {
            rs::base_set_log_handler(self.rust_handler, self.rust_param);
            c::oracle_base_set_log_handler(self.oracle_handler, self.oracle_param);
        }
    }
}

fn arm(rust_rec: &mut Rec, oracle_rec: &mut Rec) {
    // SAFETY: `base_test_record` writes one `Rec`, which is #[repr(C)] and
    // matches `struct base_test_rec`. Both records outlive the handlers.
    unsafe {
        let handler = base_test_record_ptr();
        rs::base_set_log_handler(handler, (rust_rec as *mut Rec).cast());
        c::oracle_base_set_log_handler(handler, (oracle_rec as *mut Rec).cast());
    }
}

fn cs(text: &str) -> CString {
    CString::new(text).expect("no interior nul")
}

#[test]
fn null_get_pointers_are_accepted() {
    if !crate::common::isolated(concat!(module_path!(), "::null_get_pointers_are_accepted")) {
        return;
    }
    let _gate = lock();
    // SAFETY: both out-pointers are null. The functions return without writing.
    unsafe {
        rs::base_get_log_handler(ptr::null_mut(), ptr::null_mut());
        c::oracle_base_get_log_handler(ptr::null_mut(), ptr::null_mut());
    }
}

#[test]
fn cmocka_cases_match_oracle() {
    if !crate::common::isolated(concat!(module_path!(), "::cmocka_cases_match_oracle")) {
        return;
    }
    let _gate = lock();
    let mut ours = Rec::new();
    let mut theirs = Rec::new();
    let _saved = Saved::capture();
    arm(&mut ours, &mut theirs);

    let x = cs("x=%d");
    let pair = cs("%s-%s");
    let plain = cs("plain");
    let va = cs("%s:%03d");
    let a = cs("a");
    let b = cs("b");
    let id = cs("id");
    // SAFETY: formats and strings outlive the calls. Neither side retains them.
    unsafe {
        base_test_rust_blog_i(LOG_INFO, x.as_ptr(), 5);
        base_test_oracle_blog_i(LOG_INFO, x.as_ptr(), 5);
        base_test_rust_blog_ss(LOG_WARNING, pair.as_ptr(), a.as_ptr(), b.as_ptr());
        base_test_oracle_blog_ss(LOG_WARNING, pair.as_ptr(), a.as_ptr(), b.as_ptr());
        base_test_rust_blog_0(LOG_ERROR, plain.as_ptr());
        base_test_oracle_blog_0(LOG_ERROR, plain.as_ptr());
        base_test_rust_blogva_si(LOG_DEBUG, va.as_ptr(), id.as_ptr(), 7);
        base_test_oracle_blogva_si(LOG_DEBUG, va.as_ptr(), id.as_ptr(), 7);
    }

    assert_eq!(ours.calls, 4);
    assert_eq!(theirs.calls, 4);
    assert_eq!(ours.level, LOG_DEBUG);
    assert_eq!(theirs.level, LOG_DEBUG);
    assert_eq!(ours.text(), "id:007");
    assert_eq!(theirs.text(), ours.text());
}

#[test]
fn null_handler_falls_back_to_default() {
    if !crate::common::isolated(concat!(
        module_path!(),
        "::null_handler_falls_back_to_default"
    )) {
        return;
    }
    let _gate = lock();
    let mut marker = 1u8;
    let _saved = Saved::capture();
    // SAFETY: the pointer is the process-lifetime `base_test_record` function.
    let record = unsafe { base_test_record_ptr() };
    // SAFETY: marker is live. Null handler means "default" on both sides.
    unsafe {
        rs::base_set_log_handler(record, ptr::null_mut());
        rs::base_set_log_handler(ptr::null_mut(), (&mut marker as *mut u8).cast());
        let mut handler = ptr::null_mut();
        let mut param = ptr::null_mut();
        rs::base_get_log_handler(&mut handler, &mut param);
        assert!(!handler.is_null());
        assert_ne!(handler, record);
        assert_eq!(param, (&mut marker as *mut u8).cast());

        c::oracle_base_set_log_handler(ptr::null_mut(), (&mut marker as *mut u8).cast());
        let mut ohandler = ptr::null_mut();
        let mut oparam = ptr::null_mut();
        c::oracle_base_get_log_handler(&mut ohandler, &mut oparam);
        assert!(!ohandler.is_null());
        assert_ne!(ohandler, record);
        assert_eq!(oparam, param);
    }
}

#[test]
fn default_handler_output_matches_oracle() {
    if !crate::common::isolated(concat!(
        module_path!(),
        "::default_handler_output_matches_oracle"
    )) {
        return;
    }
    let _gate = lock();
    let _saved = Saved::capture();
    // SAFETY: null handler and null param restore the default on each side.
    // Declared after the gate and before any redirected I/O. No record lives
    // in the handler slot here.
    unsafe {
        rs::base_set_log_handler(ptr::null_mut(), ptr::null_mut());
        c::oracle_base_set_log_handler(ptr::null_mut(), ptr::null_mut());
    }

    for (level, text) in [
        (LOG_DEBUG, "hello"),
        (LOG_INFO, "hello"),
        (LOG_WARNING, "hello"),
        (LOG_ERROR, "hello"),
        (0, "hello"),
    ] {
        let (rust_out, rust_err) = blog_output("rust", level, text);
        let (c_out, c_err) = blog_output("oracle", level, text);
        assert_eq!(rust_out, c_out, "stdout level {level}");
        assert_eq!(rust_err, c_err, "stderr level {level}");
        match default_log_format(level) {
            None => {
                assert!(rust_out.is_empty());
                assert!(rust_err.is_empty());
            }
            Some((pattern, stream)) => {
                let line = pattern.replacen("%s", text, 1).into_bytes();
                match stream {
                    LogStream::Stdout => {
                        assert_eq!(strip_cr(&rust_out), line);
                        assert!(strip_cr(&rust_err).is_empty());
                    }
                    LogStream::Stderr => {
                        assert_eq!(strip_cr(&rust_err), line);
                        assert!(strip_cr(&rust_out).is_empty());
                    }
                }
            }
        }
    }
}

#[test]
fn default_handler_truncates_like_c() {
    if !crate::common::isolated(concat!(
        module_path!(),
        "::default_handler_truncates_like_c"
    )) {
        return;
    }
    let _gate = lock();
    let _saved = Saved::capture();
    // SAFETY: null handler and null param select the default on each side.
    unsafe {
        rs::base_set_log_handler(ptr::null_mut(), ptr::null_mut());
        c::oracle_base_set_log_handler(ptr::null_mut(), ptr::null_mut());
    }
    let long = "a".repeat(LOG_LINE_CAP + 64);
    let (rust_out, _) = blog_output("rust", LOG_INFO, &long);
    let (c_out, _) = blog_output("oracle", LOG_INFO, &long);
    assert_eq!(rust_out, c_out);
    let shown = LOG_LINE_CAP - 1;
    let mut expect = b"info: ".to_vec();
    expect.extend(std::iter::repeat_n(b'a', shown));
    expect.push(b'\n');
    assert_eq!(strip_cr(&rust_out), expect);
}

#[test]
fn second_crash_handler_matches_oracle() {
    if !crate::common::isolated(concat!(
        module_path!(),
        "::second_crash_handler_matches_oracle"
    )) {
        return;
    }
    let _gate = lock();
    let mut ours = Rec::new();
    let mut theirs = Rec::new();
    let _saved = Saved::capture();
    arm(&mut ours, &mut theirs);
    // SAFETY: the pointer is `base_test_record`. It is stored, not called.
    let handler = unsafe { base_test_record_ptr() };
    // SAFETY: the handler is never invoked. bcrash is not called, so a
    // log-handler pointer standing in for the crash handler is not called.
    unsafe {
        rs::base_set_crash_handler(handler, ptr::null_mut());
        c::oracle_base_set_crash_handler(handler, ptr::null_mut());
    }
    assert_eq!(ours.calls, 0);
    assert_eq!(theirs.calls, 0);
    // SAFETY: same stored pointer as the first call. Still not invoked.
    unsafe {
        rs::base_set_crash_handler(handler, ptr::null_mut());
        c::oracle_base_set_crash_handler(handler, ptr::null_mut());
    }
    assert_eq!(ours.calls, 1);
    assert_eq!(theirs.calls, 1);
    assert_eq!(ours.level, LOG_WARNING);
    assert_eq!(theirs.level, LOG_WARNING);
    assert_eq!(
        ours.text(),
        "Tried to set a crash handler when one already exists."
    );
    assert_eq!(theirs.text(), ours.text());
}

#[test]
fn formatted_int_matches_oracle() {
    if !crate::common::isolated(concat!(module_path!(), "::formatted_int_matches_oracle")) {
        return;
    }
    proptest! {
        |(level in any::<i32>(), n in any::<i32>())| {
            let _gate = lock();
            let mut ours = Rec::new();
            let mut theirs = Rec::new();
            let _saved = Saved::capture();
            arm(&mut ours, &mut theirs);
            let fmt = cs("n=%d");
            // SAFETY: fmt outlives both calls.
            unsafe {
                base_test_rust_blog_i(level, fmt.as_ptr(), n);
                base_test_oracle_blog_i(level, fmt.as_ptr(), n);
            }
            assert_eq!(ours.calls, theirs.calls);
            assert_eq!(ours.level, theirs.level);
            assert_eq!(ours.text(), theirs.text());
            assert_eq!(ours.level, level);
        }
    }
}

struct RestoreFds {
    old_out: c_int,
    old_err: c_int,
}

impl Drop for RestoreFds {
    fn drop(&mut self) {
        // SAFETY: these are the fds `capture` saved with `dup`. Restoring
        // them puts the process stdio back even when the test panics.
        unsafe {
            base_test_flush();
            libc::dup2(self.old_out, 1);
            libc::dup2(self.old_err, 2);
            libc::close(self.old_out);
            libc::close(self.old_err);
        }
    }
}

/// Default-handler bytes for one `blog("%s", body)` call.
///
/// libtest writes `test <name> ...` to fd 1 before the test body runs, so
/// `dup2` in this process also catches the harness. The redirect runs in a
/// one-test child instead, and the child writes the bytes to files.
fn blog_output(which: &str, level: c_int, body: &str) -> (Vec<u8>, Vec<u8>) {
    let dir = std::env::temp_dir();
    let token = format!("{}-{}-{}-{level}", std::process::id(), which, body.len());
    let out_path = dir.join(format!("obs-base-out-{token}"));
    let err_path = dir.join(format!("obs-base-err-{token}"));
    let exe = std::env::current_exe().expect("test executable");
    let output = std::process::Command::new(exe)
        .args([
            "--exact",
            crate::common::test_name(concat!(module_path!(), "::default_handler_capture_worker")),
            "--test-threads=1",
        ])
        .env("BASE_CAPTURE_WHICH", which)
        .env("BASE_CAPTURE_LEVEL", level.to_string())
        .env("BASE_CAPTURE_BODY", body)
        .env("BASE_CAPTURE_OUT", &out_path)
        .env("BASE_CAPTURE_ERR", &err_path)
        .output()
        .expect("spawn capture worker");
    let out = std::fs::read(&out_path).unwrap_or_default();
    let err = std::fs::read(&err_path).unwrap_or_default();
    let _ = std::fs::remove_file(&out_path);
    let _ = std::fs::remove_file(&err_path);
    assert!(
        output.status.success(),
        "capture worker failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    (out, err)
}

#[test]
fn default_handler_capture_worker() {
    let Ok(which) = std::env::var("BASE_CAPTURE_WHICH") else {
        return;
    };
    let level: c_int = std::env::var("BASE_CAPTURE_LEVEL")
        .expect("level")
        .parse()
        .expect("level integer");
    let body = std::env::var("BASE_CAPTURE_BODY").expect("body");
    let out_path = std::env::var("BASE_CAPTURE_OUT").expect("stdout path");
    let err_path = std::env::var("BASE_CAPTURE_ERR").expect("stderr path");
    // SAFETY: null handler and null param select the default on each side.
    unsafe {
        rs::base_set_log_handler(ptr::null_mut(), ptr::null_mut());
        c::oracle_base_set_log_handler(ptr::null_mut(), ptr::null_mut());
    }
    let fmt = cs("%s");
    let text = cs(&body);
    let (out, err) = capture(|| {
        // SAFETY: fmt and text outlive the call. The default handler does not keep them.
        unsafe {
            match which.as_str() {
                "rust" => base_test_rust_blog_s(level, fmt.as_ptr(), text.as_ptr()),
                "oracle" => base_test_oracle_blog_s(level, fmt.as_ptr(), text.as_ptr()),
                other => panic!("unknown capture side {other}"),
            }
        }
    });
    std::fs::write(&out_path, out).expect("write stdout capture");
    std::fs::write(&err_path, err).expect("write stderr capture");
}

/// Drop `\r` inserted by a text-mode C runtime. The handler writes `\n`.
/// Both sides go through the same runtime, so the raw buffers are compared
/// separately.
fn strip_cr(bytes: &[u8]) -> Vec<u8> {
    bytes.iter().copied().filter(|b| *b != b'\r').collect()
}

fn open_trunc(path: &std::path::Path) -> c_int {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        let bytes = path.as_os_str().as_bytes();
        let mut buf = Vec::with_capacity(bytes.len() + 1);
        buf.extend_from_slice(bytes);
        buf.push(0);
        // SAFETY: `buf` is a NUL-terminated path. Create or truncate the file.
        unsafe {
            libc::open(
                buf.as_ptr().cast(),
                libc::O_RDWR | libc::O_CREAT | libc::O_TRUNC,
                0o666,
            )
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        // SAFETY: `wide` is a NUL-terminated UTF-16 path. `O_BINARY` keeps
        // the handler's `\n` from being translated while the fd is swapped.
        unsafe {
            libc::wopen(
                wide.as_ptr(),
                libc::O_RDWR | libc::O_CREAT | libc::O_TRUNC | libc::O_BINARY,
                0o666,
            )
        }
    }
}

fn capture(f: impl FnOnce()) -> (Vec<u8>, Vec<u8>) {
    let token = format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    );
    let dir = std::env::temp_dir();
    let out_path = dir.join(format!("obs-base-cap-out-{token}"));
    let err_path = dir.join(format!("obs-base-cap-err-{token}"));
    let out_fd = open_trunc(&out_path);
    let err_fd = open_trunc(&err_path);
    assert!(out_fd >= 0 && err_fd >= 0, "open capture files");
    // SAFETY: swap fds 1 and 2 onto those files for `f`, then restore them.
    // A file has no pipe-sized buffer, so a long log line cannot block.
    unsafe {
        base_test_flush();
        let old_out = libc::dup(1);
        let old_err = libc::dup(2);
        assert!(old_out >= 0 && old_err >= 0);
        // POSIX dup2 returns the new fd; the MSVC CRT returns 0. Both
        // return -1 on failure.
        assert_ne!(libc::dup2(out_fd, 1), -1);
        assert_ne!(libc::dup2(err_fd, 2), -1);
        assert_eq!(libc::close(out_fd), 0);
        assert_eq!(libc::close(err_fd), 0);
        let restore = RestoreFds { old_out, old_err };
        f();
        drop(restore);
    }
    let out = std::fs::read(&out_path).unwrap_or_default();
    let err = std::fs::read(&err_path).unwrap_or_default();
    let _ = std::fs::remove_file(&out_path);
    let _ = std::fs::remove_file(&err_path);
    (out, err)
}
