//! Tier 3: the Rust C ABI shim matches `libobs/util/platform-nix.c`.
//!
//! Intentional differences, excluded here:
//! - A null pointer C would crash on returns an error or null.
//! - `os_opendir` copies the path. C keeps the caller's pointer. Both sides
//!   are called with a path that stays alive.
//! - `os_glob` path strings are copies. The bytes are compared, then both
//!   results are freed with `os_globfree`.
//! - `os_generate_uuid` is random on both sides and is not compared.
//! - `os_breakpoint` and `os_oom` raise `SIGTRAP` and are not called.
//! - Sleep inhibit is `util/platform-nix-inhibit.c`, compared in
//!   `nix_inhibit_parity`.
//! - CPU percent and process memory change between the two calls.
//! - A missing `$HOME` still crashes in the shim. These tests keep `HOME`.
//!
//! On FreeBSD, `os_get_sys_total_size` reads `hw.physmem`. C calls libsysinfo.
//! This file runs on the host that builds the tests.
//!
//! Reminder: the mutation check (break the core, see this file fail) is done
//! once per port, not on every change.

#![cfg(unix)]

use std::ffi::{CStr, CString};
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::sync::Mutex;

use obs_c_oracle::platform_nix as c;
use obs_util::ffi::bmem::bfree;
use obs_util::ffi::platform_nix as rs;

use super::common;
use common::ScratchDir;

fn c_path(path: &Path) -> CString {
    CString::new(path.as_os_str().as_bytes()).unwrap()
}

fn scratch(label: &str) -> ScratchDir {
    ScratchDir::new(&format!("obs-platform-nix-par-{label}"))
}

fn cstr_bytes(p: *const core::ffi::c_char) -> Option<Vec<u8>> {
    if p.is_null() {
        return None;
    }
    // SAFETY: `p` is a C string from a path helper, still live.
    Some(unsafe { CStr::from_ptr(p) }.to_bytes().to_vec())
}

fn free_str(p: *mut core::ffi::c_char) {
    if !p.is_null() {
        // SAFETY: `p` came from `bmalloc` in the shim or the oracle.
        unsafe { bfree(p.cast()) };
    }
}

/// `realpath(".")` and `chdir` share the process cwd.
fn cwd_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

#[test]
fn file_ops_match_the_c_file() {
    let root = scratch("files");
    let rs_dir = root.join("rs");
    let c_dir = root.join("c");
    fs::create_dir(&rs_dir).unwrap();
    fs::create_dir(&c_dir).unwrap();

    let rs_a = rs_dir.join("a.txt");
    let c_a = c_dir.join("a.txt");
    let rs_b = rs_dir.join("b.txt");
    let c_b = c_dir.join("b.txt");
    fs::write(&rs_a, b"payload").unwrap();
    fs::write(&c_a, b"payload").unwrap();

    let rs_a_c = c_path(&rs_a);
    let c_a_c = c_path(&c_a);
    let rs_b_c = c_path(&rs_b);
    let c_b_c = c_path(&c_b);
    let missing_rs = c_path(&rs_dir.join("nope"));
    let missing_c = c_path(&c_dir.join("nope"));

    // SAFETY: every pointer is a live CString.
    unsafe {
        assert_eq!(
            rs::os_file_exists(rs_a_c.as_ptr()),
            c::oracle_os_file_exists(c_a_c.as_ptr())
        );
        assert!(rs::os_file_exists(rs_a_c.as_ptr()));
        assert_eq!(
            rs::os_file_exists(missing_rs.as_ptr()),
            c::oracle_os_file_exists(missing_c.as_ptr())
        );
        assert_eq!(
            rs::os_rename(rs_a_c.as_ptr(), rs_b_c.as_ptr()),
            c::oracle_os_rename(c_a_c.as_ptr(), c_b_c.as_ptr())
        );
        assert_eq!(fs::read(&rs_b).unwrap(), fs::read(&c_b).unwrap());
        assert_eq!(
            rs::os_unlink(missing_rs.as_ptr()),
            c::oracle_os_unlink(missing_c.as_ptr())
        );
        assert_eq!(
            rs::os_unlink(rs_b_c.as_ptr()),
            c::oracle_os_unlink(c_b_c.as_ptr())
        );
    }

    let rs_mk = c_path(&rs_dir.join("mk"));
    let c_mk = c_path(&c_dir.join("mk"));
    let rs_child = c_path(&rs_dir.join("no_parent").join("child"));
    let c_child = c_path(&c_dir.join("no_parent").join("child"));
    // SAFETY: the paths are live CStrings.
    unsafe {
        assert_eq!(
            rs::os_mkdir(rs_mk.as_ptr()),
            c::oracle_os_mkdir(c_mk.as_ptr())
        );
        assert_eq!(
            rs::os_mkdir(rs_mk.as_ptr()),
            c::oracle_os_mkdir(c_mk.as_ptr())
        );
        assert_eq!(
            rs::os_mkdir(rs_child.as_ptr()),
            c::oracle_os_mkdir(c_child.as_ptr())
        );
        assert_eq!(
            rs::os_rmdir(rs_mk.as_ptr()),
            c::oracle_os_rmdir(c_mk.as_ptr())
        );
    }
}

#[test]
fn copy_and_replace_match() {
    let root = scratch("copy");
    let rs_dir = root.join("rs");
    let c_dir = root.join("c");
    fs::create_dir(&rs_dir).unwrap();
    fs::create_dir(&c_dir).unwrap();
    let rs_src = rs_dir.join("src");
    let c_src = c_dir.join("src");
    let rs_dst = rs_dir.join("dst");
    let c_dst = c_dir.join("dst");
    fs::write(&rs_src, b"abc").unwrap();
    fs::write(&c_src, b"abc").unwrap();
    let rs_src_c = c_path(&rs_src);
    let c_src_c = c_path(&c_src);
    let rs_dst_c = c_path(&rs_dst);
    let c_dst_c = c_path(&c_dst);
    // SAFETY: the paths are live CStrings.
    unsafe {
        assert_eq!(
            rs::os_copyfile(rs_src_c.as_ptr(), rs_dst_c.as_ptr()),
            c::oracle_os_copyfile(c_src_c.as_ptr(), c_dst_c.as_ptr())
        );
        assert_eq!(
            rs::os_copyfile(rs_src_c.as_ptr(), rs_dst_c.as_ptr()),
            c::oracle_os_copyfile(c_src_c.as_ptr(), c_dst_c.as_ptr())
        );
    }
    assert_eq!(fs::read(&rs_dst).unwrap(), fs::read(&c_dst).unwrap());

    let rs_from = rs_dir.join("from");
    let c_from = c_dir.join("from");
    let rs_bak = rs_dir.join("bak");
    let c_bak = c_dir.join("bak");
    fs::write(&rs_from, b"new").unwrap();
    fs::write(&c_from, b"new").unwrap();
    let rs_from_c = c_path(&rs_from);
    let c_from_c = c_path(&c_from);
    let rs_bak_c = c_path(&rs_bak);
    let c_bak_c = c_path(&c_bak);
    // SAFETY: the paths are live CStrings. A null backup is the C NULL.
    unsafe {
        assert_eq!(
            rs::os_safe_replace(rs_dst_c.as_ptr(), rs_from_c.as_ptr(), rs_bak_c.as_ptr()),
            c::oracle_os_safe_replace(c_dst_c.as_ptr(), c_from_c.as_ptr(), c_bak_c.as_ptr())
        );
    }
    assert_eq!(fs::read(&rs_dst).unwrap(), b"new");
    assert_eq!(fs::read(&rs_bak).unwrap(), fs::read(&c_bak).unwrap());
}

#[test]
fn directory_listing_matches() {
    let root = scratch("dir");
    fs::create_dir(root.join("sub")).unwrap();
    fs::write(root.join("l1.txt"), b"1").unwrap();
    let path = c_path(&root);
    // SAFETY: `path` outlives both directory walks.
    let (mut rs_names, mut c_names) = unsafe {
        let rs_dir = rs::os_opendir(path.as_ptr());
        let c_dir = c::oracle_os_opendir(path.as_ptr());
        assert!(!rs_dir.is_null() && !c_dir.is_null());
        let mut rs_names = Vec::new();
        let mut c_names = Vec::new();
        loop {
            let ent = rs::os_readdir(rs_dir);
            if ent.is_null() {
                break;
            }
            let name = CStr::from_ptr((*ent).d_name.as_ptr()).to_bytes().to_vec();
            rs_names.push((name, (*ent).directory));
        }
        loop {
            let ent = c::oracle_os_readdir(c_dir);
            if ent.is_null() {
                break;
            }
            let name = CStr::from_ptr((*ent).d_name.as_ptr()).to_bytes().to_vec();
            c_names.push((name, (*ent).directory));
        }
        rs::os_closedir(rs_dir);
        c::oracle_os_closedir(c_dir);
        assert!(rs::os_readdir(std::ptr::null_mut()).is_null());
        assert!(c::oracle_os_readdir(std::ptr::null_mut()).is_null());
        rs::os_closedir(std::ptr::null_mut());
        c::oracle_os_closedir(std::ptr::null_mut());
        (rs_names, c_names)
    };
    rs_names.sort();
    c_names.sort();
    assert_eq!(rs_names, c_names);
}

#[test]
fn abs_path_and_glob_match() {
    let _cwd = cwd_lock();
    let root = scratch("abs");
    fs::write(root.join("a.txt"), b"a").unwrap();
    fs::write(root.join("b.txt"), b"b").unwrap();
    let dot = CString::new(".").unwrap();
    let file = c_path(&root.join("a.txt"));
    // SAFETY: the strings are live.
    unsafe {
        let mut rs_buf = [0u8; 512];
        let mut c_buf = [0u8; 512];
        let rs_n = rs::os_get_abs_path(dot.as_ptr(), rs_buf.as_mut_ptr().cast(), rs_buf.len());
        let c_n = c::oracle_os_get_abs_path(dot.as_ptr(), c_buf.as_mut_ptr().cast(), c_buf.len());
        assert_eq!(rs_n, c_n);
        assert_eq!(rs_buf, c_buf);
        assert_eq!(
            rs::os_get_abs_path(dot.as_ptr(), std::ptr::null_mut(), 8),
            0
        );
        assert_eq!(
            c::oracle_os_get_abs_path(dot.as_ptr(), std::ptr::null_mut(), 8),
            0
        );

        let rs_ptr = rs::os_get_abs_path_ptr(file.as_ptr());
        let c_ptr = c::oracle_os_get_abs_path_ptr(file.as_ptr());
        assert_eq!(cstr_bytes(rs_ptr), cstr_bytes(c_ptr));
        free_str(rs_ptr);
        free_str(c_ptr);
    }

    let pattern = c_path(&root.join("*.txt"));
    // SAFETY: `pattern` is a live CString.
    unsafe {
        let mut rs_glob = std::ptr::null_mut();
        let mut c_glob = std::ptr::null_mut();
        assert_eq!(
            rs::os_glob(pattern.as_ptr(), 0, &mut rs_glob),
            c::oracle_os_glob(pattern.as_ptr(), 0, &mut c_glob)
        );
        assert!(!rs_glob.is_null() && !c_glob.is_null());
        let mut rs_ents = Vec::new();
        let mut c_ents = Vec::new();
        for i in 0..(*rs_glob).gl_pathc {
            let ent = &*(*rs_glob).gl_pathv.add(i);
            rs_ents.push((cstr_bytes(ent.path), ent.directory));
        }
        for i in 0..(*c_glob).gl_pathc {
            let ent = &*(*c_glob).gl_pathv.add(i);
            c_ents.push((cstr_bytes(ent.path), ent.directory));
        }
        rs_ents.sort();
        c_ents.sort();
        assert_eq!(rs_ents, c_ents);
        rs::os_globfree(rs_glob);
        c::oracle_os_globfree(c_glob);
        rs::os_globfree(std::ptr::null_mut());
        c::oracle_os_globfree(std::ptr::null_mut());
    }
}

#[test]
fn getcwd_matches_and_chdir_roundtrips() {
    if !crate::common::isolated(concat!(
        module_path!(),
        "::getcwd_matches_and_chdir_roundtrips"
    )) {
        return;
    }
    let _cwd = cwd_lock();
    let mut rs_buf = [0u8; 4096];
    let mut c_buf = [0u8; 4096];
    // SAFETY: both buffers are 4096 bytes.
    unsafe {
        let rs_p = rs::os_getcwd(rs_buf.as_mut_ptr().cast(), rs_buf.len());
        let c_p = c::oracle_os_getcwd(c_buf.as_mut_ptr().cast(), c_buf.len());
        assert!(!rs_p.is_null() && !c_p.is_null());
        assert_eq!(
            CStr::from_ptr(rs_p).to_bytes(),
            CStr::from_ptr(c_p).to_bytes()
        );
        // A null buffer allocates with malloc. Both sides return that
        // pointer, and the caller frees it with free.
        let rs_alloc = rs::os_getcwd(std::ptr::null_mut(), 0);
        let c_alloc = c::oracle_os_getcwd(std::ptr::null_mut(), 0);
        assert!(!rs_alloc.is_null() && !c_alloc.is_null());
        assert_eq!(
            CStr::from_ptr(rs_alloc).to_bytes(),
            CStr::from_ptr(c_alloc).to_bytes()
        );
        libc::free(rs_alloc.cast());
        libc::free(c_alloc.cast());
    }
    let dir = scratch("cwd");
    let path = c_path(&dir);
    let saved = std::env::current_dir().unwrap();
    // SAFETY: `path` is a live directory. Both calls target it, so both
    // return 0. macOS `getcwd` resolves `/var` to `/private/var`, so the
    // check uses the canonical path. The process cwd is restored before
    // the lock drops.
    unsafe {
        assert_eq!(rs::os_chdir(path.as_ptr()), 0);
        assert_eq!(c::oracle_os_chdir(path.as_ptr()), 0);
        let rs_p = rs::os_getcwd(rs_buf.as_mut_ptr().cast(), rs_buf.len());
        let c_p = c::oracle_os_getcwd(c_buf.as_mut_ptr().cast(), c_buf.len());
        assert!(!rs_p.is_null() && !c_p.is_null());
        assert_eq!(
            CStr::from_ptr(rs_p).to_bytes(),
            CStr::from_ptr(c_p).to_bytes()
        );
    }
    assert_eq!(
        std::env::current_dir().unwrap(),
        fs::canonicalize(&dir).unwrap()
    );
    std::env::set_current_dir(&saved).unwrap();
}

#[test]
fn sleepto_past_and_plugin_flag_match() {
    // SAFETY: no pointers, and the target is already in the past.
    unsafe {
        assert_eq!(rs::os_sleepto_ns(0), c::oracle_os_sleepto_ns(0));
        assert!(!rs::os_sleepto_ns(0));
        assert_eq!(rs::os_sleepto_ns_fast(0), c::oracle_os_sleepto_ns_fast(0));
        rs::os_sleep_ms(0);
        c::oracle_os_sleep_ms(0);
        assert!(rs::os_is_obs_plugin(std::ptr::null()));
        assert!(c::oracle_os_is_obs_plugin(std::ptr::null()));
        let mut rs_flag = false;
        let mut c_flag = false;
        rs::get_plugin_info(std::ptr::null(), &mut rs_flag);
        c::oracle_get_plugin_info(std::ptr::null(), &mut c_flag);
        assert!(rs_flag && c_flag);
        let missing = CString::new("/no/such/libobs-plugin-nix.so").unwrap();
        assert!(rs::os_dlopen(missing.as_ptr()).is_null());
        assert!(c::oracle_os_dlopen(missing.as_ptr()).is_null());
    }
}

#[cfg(not(target_os = "macos"))]
#[test]
fn host_paths_and_cores_match() {
    let name = CString::new("obs-studio").unwrap();
    let empty = CString::new("").unwrap();
    // SAFETY: the names are live CStrings. HOME is set in this environment.
    unsafe {
        let mut rs_buf = [0u8; 4096];
        let mut c_buf = [0u8; 4096];
        let rs_n =
            rs::os_get_program_data_path(rs_buf.as_mut_ptr().cast(), rs_buf.len(), name.as_ptr());
        let c_n = c::oracle_os_get_program_data_path(
            c_buf.as_mut_ptr().cast(),
            c_buf.len(),
            name.as_ptr(),
        );
        assert_eq!(rs_n, c_n);
        assert_eq!(&rs_buf[..rs_n as usize], &c_buf[..c_n as usize]);

        let rs_ptr = rs::os_get_program_data_path_ptr(std::ptr::null());
        let c_ptr = c::oracle_os_get_program_data_path_ptr(std::ptr::null());
        assert_eq!(cstr_bytes(rs_ptr), cstr_bytes(c_ptr));
        free_str(rs_ptr);
        free_str(c_ptr);

        let rs_n = rs::os_get_config_path(rs_buf.as_mut_ptr().cast(), rs_buf.len(), empty.as_ptr());
        let c_n =
            c::oracle_os_get_config_path(c_buf.as_mut_ptr().cast(), c_buf.len(), empty.as_ptr());
        assert_eq!(rs_n, c_n);
        assert_eq!(&rs_buf[..rs_n as usize], &c_buf[..c_n as usize]);

        let rs_ptr = rs::os_get_config_path_ptr(name.as_ptr());
        let c_ptr = c::oracle_os_get_config_path_ptr(name.as_ptr());
        assert_eq!(cstr_bytes(rs_ptr), cstr_bytes(c_ptr));
        free_str(rs_ptr);
        free_str(c_ptr);

        let rs_ptr = rs::os_get_executable_path_ptr(std::ptr::null());
        let c_ptr = c::oracle_os_get_executable_path_ptr(std::ptr::null());
        assert_eq!(cstr_bytes(rs_ptr), cstr_bytes(c_ptr));
        free_str(rs_ptr);
        free_str(c_ptr);

        assert_eq!(
            rs::os_get_emulation_status(),
            c::oracle_os_get_emulation_status()
        );
        assert_eq!(
            rs::os_get_physical_cores(),
            c::oracle_os_get_physical_cores()
        );
        assert_eq!(rs::os_get_logical_cores(), c::oracle_os_get_logical_cores());
        assert_eq!(
            rs::os_get_sys_total_size(),
            c::oracle_os_get_sys_total_size()
        );
        assert!(rs::os_request_high_performance(name.as_ptr()).is_null());
        assert!(c::oracle_os_request_high_performance(name.as_ptr()).is_null());
        rs::os_end_high_performance(std::ptr::null_mut());
        c::oracle_os_end_high_performance(std::ptr::null_mut());
    }
}

/// A C string longer than `INT_MAX` that costs two 64 KiB pages of memory:
/// one page of `a`s mapped over and over, then a page ending in NUL.
#[cfg(target_os = "linux")]
struct HugeName {
    base: *mut libc::c_void,
    len: usize,
}

#[cfg(target_os = "linux")]
impl HugeName {
    const CHUNK: usize = 64 * 1024;

    fn new() -> Self {
        let chunks = (1usize << 31) / Self::CHUNK;
        let len = (chunks + 1) * Self::CHUNK;
        // SAFETY: plain mmap/memfd calls; every mapping lands inside the
        // reserved range.
        unsafe {
            let base = libc::mmap(
                std::ptr::null_mut(),
                len,
                libc::PROT_NONE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS | libc::MAP_NORESERVE,
                -1,
                0,
            );
            assert_ne!(base, libc::MAP_FAILED);
            let fill = |byte_end: u8| {
                let fd = libc::memfd_create(c"huge-name".as_ptr(), 0);
                assert!(fd >= 0);
                let mut page = vec![b'a'; Self::CHUNK];
                page[Self::CHUNK - 1] = byte_end;
                assert_eq!(
                    libc::write(fd, page.as_ptr().cast(), page.len()),
                    page.len() as isize
                );
                fd
            };
            let body = fill(b'a');
            let tail = fill(0);
            for i in 0..=chunks {
                let fd = if i == chunks { tail } else { body };
                let at = base.cast::<u8>().add(i * Self::CHUNK).cast();
                let p = libc::mmap(
                    at,
                    Self::CHUNK,
                    libc::PROT_READ,
                    libc::MAP_SHARED | libc::MAP_FIXED,
                    fd,
                    0,
                );
                assert_eq!(p, at);
            }
            libc::close(body);
            libc::close(tail);
            Self { base, len }
        }
    }

    fn as_ptr(&self) -> *const core::ffi::c_char {
        self.base.cast()
    }
}

#[cfg(target_os = "linux")]
impl Drop for HugeName {
    fn drop(&mut self) {
        // SAFETY: the range was mapped in `new`.
        unsafe { libc::munmap(self.base, self.len) };
    }
}

/// obs-rust/obs-studio#566: a program data path longer than `INT_MAX` is an
/// `snprintf` failure (-1). C used to take -1 as a `size_t` length.
#[cfg(target_os = "linux")]
#[test]
fn program_data_path_longer_than_int_max_fails() {
    let name = HugeName::new();
    // SAFETY: `name` is a NUL-terminated string that outlives the calls.
    unsafe {
        assert_eq!(
            c::oracle_os_get_program_data_path(std::ptr::null_mut(), 0, name.as_ptr()),
            -1
        );
        assert_eq!(
            rs::os_get_program_data_path(std::ptr::null_mut(), 0, name.as_ptr()),
            -1
        );
        assert!(c::oracle_os_get_program_data_path_ptr(name.as_ptr()).is_null());
        assert!(rs::os_get_program_data_path_ptr(name.as_ptr()).is_null());
    }
}
