//! Tier 3: the shim against the original `obs-missing-files.c.h`.
//!
//! Both sides call the production source-name getter with a real-layout fixture and
//! `obs_source_replace_missing_file`. Allocations share `bnum_allocs`, so a
//! count is taken only after one side has freed everything.

use std::ffi::{CStr, CString, c_char, c_int, c_void};

use obs_c_oracle::missing_files::{
    oracle_missing_file_enum_script, oracle_missing_file_enum_source,
    oracle_obs_missing_file_create, oracle_obs_missing_file_destroy,
    oracle_obs_missing_file_get_path, oracle_obs_missing_file_get_source_name,
    oracle_obs_missing_file_issue_callback, oracle_obs_missing_file_release,
    oracle_obs_missing_files_add_file, oracle_obs_missing_files_append,
    oracle_obs_missing_files_count, oracle_obs_missing_files_create,
    oracle_obs_missing_files_destroy, oracle_obs_missing_files_get_file,
};
use obs_missing_files::ffi::missing_files::{
    obs_missing_file_create, obs_missing_file_destroy, obs_missing_file_get_path,
    obs_missing_file_get_source_name, obs_missing_file_issue_callback, obs_missing_file_release,
    obs_missing_files_add_file, obs_missing_files_append, obs_missing_files_count,
    obs_missing_files_create, obs_missing_files_destroy, obs_missing_files_get_file,
};
use obs_missing_files::{SCRIPT, SOURCE};
use proptest::prelude::*;

use super::{SOURCE_LOCK, allocs, child, set_source};

type Cb = Option<unsafe extern "C" fn(*mut c_void, *const c_char, *mut c_void)>;
type CreateFile =
    unsafe extern "C" fn(*const c_char, Cb, c_int, *mut c_void, *mut c_void) -> *mut c_void;
type CreateList = unsafe extern "C" fn() -> *mut c_void;

struct Side {
    create_list: CreateList,
    destroy_list: unsafe extern "C" fn(*mut c_void),
    add: unsafe extern "C" fn(*mut c_void, *mut c_void),
    count: unsafe extern "C" fn(*mut c_void) -> usize,
    get: unsafe extern "C" fn(*mut c_void, c_int) -> *mut c_void,
    append: unsafe extern "C" fn(*mut c_void, *mut c_void),
    create_file: CreateFile,
    release: unsafe extern "C" fn(*mut c_void),
    destroy_file: unsafe extern "C" fn(*mut c_void),
    issue: unsafe extern "C" fn(*mut c_void, *const c_char),
    path: unsafe extern "C" fn(*mut c_void) -> *const c_char,
    name: unsafe extern "C" fn(*mut c_void) -> *const c_char,
}

fn rust_side() -> Side {
    Side {
        create_list: obs_missing_files_create,
        destroy_list: obs_missing_files_destroy,
        add: obs_missing_files_add_file,
        count: obs_missing_files_count,
        get: obs_missing_files_get_file,
        append: obs_missing_files_append,
        create_file: obs_missing_file_create,
        release: obs_missing_file_release,
        destroy_file: obs_missing_file_destroy,
        issue: obs_missing_file_issue_callback,
        path: obs_missing_file_get_path,
        name: obs_missing_file_get_source_name,
    }
}

fn oracle_side() -> Side {
    Side {
        create_list: oracle_obs_missing_files_create,
        destroy_list: oracle_obs_missing_files_destroy,
        add: oracle_obs_missing_files_add_file,
        count: oracle_obs_missing_files_count,
        get: oracle_obs_missing_files_get_file,
        append: oracle_obs_missing_files_append,
        create_file: oracle_obs_missing_file_create,
        release: oracle_obs_missing_file_release,
        destroy_file: oracle_obs_missing_file_destroy,
        issue: oracle_obs_missing_file_issue_callback,
        path: oracle_obs_missing_file_get_path,
        name: oracle_obs_missing_file_get_source_name,
    }
}

fn owned(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    // SAFETY: the missing-file API returns a NUL-terminated path or name.
    Some(
        unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned(),
    )
}

struct FileSeen {
    path: Option<String>,
    name: Option<String>,
    path_is_copy: bool,
    calls: u32,
    saw_null_path: bool,
    src_is_private: bool,
    data_is_marker: bool,
    path_ptr_was_arg: bool,
}

fn on_file(
    side: &Side,
    path: *const c_char,
    src_type: c_int,
    source: *mut c_void,
    private: *mut c_void,
) -> FileSeen {
    let mut marker = 0i32;
    let marker_ptr = (&raw mut marker).cast();
    // SAFETY: the pointers are the test's, or null. The file is released below.
    let file =
        unsafe { (side.create_file)(path, Some(super::on_replace), src_type, source, marker_ptr) };
    let stored_path = unsafe { (side.path)(file) };
    let stored_name = unsafe { (side.name)(file) };
    let seen_path = owned(stored_path);
    let seen_name = owned(stored_name);
    let path_is_copy = path.is_null() || stored_path != path;

    super::reset_hit();
    let fixed = c"fixed.png";
    unsafe { (side.issue)(file, fixed.as_ptr()) };
    let first = super::hit();
    super::reset_hit();
    unsafe { (side.issue)(file, std::ptr::null()) };
    let second = super::hit();
    unsafe { (side.release)(file) };

    FileSeen {
        path: seen_path,
        name: seen_name,
        path_is_copy,
        calls: first.n,
        saw_null_path: second.n == 1 && second.path == 0,
        src_is_private: first.n == 1 && first.src == private as usize,
        data_is_marker: first.n == 1 && first.data == marker_ptr as usize,
        path_ptr_was_arg: first.n == 1 && first.path == fixed.as_ptr() as usize,
    }
}

fn compare_file(path: Option<&str>, src_type: c_int, source_null: bool) {
    let _guard = SOURCE_LOCK.lock().expect("source stand-in");
    let text = path.map(|p| CString::new(p).expect("path"));
    let path_ptr = text
        .as_ref()
        .map(|c| c.as_ptr())
        .unwrap_or(std::ptr::null());
    let mut private_data = 0i32;
    let private = (&raw mut private_data).cast();
    let source = if source_null {
        std::ptr::null_mut()
    } else {
        super::source_fixture()
    };
    if !source_null {
        set_source(c"clip".as_ptr(), private);
    }
    let rust = on_file(&rust_side(), path_ptr, src_type, source, private);
    if !source_null {
        set_source(c"clip".as_ptr(), private);
    }
    let oracle = on_file(&oracle_side(), path_ptr, src_type, source, private);
    assert_eq!(rust.path, oracle.path);
    assert_eq!(rust.name, oracle.name);
    assert_eq!(rust.path_is_copy, oracle.path_is_copy);
    assert_eq!(rust.calls, oracle.calls);
    assert_eq!(rust.saw_null_path, oracle.saw_null_path);
    assert_eq!(rust.src_is_private, oracle.src_is_private);
    assert_eq!(rust.data_is_marker, oracle.data_is_marker);
    assert_eq!(rust.path_ptr_was_arg, oracle.path_ptr_was_arg);
    if src_type == SOURCE && !source_null {
        assert_eq!(rust.calls, 1);
        assert_eq!(rust.name.as_deref(), Some("clip"));
        assert!(rust.src_is_private);
        assert!(rust.path_ptr_was_arg);
        assert!(rust.saw_null_path);
    } else {
        assert_eq!(rust.calls, 0);
        assert!(rust.name.is_none());
    }
}

fn compare_append() {
    let _guard = SOURCE_LOCK.lock().expect("source stand-in");
    for side in [rust_side(), oracle_side()] {
        // SAFETY: lists and files are destroyed before the next side runs.
        unsafe {
            let src = (side.create_list)();
            let dst = (side.create_list)();
            let a = (side.create_file)(
                c"a.png".as_ptr(),
                None,
                SCRIPT,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
            let b = (side.create_file)(
                c"b.png".as_ptr(),
                None,
                SCRIPT,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
            (side.add)(src, a);
            (side.add)(src, b);
            (side.append)(dst, src);
            assert_eq!((side.count)(dst), 2);
            assert_eq!((side.get)(dst, 0), a);
            assert_eq!((side.get)(dst, 1), b);
            (side.destroy_list)(src);
            assert_eq!(CStr::from_ptr((side.path)((side.get)(dst, 0))), c"a.png");
            assert_eq!(CStr::from_ptr((side.path)((side.get)(dst, 1))), c"b.png");
            (side.destroy_list)(dst);
        }
    }
}

#[test]
fn enum_values_match_the_header() {
    // SAFETY: the oracle returns the header's enum constants.
    assert_eq!(unsafe { oracle_missing_file_enum_source() }, SOURCE);
    assert_eq!(unsafe { oracle_missing_file_enum_script() }, SCRIPT);
    assert_eq!(SOURCE, 0);
    assert_eq!(SCRIPT, 1);
}

#[test]
fn scripted_cases_match_oracle() {
    compare_file(Some("shot.png"), SCRIPT, true);
    compare_file(None, SCRIPT, true);
    compare_file(Some(""), SCRIPT, true);
    compare_file(Some("a.png"), SOURCE, true);
    compare_file(Some("clip.png"), SOURCE, false);
    compare_file(Some("script.lua"), SCRIPT, false);
    compare_file(Some("other.bin"), 2, false);
    compare_append();
}

#[test]
fn direct_destroy_returns_the_alloc_counter() {
    if !child(concat!(
        module_path!(),
        "::direct_destroy_returns_the_alloc_counter"
    )) {
        return;
    }
    let before = allocs();
    for side in [rust_side(), oracle_side()] {
        // SAFETY: one file, destroyed while its ref is still 1, as in C.
        unsafe {
            let file = (side.create_file)(
                c"a.png".as_ptr(),
                None,
                SCRIPT,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
            (side.destroy_file)(file);
        }
        assert_eq!(allocs(), before);
        // SAFETY: an empty list is one allocation, and release(NULL) is none.
        unsafe {
            let list = (side.create_list)();
            (side.add)(list, std::ptr::null_mut());
            assert_eq!((side.count)(list), 1);
            (side.destroy_list)(list);
            (side.release)(std::ptr::null_mut());
        }
        assert_eq!(allocs(), before);
    }
}

proptest! {
    #[test]
    fn files_match_oracle(
        path in proptest::option::of("[ -~]{0,24}"),
        src_type in 0i32..=2,
        source_null in any::<bool>(),
    ) {
        let path = path.as_deref().filter(|p| !p.contains('\0'));
        compare_file(path, src_type, source_null);
    }
}
