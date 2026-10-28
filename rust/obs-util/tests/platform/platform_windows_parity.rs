//! Tier 3: `platform-windows.c` through the oracle, against the Rust shims.
//!
//! Not compared: UUID values (both are random), CPU percent, process-memory
//! byte counts, and free-space counts that can change between the two calls.
//! `os_breakpoint` and `os_oom` trap. `DllMain` is not called: the oracle
//! build defines `PTW32_STATIC_LIB` and the Rust copy matches libobs, which
//! does not. A null `os_glob` out-pointer is Rust-only (C writes through it).
//!
//! `chdir` and the process-global version cache share one lock.

// The C oracle for platform-windows.c is MSVC-only (obs-c-oracle build.rs).
#![cfg(all(windows, target_env = "msvc"))]

use std::ffi::{CStr, CString};
use std::fs;
use std::os::raw::c_char;
use std::path::Path;
use std::sync::Mutex;

use obs_c_oracle::platform_windows as c;
use obs_util::ffi::bmem::bfree;
use obs_util::ffi::platform_windows as r;

use super::common;
use common::ScratchDir;

static PARITY: Mutex<()> = Mutex::new(());

fn cstring(bytes: &[u8]) -> CString {
    CString::new(bytes).unwrap()
}

fn path_bytes(path: &Path) -> Vec<u8> {
    path.to_str().unwrap().as_bytes().to_vec()
}

fn owned(p: *const c_char) -> Vec<u8> {
    if p.is_null() {
        return Vec::new();
    }
    // SAFETY: `p` is a C string from the oracle or a shim.
    unsafe { CStr::from_ptr(p) }.to_bytes().to_vec()
}

fn free_owned(p: *mut c_char) -> Vec<u8> {
    let bytes = owned(p);
    if !p.is_null() {
        // SAFETY: `p` came from `bmalloc` (dstr or the path helpers).
        unsafe { bfree(p.cast()) };
    }
    bytes
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn scratch() -> ScratchDir {
    ScratchDir::new("obs-platform-windows-parity")
}

fn forward_slash(path: &Path) -> Vec<u8> {
    path.to_str().unwrap().replace('\\', "/").into_bytes()
}

#[test]
#[expect(
    clippy::cognitive_complexity,
    reason = "parity table: one block per ported C case"
)]
#[expect(
    clippy::too_many_lines,
    reason = "parity table: one block per ported C case"
)]
fn filesystem_matches_the_c_file() {
    let _guard = PARITY.lock().unwrap_or_else(|e| e.into_inner());
    let dir = scratch();
    let file = dir.join("a.txt");
    fs::write(&file, b"abc").unwrap();
    let file_c = cstring(&path_bytes(&file));
    let missing = cstring(&path_bytes(&dir.join("nope.txt")));

    // SAFETY: the paths are NUL-terminated.
    unsafe {
        assert_eq!(
            r::os_file_exists(file_c.as_ptr()),
            c::oracle_os_file_exists(file_c.as_ptr())
        );
        assert!(!r::os_file_exists(missing.as_ptr()));
        assert!(!c::oracle_os_file_exists(missing.as_ptr()));
        assert_eq!(
            r::os_unlink(missing.as_ptr()),
            c::oracle_os_unlink(missing.as_ptr())
        );
    }

    let made = dir.join("sub");
    let made_c = cstring(&path_bytes(&made));
    // SAFETY: the path is NUL-terminated.
    unsafe {
        assert_eq!(r::os_mkdir(made_c.as_ptr()), 0);
        assert_eq!(c::oracle_os_mkdir(made_c.as_ptr()), 1);
        assert_eq!(r::os_mkdir(made_c.as_ptr()), 1);
    }

    let renamed = dir.join("b.txt");
    let renamed_c = cstring(&path_bytes(&renamed));
    // SAFETY: both paths are NUL-terminated. The C call finds the file already renamed.
    unsafe {
        assert_eq!(r::os_rename(file_c.as_ptr(), renamed_c.as_ptr()), 0);
        assert_eq!(c::oracle_os_rename(file_c.as_ptr(), renamed_c.as_ptr()), -1);
    }
    assert!(renamed.is_file());

    let copied = dir.join("c.txt");
    let copied_c = cstring(&path_bytes(&copied));
    // SAFETY: both paths are NUL-terminated. The second copy finds the dest.
    unsafe {
        assert_eq!(r::os_copyfile(renamed_c.as_ptr(), copied_c.as_ptr()), 0);
        assert_eq!(
            c::oracle_os_copyfile(renamed_c.as_ptr(), copied_c.as_ptr()),
            -1
        );
    }
    assert_eq!(fs::read(&copied).unwrap(), b"abc");

    let replaced = dir.join("d.txt");
    fs::write(&replaced, b"old").unwrap();
    let from = dir.join("e.txt");
    fs::write(&from, b"new").unwrap();
    let backup = dir.join("e.bak");
    // SAFETY: the three paths are NUL-terminated.
    unsafe {
        assert_eq!(
            r::os_safe_replace(
                cstring(&path_bytes(&replaced)).as_ptr(),
                cstring(&path_bytes(&from)).as_ptr(),
                cstring(&path_bytes(&backup)).as_ptr(),
            ),
            0
        );
    }
    assert_eq!(fs::read(&replaced).unwrap(), b"new");
    assert_eq!(fs::read(&backup).unwrap(), b"old");

    let fresh = dir.join("f.txt");
    fs::write(&fresh, b"only").unwrap();
    let dest = dir.join("g.txt");
    // SAFETY: backup is null. The destination does not exist yet, so ReplaceFile
    // fails with ERROR_FILE_NOT_FOUND and MoveFileEx takes over.
    unsafe {
        assert_eq!(
            c::oracle_os_safe_replace(
                cstring(&path_bytes(&dest)).as_ptr(),
                cstring(&path_bytes(&fresh)).as_ptr(),
                std::ptr::null(),
            ),
            0
        );
    }
    assert_eq!(fs::read(&dest).unwrap(), b"only");

    let mut rust_names = Vec::new();
    let dir_c = cstring(&path_bytes(&dir));
    // SAFETY: the directory path is NUL-terminated. The handle is closed below.
    unsafe {
        let dir_handle = r::os_opendir(dir_c.as_ptr());
        assert!(!dir_handle.is_null());
        loop {
            let ent = r::os_readdir(dir_handle);
            if ent.is_null() {
                break;
            }
            rust_names.push(owned((*ent).d_name.as_ptr()));
        }
        r::os_closedir(dir_handle);
        assert!(r::os_readdir(std::ptr::null_mut()).is_null());
        r::os_closedir(std::ptr::null_mut());
    }
    let mut c_names = Vec::new();
    // SAFETY: the directory path is NUL-terminated. The handle is closed below.
    unsafe {
        let dir_handle = c::oracle_os_opendir(dir_c.as_ptr());
        assert!(!dir_handle.is_null());
        loop {
            let ent = c::oracle_os_readdir(dir_handle);
            if ent.is_null() {
                break;
            }
            c_names.push(owned((*ent).d_name.as_ptr()));
        }
        c::oracle_os_closedir(dir_handle);
    }
    rust_names.sort();
    c_names.sort();
    assert_eq!(rust_names, c_names);

    fs::write(dir.join("a.txt"), b"a").unwrap();
    fs::write(dir.join("b.txt"), b"b").unwrap();
    let slash = cstring(&{
        let mut p = forward_slash(&dir);
        p.extend_from_slice(b"/*.txt");
        p
    });
    let back = cstring(&{
        let mut p = path_bytes(&dir);
        p.extend_from_slice(b"\\*.txt");
        p
    });
    assert_eq!(
        glob_entries(slash.as_ptr(), true),
        glob_entries(slash.as_ptr(), false)
    );
    assert_eq!(
        glob_entries(back.as_ptr(), true),
        glob_entries(back.as_ptr(), false)
    );

    let mut abs_r = [0u8; 260];
    let mut abs_c = [0u8; 260];
    let dot = cstring(b".");
    // SAFETY: both buffers are 260 bytes. `.` is a C string.
    unsafe {
        let n_r = r::os_get_abs_path(dot.as_ptr(), abs_r.as_mut_ptr().cast(), abs_r.len());
        let n_c = c::oracle_os_get_abs_path(dot.as_ptr(), abs_c.as_mut_ptr().cast(), abs_c.len());
        assert_eq!(&abs_r[..n_r], &abs_c[..n_c]);
        assert_eq!(abs_r.get(1).copied(), Some(b':'));
        assert_eq!(r::os_glob(slash.as_ptr(), 0, std::ptr::null_mut()), -1);
    }
}

fn glob_entries(pattern: *const c_char, rust: bool) -> Vec<(Vec<u8>, bool)> {
    let mut out = Vec::new();
    if rust {
        let mut info = std::ptr::null_mut();
        // SAFETY: `pattern` is a C string. `info` is freed below.
        let rc = unsafe { r::os_glob(pattern, 0, &mut info) };
        assert_eq!(rc, 0);
        // SAFETY: `os_glob` wrote `info` on success.
        unsafe {
            let n = (*info).gl_pathc;
            let pathv = (*info).gl_pathv;
            for i in 0..n {
                let ent = &*pathv.add(i);
                out.push((owned(ent.path), ent.directory));
            }
            r::os_globfree(info);
        }
    } else {
        let mut info = std::ptr::null_mut();
        // SAFETY: `pattern` is a C string. `info` is freed below.
        let rc = unsafe { c::oracle_os_glob(pattern, 0, &mut info) };
        assert_eq!(rc, 0);
        // SAFETY: the oracle wrote `info` on success.
        unsafe {
            let n = (*info).gl_pathc;
            let pathv = (*info).gl_pathv;
            for i in 0..n {
                let ent = &*pathv.add(i);
                out.push((owned(ent.path), ent.directory));
            }
            c::oracle_os_globfree(info);
        }
    }
    out.sort();
    out
}

#[test]
#[expect(
    clippy::cognitive_complexity,
    reason = "parity table: one block per ported C case"
)]
fn paths_version_and_cores_match_the_c_file() {
    let _guard = PARITY.lock().unwrap_or_else(|e| e.into_inner());
    let name = cstring(b"obs-studio");
    let mut rust_buf = [0u8; 512];
    let mut c_buf = [0u8; 512];
    // SAFETY: both buffers are 512 bytes. `name` is a C string.
    unsafe {
        let n_r =
            r::os_get_config_path(rust_buf.as_mut_ptr().cast(), rust_buf.len(), name.as_ptr());
        let n_c =
            c::oracle_os_get_config_path(c_buf.as_mut_ptr().cast(), c_buf.len(), name.as_ptr());
        assert!(n_r > 0);
        assert_eq!(n_r, n_c);
        assert_eq!(&rust_buf[..n_r as usize], &c_buf[..n_c as usize]);

        let n_r = r::os_get_program_data_path(
            rust_buf.as_mut_ptr().cast(),
            rust_buf.len(),
            name.as_ptr(),
        );
        let n_c = c::oracle_os_get_program_data_path(
            c_buf.as_mut_ptr().cast(),
            c_buf.len(),
            name.as_ptr(),
        );
        assert!(n_r > 0);
        assert_eq!(n_r, n_c);
        assert_eq!(&rust_buf[..n_r as usize], &c_buf[..n_c as usize]);

        assert_eq!(
            free_owned(r::os_get_config_path_ptr(std::ptr::null())),
            free_owned(c::oracle_os_get_config_path_ptr(std::ptr::null()))
        );
        assert_eq!(
            free_owned(r::os_get_executable_path_ptr(name.as_ptr())),
            free_owned(c::oracle_os_get_executable_path_ptr(name.as_ptr()))
        );
    }

    // SAFETY: the out-structs are writable. The wide strings are terminated.
    unsafe {
        let mut rust_ver = std::mem::zeroed();
        let mut c_ver = std::mem::zeroed();
        r::get_win_ver(&mut rust_ver);
        c::oracle_get_win_ver(&mut c_ver);
        assert_eq!(
            (
                rust_ver.major,
                rust_ver.minor,
                rust_ver.build,
                rust_ver.revis
            ),
            (c_ver.major, c_ver.minor, c_ver.build, c_ver.revis)
        );
        assert_eq!(r::get_win_ver_int(), c::oracle_get_win_ver_int());
        assert_eq!(
            owned(r::get_win_release_id()),
            owned(c::oracle_get_win_release_id())
        );
        assert_eq!(r::is_64_bit_windows(), c::oracle_is_64_bit_windows());
        assert_eq!(r::is_arm64_windows(), c::oracle_is_arm64_windows());
        assert_eq!(
            r::os_get_emulation_status(),
            c::oracle_os_get_emulation_status()
        );
        assert_eq!(
            r::os_get_physical_cores(),
            c::oracle_os_get_physical_cores()
        );
        assert_eq!(r::os_get_logical_cores(), c::oracle_os_get_logical_cores());
        assert!(r::os_get_physical_cores() > 0);
        assert!(r::os_get_sys_total_size() > 0);
        assert_eq!(
            r::os_get_sys_total_size(),
            c::oracle_os_get_sys_total_size()
        );

        let lib = wide("ntoskrnl.exe");
        let mut rust_dll = std::mem::zeroed();
        let mut c_dll = std::mem::zeroed();
        assert!(r::get_dll_ver(lib.as_ptr(), &mut rust_dll));
        assert!(c::oracle_get_dll_ver(lib.as_ptr(), &mut c_dll));
        assert_eq!(
            (
                rust_dll.major,
                rust_dll.minor,
                rust_dll.build,
                rust_dll.revis
            ),
            (c_dll.major, c_dll.minor, c_dll.build, c_dll.revis)
        );

        let sub = wide("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion");
        let value = wide("CurrentMajorVersionNumber");
        let hklm = 0x8000_0002u32 as i32 as isize as *mut std::ffi::c_void;
        let mut rust_reg = std::mem::zeroed();
        let mut c_reg = std::mem::zeroed();
        r::get_reg_dword(hklm, sub.as_ptr(), value.as_ptr(), &mut rust_reg);
        c::oracle_get_reg_dword(hklm, sub.as_ptr(), value.as_ptr(), &mut c_reg);
        assert_eq!(rust_reg.status, c_reg.status);
        assert_eq!(rust_reg.size, c_reg.size);
        assert_eq!(rust_reg.return_value, c_reg.return_value);
        assert_eq!(rust_reg.status, 0);
    }
}

#[test]
#[expect(
    clippy::cognitive_complexity,
    reason = "parity table: one block per ported C case"
)]
fn load_uuid_and_inhibit_match_where_they_are_stable() {
    let _guard = PARITY.lock().unwrap_or_else(|e| e.into_inner());
    let kernel = cstring(b"kernel32");
    let sym = cstring(b"GetCurrentProcess");
    // SAFETY: the names are C strings. Both modules are closed.
    unsafe {
        let rust_mod = r::os_dlopen(kernel.as_ptr());
        let c_mod = c::oracle_os_dlopen(kernel.as_ptr());
        assert!(!rust_mod.is_null());
        assert!(!c_mod.is_null());
        assert!(!r::os_dlsym(rust_mod, sym.as_ptr()).is_null());
        assert!(!c::oracle_os_dlsym(c_mod, sym.as_ptr()).is_null());
        r::os_dlclose(rust_mod);
        c::oracle_os_dlclose(c_mod);
        assert!(r::os_dlopen(std::ptr::null()).is_null());
        assert!(c::oracle_os_dlopen(std::ptr::null()).is_null());
        assert!(r::os_request_high_performance(std::ptr::null()).is_null());
        assert!(c::oracle_os_request_high_performance(std::ptr::null()).is_null());
        assert!(!r::os_sleepto_ns(0));
        assert!(!c::oracle_os_sleepto_ns(0));
        assert!(!r::os_sleepto_ns_fast(0));
        assert!(!c::oracle_os_sleepto_ns_fast(0));
    }

    let plugin_dir = scratch();
    let plugin_path = plugin_dir.join("not-a-plugin.txt");
    let text = cstring(&path_bytes(&plugin_path));
    fs::write(&plugin_path, b"nope").unwrap();
    // SAFETY: the path is a C string. The bool out-pointer is writable.
    unsafe {
        assert!(!r::os_is_obs_plugin(text.as_ptr()));
        assert!(!c::oracle_os_is_obs_plugin(text.as_ptr()));
        let mut rust_flag = true;
        let mut c_flag = true;
        r::get_plugin_info(text.as_ptr(), &mut rust_flag);
        c::oracle_get_plugin_info(text.as_ptr(), &mut c_flag);
        assert!(!rust_flag);
        assert!(!c_flag);
    }

    // SAFETY: both strings are `bmalloc`'d and freed here.
    unsafe {
        let rust_uuid = free_owned(r::os_generate_uuid());
        let c_uuid = free_owned(c::oracle_os_generate_uuid());
        assert!(regex_uuid(&rust_uuid), "{rust_uuid:?}");
        assert!(regex_uuid(&c_uuid), "{c_uuid:?}");
    }

    let dot = cstring(b".");
    // SAFETY: `.` is a C string. The usage struct is writable.
    unsafe {
        assert!(r::os_get_free_space(dot.as_ptr()) > 0);
        assert!(c::oracle_os_get_free_space(dot.as_ptr()) > 0);
        assert!(r::os_get_free_disk_space(dot.as_ptr()) > 0);
        assert!(c::oracle_os_get_free_disk_space(dot.as_ptr()) > 0);
        let mut usage = std::mem::zeroed();
        assert!(r::os_get_proc_memory_usage(&mut usage));
        assert!(usage.resident_size > 0);
        let mut c_usage = std::mem::zeroed();
        assert!(c::oracle_os_get_proc_memory_usage(&mut c_usage));
        assert!(c_usage.resident_size > 0);
    }

    // SAFETY: each inhibitor is destroyed, which clears the request.
    unsafe {
        let rust_info = r::os_inhibit_sleep_create(std::ptr::null());
        assert!(r::os_inhibit_sleep_set_active(rust_info, true));
        assert!(!r::os_inhibit_sleep_set_active(rust_info, true));
        r::os_inhibit_sleep_destroy(rust_info);
        let c_info = c::oracle_os_inhibit_sleep_create(std::ptr::null());
        assert!(c::oracle_os_inhibit_sleep_set_active(c_info, true));
        assert!(!c::oracle_os_inhibit_sleep_set_active(c_info, true));
        c::oracle_os_inhibit_sleep_destroy(c_info);
    }
}

fn regex_uuid(bytes: &[u8]) -> bool {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return false;
    };
    let mut parts = text.split('-');
    let widths = [8, 4, 4, 4, 12];
    for width in widths {
        let Some(part) = parts.next() else {
            return false;
        };
        if part.len() != width
            || !part
                .chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        {
            return false;
        }
    }
    parts.next().is_none()
}

#[test]
fn chdir_matches_the_c_file() {
    if !crate::common::isolated(concat!(module_path!(), "::chdir_matches_the_c_file")) {
        return;
    }
    let _guard = PARITY.lock().unwrap_or_else(|e| e.into_inner());
    let mut saved = [0u8; 512];
    // SAFETY: `saved` is 512 bytes.
    let home = unsafe { r::os_getcwd(saved.as_mut_ptr().cast(), saved.len()) };
    assert!(!home.is_null());
    let dir = scratch();
    let dir_c = cstring(&path_bytes(&dir));
    let mut rust_cwd = [0u8; 512];
    let mut c_cwd = [0u8; 512];
    // SAFETY: the path is NUL-terminated. Both cwd buffers are 512 bytes.
    unsafe {
        assert_eq!(r::os_chdir(dir_c.as_ptr()), 0);
        assert!(!r::os_getcwd(rust_cwd.as_mut_ptr().cast(), rust_cwd.len()).is_null());
        assert!(!c::oracle_os_getcwd(c_cwd.as_mut_ptr().cast(), c_cwd.len()).is_null());
        assert_eq!(
            owned(rust_cwd.as_ptr().cast()),
            owned(c_cwd.as_ptr().cast())
        );
        assert_eq!(c::oracle_os_chdir(saved.as_ptr().cast()), 0);
    }
}
