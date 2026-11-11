//! Tier 1 mirror of `test/cmocka/test_missing_files.c`.
//!
//! The source name uses the production Rust getter and a real-layout C fixture.
//! Only the missing-file replacement callback remains a test stand-in.
//!
//! Not called, because the C crashes or does not return: a null list, a
//! null file (except `release`), `get_file` past the end, append of a list
//! to itself, `issue_callback` of a source file whose callback is null and
//! whose source is live, and adding the same file twice.

use std::ffi::{CStr, c_char, c_void};
use std::process::Command;
use std::sync::Mutex;
use std::sync::atomic::{AtomicPtr, Ordering};

use obs_missing_files::ffi::missing_files::{
    obs_missing_file_create, obs_missing_file_destroy, obs_missing_file_get_path,
    obs_missing_file_get_source_name, obs_missing_file_issue_callback, obs_missing_file_release,
    obs_missing_files_add_file, obs_missing_files_append, obs_missing_files_count,
    obs_missing_files_create, obs_missing_files_destroy, obs_missing_files_get_file,
};
use obs_missing_files::{SCRIPT, SOURCE};
use obs_util::ffi::bmem::bnum_allocs;

mod parity;

const MARKER_ENV: &str = "OBS_MISSING_FILES_ISOLATED_TEST";

/// Serializes the real source-name fixture. Tier 1 children
/// each have their own process. Differential tests in this process share it.
pub(super) static SOURCE_LOCK: Mutex<()> = Mutex::new(());

pub(super) struct Hit {
    pub(super) n: u32,
    pub(super) src: usize,
    pub(super) path: usize,
    pub(super) data: usize,
}

static HIT: Mutex<Hit> = Mutex::new(Hit {
    n: 0,
    src: 0,
    path: 0,
    data: 0,
});

use obs_source::ffi::source_name::obs_source_get_name;
unsafe extern "C" {
    fn oracle_source_name_missing_source() -> *mut c_void;
    fn oracle_source_name_missing_set(name: *mut c_char);
}

pub(super) fn source_fixture() -> *mut c_void {
    // SAFETY: returns a static real-layout source, accessed under SOURCE_LOCK.
    unsafe { oracle_source_name_missing_source() }
}
static SOURCE_PRIVATE: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());

pub(super) fn set_source(name: *const c_char, private: *mut c_void) {
    // SAFETY: SOURCE_LOCK serializes the fixture; the test keeps name alive.
    unsafe { oracle_source_name_missing_set(name.cast_mut()) };
    SOURCE_PRIVATE.store(private, Ordering::SeqCst);
}

pub(super) fn allocs() -> i64 {
    // SAFETY: reads the process-wide counter.
    unsafe { bnum_allocs() as i64 }
}

fn cstr<'a>(ptr: *const c_char) -> &'a CStr {
    assert!(!ptr.is_null());
    // SAFETY: `ptr` is a NUL-terminated string the missing-file API returned
    // or the test owns.
    unsafe { CStr::from_ptr(ptr) }
}

pub(super) fn reset_hit() {
    *HIT.lock().expect("hit lock") = Hit {
        n: 0,
        src: 0,
        path: 0,
        data: 0,
    };
}

pub(super) fn hit() -> Hit {
    let guard = HIT.lock().expect("hit lock");
    Hit {
        n: guard.n,
        src: guard.src,
        path: guard.path,
        data: guard.data,
    }
}

pub(super) unsafe extern "C" fn on_replace(
    src: *mut c_void,
    path: *const c_char,
    data: *mut c_void,
) {
    let mut hit = HIT.lock().expect("hit lock");
    hit.n += 1;
    hit.src = src as usize;
    hit.path = path as usize;
    hit.data = data as usize;
}

/// `obs_source_replace_missing_file` for cargo tests. A null source returns.
/// Otherwise the callback is invoked with the private pointer from
/// [`set_source`], the path argument, and `data`.
///
/// # Safety
/// `cb` is null or a function the test registered. A null `cb` with a live
/// source is not called from these tests.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_replace_missing_file(
    cb: Option<unsafe extern "C" fn(*mut c_void, *const c_char, *mut c_void)>,
    source: *mut c_void,
    new_path: *const c_char,
    data: *mut c_void,
) {
    if source.is_null() {
        return;
    }
    if let Some(cb) = cb {
        // SAFETY: the test registered `cb` and keeps `data` alive.
        unsafe { cb(SOURCE_PRIVATE.load(Ordering::SeqCst), new_path, data) };
    }
}

fn test_name(test: &str) -> &str {
    test.strip_prefix(concat!(env!("CARGO_CRATE_NAME"), "::"))
        .unwrap_or(test)
}

fn isolated(test: &str) -> bool {
    let test = test_name(test);
    if std::env::var(MARKER_ENV).as_deref() == Ok(test) {
        return true;
    }
    let exe = std::env::current_exe().expect("path of the test binary");
    let out = Command::new(exe)
        .args([test, "--exact", "--test-threads=1", "--nocapture"])
        .env(MARKER_ENV, test)
        .output()
        .expect("start the isolated test process");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "isolated {test} failed:\n{stdout}\n{stderr}"
    );
    assert!(
        stdout.contains("1 passed"),
        "isolated {test} did not run exactly one test:\n{stdout}"
    );
    false
}

pub(super) fn child(name: &str) -> bool {
    isolated(name)
}

#[test]
fn empty_list_test() {
    if !child(concat!(module_path!(), "::empty_list_test")) {
        return;
    }
    let before = allocs();
    // SAFETY: create and destroy follow the C contract. The list is live.
    let files = unsafe { obs_missing_files_create() };
    assert!(!files.is_null());
    assert_eq!(unsafe { obs_missing_files_count(files) }, 0);
    assert_eq!(allocs(), before + 1);
    unsafe { obs_missing_files_destroy(files) };
    assert_eq!(allocs(), before);
}

#[test]
fn path_copy_test() {
    if !child(concat!(module_path!(), "::path_copy_test")) {
        return;
    }
    let before = allocs();
    let mut path = *b"shot.png\0";
    let path_ptr = path.as_mut_ptr().cast();
    // SAFETY: `path_ptr` is a NUL-terminated buffer this test owns.
    let file = unsafe {
        obs_missing_file_create(
            path_ptr,
            None,
            SCRIPT,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    path[0] = b'x';
    assert_eq!(path[0], b'x');
    let got = unsafe { obs_missing_file_get_path(file) };
    assert_eq!(cstr(got), c"shot.png");
    assert_ne!(got, path_ptr);
    assert!(unsafe { obs_missing_file_get_source_name(file) }.is_null());
    // struct + path copy. A script stores no source name.
    assert_eq!(allocs(), before + 2);
    unsafe { obs_missing_file_release(file) };
    assert_eq!(allocs(), before);
}

#[test]
fn null_and_empty_path_test() {
    if !child(concat!(module_path!(), "::null_and_empty_path_test")) {
        return;
    }
    let before = allocs();
    let null_path = unsafe {
        obs_missing_file_create(
            std::ptr::null(),
            None,
            SCRIPT,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    let empty_path = unsafe {
        obs_missing_file_create(
            c"".as_ptr(),
            None,
            SCRIPT,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    assert!(unsafe { obs_missing_file_get_path(null_path) }.is_null());
    let empty = unsafe { obs_missing_file_get_path(empty_path) };
    assert_eq!(cstr(empty), c"");
    assert_ne!(empty, c"".as_ptr());
    // null path is not allocated. The empty path is one byte plus the NUL.
    assert_eq!(allocs(), before + 3);
    unsafe { obs_missing_file_release(null_path) };
    unsafe { obs_missing_file_release(empty_path) };
    assert_eq!(allocs(), before);
}

/// Characterized, not endorsed: destroy frees the file while its ref is still 1.
#[test]
fn direct_destroy_test() {
    if !child(concat!(module_path!(), "::direct_destroy_test")) {
        return;
    }
    let before = allocs();
    let file = unsafe {
        obs_missing_file_create(
            c"a.png".as_ptr(),
            None,
            SCRIPT,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    unsafe { obs_missing_file_destroy(file) };
    assert_eq!(allocs(), before);
}

#[test]
fn release_null_test() {
    if !child(concat!(module_path!(), "::release_null_test")) {
        return;
    }
    let before = allocs();
    unsafe { obs_missing_file_release(std::ptr::null_mut()) };
    assert_eq!(allocs(), before);
}

/// Characterized, not endorsed: add_file does not take a reference.
/// Destroying the list is what frees the file.
#[test]
fn add_does_not_ref_test() {
    if !child(concat!(module_path!(), "::add_does_not_ref_test")) {
        return;
    }
    let before = allocs();
    let files = unsafe { obs_missing_files_create() };
    let file = unsafe {
        obs_missing_file_create(
            c"a.png".as_ptr(),
            None,
            SCRIPT,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    unsafe { obs_missing_files_add_file(files, file) };
    assert_eq!(unsafe { obs_missing_files_count(files) }, 1);
    assert_eq!(unsafe { obs_missing_files_get_file(files, 0) }, file);
    assert_eq!(cstr(unsafe { obs_missing_file_get_path(file) }), c"a.png");
    unsafe { obs_missing_files_destroy(files) };
    assert_eq!(allocs(), before);
}

#[test]
fn insertion_order_test() {
    if !child(concat!(module_path!(), "::insertion_order_test")) {
        return;
    }
    let before = allocs();
    let files = unsafe { obs_missing_files_create() };
    let first = unsafe {
        obs_missing_file_create(
            c"first.png".as_ptr(),
            None,
            SCRIPT,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    let second = unsafe {
        obs_missing_file_create(
            c"second.png".as_ptr(),
            None,
            SCRIPT,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    unsafe { obs_missing_files_add_file(files, first) };
    unsafe { obs_missing_files_add_file(files, second) };
    assert_eq!(unsafe { obs_missing_files_count(files) }, 2);
    let after_add = allocs();
    assert_eq!(unsafe { obs_missing_files_get_file(files, 0) }, first);
    assert_eq!(unsafe { obs_missing_files_get_file(files, 1) }, second);
    assert_eq!(allocs(), after_add);
    unsafe { obs_missing_files_destroy(files) };
    assert_eq!(allocs(), before);
}

/// Characterized, not endorsed: a null entry is stored, and destroy releases
/// it with `obs_missing_file_release(NULL)`, which returns.
#[test]
fn null_entry_test() {
    if !child(concat!(module_path!(), "::null_entry_test")) {
        return;
    }
    let before = allocs();
    let files = unsafe { obs_missing_files_create() };
    unsafe { obs_missing_files_add_file(files, std::ptr::null_mut()) };
    assert_eq!(unsafe { obs_missing_files_count(files) }, 1);
    let after_get = allocs();
    assert!(unsafe { obs_missing_files_get_file(files, 0) }.is_null());
    assert_eq!(allocs(), after_get);
    unsafe { obs_missing_files_destroy(files) };
    assert_eq!(allocs(), before);
}

#[test]
fn append_keeps_files_test() {
    if !child(concat!(module_path!(), "::append_keeps_files_test")) {
        return;
    }
    let before = allocs();
    let src = unsafe { obs_missing_files_create() };
    let dst = unsafe { obs_missing_files_create() };
    let a = unsafe {
        obs_missing_file_create(
            c"a.png".as_ptr(),
            None,
            SCRIPT,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    let b = unsafe {
        obs_missing_file_create(
            c"b.png".as_ptr(),
            None,
            SCRIPT,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    unsafe { obs_missing_files_add_file(src, a) };
    unsafe { obs_missing_files_add_file(src, b) };
    unsafe { obs_missing_files_append(dst, src) };
    assert_eq!(unsafe { obs_missing_files_count(dst) }, 2);
    assert_eq!(unsafe { obs_missing_files_get_file(dst, 0) }, a);
    assert_eq!(unsafe { obs_missing_files_get_file(dst, 1) }, b);
    // src's release drops one ref each. append's ref keeps the files.
    unsafe { obs_missing_files_destroy(src) };
    let after_src_gone = allocs();
    assert_eq!(
        cstr(unsafe { obs_missing_file_get_path(obs_missing_files_get_file(dst, 0)) }),
        c"a.png"
    );
    assert_eq!(
        cstr(unsafe { obs_missing_file_get_path(obs_missing_files_get_file(dst, 1)) }),
        c"b.png"
    );
    assert_eq!(allocs(), after_src_gone);
    unsafe { obs_missing_files_destroy(dst) };
    assert_eq!(allocs(), before);
}

#[test]
fn append_empty_test() {
    if !child(concat!(module_path!(), "::append_empty_test")) {
        return;
    }
    let before = allocs();
    let src = unsafe { obs_missing_files_create() };
    let dst = unsafe { obs_missing_files_create() };
    let file = unsafe {
        obs_missing_file_create(
            c"a.png".as_ptr(),
            None,
            SCRIPT,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    unsafe { obs_missing_files_add_file(dst, file) };
    unsafe { obs_missing_files_append(dst, src) };
    assert_eq!(unsafe { obs_missing_files_count(dst) }, 1);
    assert_eq!(unsafe { obs_missing_files_get_file(dst, 0) }, file);
    unsafe { obs_missing_files_destroy(src) };
    unsafe { obs_missing_files_destroy(dst) };
    assert_eq!(allocs(), before);
}

/// Characterized, not endorsed: a script file stores the callback and never
/// calls it. An unknown type does the same, and stores no source name.
#[test]
fn script_and_unknown_issue_test() {
    if !child(concat!(module_path!(), "::script_and_unknown_issue_test")) {
        return;
    }
    let before = allocs();
    let mut marker = 0i32;
    let script = unsafe {
        obs_missing_file_create(
            c"script.lua".as_ptr(),
            Some(on_replace),
            SCRIPT,
            std::ptr::dangling_mut(),
            (&raw mut marker).cast(),
        )
    };
    let other = unsafe {
        obs_missing_file_create(
            c"other.bin".as_ptr(),
            Some(on_replace),
            2,
            std::ptr::dangling_mut(),
            (&raw mut marker).cast(),
        )
    };
    reset_hit();
    unsafe { obs_missing_file_issue_callback(script, c"new.lua".as_ptr()) };
    unsafe { obs_missing_file_issue_callback(other, std::ptr::null()) };
    assert_eq!(hit().n, 0);
    assert!(unsafe { obs_missing_file_get_source_name(script) }.is_null());
    assert!(unsafe { obs_missing_file_get_source_name(other) }.is_null());
    assert_eq!(
        cstr(unsafe { obs_missing_file_get_path(script) }),
        c"script.lua"
    );
    assert_eq!(
        cstr(unsafe { obs_missing_file_get_path(other) }),
        c"other.bin"
    );
    unsafe { obs_missing_file_release(script) };
    unsafe { obs_missing_file_release(other) };
    assert_eq!(allocs(), before);
}

/// Characterized, not endorsed: a source file with a null source stores a
/// null name, and issue_callback returns without calling.
#[test]
fn null_source_issue_test() {
    if !child(concat!(module_path!(), "::null_source_issue_test")) {
        return;
    }
    let before = allocs();
    let mut marker = 0i32;
    let file = unsafe {
        obs_missing_file_create(
            c"a.png".as_ptr(),
            Some(on_replace),
            SOURCE,
            std::ptr::null_mut(),
            (&raw mut marker).cast(),
        )
    };
    reset_hit();
    assert!(unsafe { obs_missing_file_get_source_name(file) }.is_null());
    unsafe { obs_missing_file_issue_callback(file, c"b.png".as_ptr()) };
    assert_eq!(hit().n, 0);
    assert_eq!(allocs(), before + 2);
    unsafe { obs_missing_file_release(file) };
    assert_eq!(allocs(), before);
}

#[test]
fn source_snapshot_test() {
    if !child(concat!(module_path!(), "::source_snapshot_test")) {
        return;
    }
    let _source = SOURCE_LOCK.lock().expect("source stand-in");
    let mut marker = 0i32;
    let mut private_data = 0i32;
    let clip = c"clip";
    let renamed = c"renamed";
    let source = source_fixture();
    set_source(clip.as_ptr(), (&raw mut private_data).cast());

    let before_file = allocs();
    let file = unsafe {
        obs_missing_file_create(
            c"clip.png".as_ptr(),
            Some(on_replace),
            SOURCE,
            source,
            (&raw mut marker).cast(),
        )
    };
    let after_file = allocs();
    // struct, path, and a copy of the source name.
    assert_eq!(after_file, before_file + 3);
    assert_eq!(
        cstr(unsafe { obs_missing_file_get_path(file) }),
        c"clip.png"
    );
    assert_eq!(
        cstr(unsafe { obs_missing_file_get_source_name(file) }),
        c"clip"
    );
    assert_ne!(unsafe { obs_missing_file_get_source_name(file) }, unsafe {
        obs_source_get_name(source)
    });

    set_source(renamed.as_ptr(), (&raw mut private_data).cast());
    assert_eq!(cstr(unsafe { obs_source_get_name(source) }), c"renamed");
    assert_eq!(
        cstr(unsafe { obs_missing_file_get_source_name(file) }),
        c"clip"
    );

    let new_path = c"fixed.png";
    reset_hit();
    unsafe { obs_missing_file_issue_callback(file, new_path.as_ptr()) };
    let got = hit();
    assert_eq!(got.n, 1);
    assert_eq!(got.src, (&raw const private_data) as usize);
    assert_eq!(got.path, new_path.as_ptr() as usize);
    assert_eq!(got.data, (&raw const marker) as usize);

    reset_hit();
    unsafe { obs_missing_file_issue_callback(file, std::ptr::null()) };
    let got = hit();
    assert_eq!(got.n, 1);
    assert_eq!(got.path, 0);
    assert_eq!(got.src, (&raw const private_data) as usize);
    assert_eq!(got.data, (&raw const marker) as usize);

    let after_ops = allocs();
    unsafe { obs_missing_file_release(file) };
    assert_eq!(allocs(), after_ops - 3);
}
