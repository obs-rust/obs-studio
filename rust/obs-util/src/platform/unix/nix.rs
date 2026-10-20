//! POSIX half of `libobs/util/platform-nix.c` (macOS, Linux, FreeBSD).
//!
//! `unsafe` is the libc calls that have to match the C errno, modes, and
//! flags (`mkdir` 0755, `realpath`, `glob`, `dlopen`, `nanosleep`, `uuid`).
//! The safe signatures take byte paths and do not dereference null.

#![allow(unsafe_code)]

use std::ffi::{CStr, CString};
use std::io::Error;
use std::mem::MaybeUninit;
use std::os::raw::{c_char, c_int, c_void};

use crate::base::{LOG_DEBUG, LOG_ERROR};

const MKDIR_SUCCESS: i32 = 0;
const MKDIR_EXISTS: i32 = 1;
const MKDIR_ERROR: i32 = -1;

fn c_path(path: &[u8]) -> Option<CString> {
    CString::new(path).ok()
}

fn errno() -> i32 {
    Error::last_os_error().raw_os_error().unwrap_or(0)
}

unsafe extern "C" {
    fn blog(level: c_int, fmt: *const c_char, ...);
    fn dlerror() -> *mut c_char;
    fn uuid_generate(out: *mut u8);
    fn uuid_unparse_lower(uu: *const u8, out: *mut c_char);
    #[cfg(target_os = "macos")]
    fn clock_gettime_nsec_np(clock_id: libc::clockid_t) -> u64;
}

#[cfg(target_os = "linux")]
#[link(name = "uuid")]
unsafe extern "C" {}

/// `os_file_exists`.
pub fn file_exists(path: &[u8]) -> bool {
    let Some(path) = c_path(path) else {
        return false;
    };
    // SAFETY: `path` is a NUL-terminated path.
    unsafe { libc::access(path.as_ptr(), libc::F_OK) == 0 }
}

/// `os_unlink`. Returns 0 or -1, as `unlink(2)` does.
pub fn unlink(path: &[u8]) -> i32 {
    let Some(path) = c_path(path) else {
        return -1;
    };
    // SAFETY: `path` is a NUL-terminated path.
    unsafe { libc::unlink(path.as_ptr()) }
}

/// `os_rmdir`.
pub fn rmdir(path: &[u8]) -> i32 {
    let Some(path) = c_path(path) else {
        return -1;
    };
    // SAFETY: `path` is a NUL-terminated path.
    unsafe { libc::rmdir(path.as_ptr()) }
}

/// `os_mkdir`. `0` created, `1` already exists, `-1` any other error.
pub fn mkdir(path: &[u8]) -> i32 {
    let Some(path) = c_path(path) else {
        return MKDIR_ERROR;
    };
    // SAFETY: `path` is a NUL-terminated path. Mode 0755 matches the C call.
    let rc = unsafe { libc::mkdir(path.as_ptr(), 0o755) };
    if rc == 0 {
        MKDIR_SUCCESS
    } else if errno() == libc::EEXIST {
        MKDIR_EXISTS
    } else {
        MKDIR_ERROR
    }
}

/// `os_rename`.
pub fn rename(from: &[u8], to: &[u8]) -> i32 {
    let (Some(from), Some(to)) = (c_path(from), c_path(to)) else {
        return -1;
    };
    // SAFETY: both paths are NUL-terminated.
    unsafe { libc::rename(from.as_ptr(), to.as_ptr()) }
}

/// `os_safe_replace`. A missing `backup` skips the backup rename. An empty
/// backup is still a backup path, matching a non-NULL C pointer.
pub fn safe_replace(target: &[u8], from: &[u8], backup: Option<&[u8]>) -> i32 {
    if let Some(backup) = backup
        && file_exists(target)
        && rename(target, backup) != 0
    {
        return -1;
    }
    rename(from, target)
}

fn is_dir(path: &CStr) -> bool {
    let mut st = MaybeUninit::<libc::stat>::uninit();
    // SAFETY: `path` is NUL-terminated and `st` is a stat buffer.
    let rc = unsafe { libc::stat(path.as_ptr(), st.as_mut_ptr()) };
    if rc == 0 {
        // SAFETY: `stat` succeeded, so `st` is initialized.
        let mode = unsafe { st.assume_init() }.st_mode;
        return (mode & libc::S_IFMT) == libc::S_IFDIR;
    }
    let err = errno();
    // SAFETY: `path` outlives the call. The format matches platform-nix.c.
    unsafe {
        blog(
            LOG_DEBUG,
            c"is_dir: stat for %s failed, errno: %d".as_ptr(),
            path.as_ptr(),
            err,
        );
    }
    false
}

/// One `os_readdir` result. `.` and `..` are included.
pub struct DirEnt {
    /// `d_name` without the NUL.
    pub name: Vec<u8>,
    /// `directory`.
    pub directory: bool,
}

/// `os_opendir` / `os_readdir` / `os_closedir`.
///
/// The handle owns a copy of the path. C keeps the caller's pointer.
pub struct Dir {
    path: CString,
    dir: *mut libc::DIR,
}

// SAFETY: the `DIR*` is used from one thread, the same rule as `readdir`.
unsafe impl Send for Dir {}

impl Drop for Dir {
    fn drop(&mut self) {
        if !self.dir.is_null() {
            // SAFETY: `dir` came from `opendir`.
            unsafe { libc::closedir(self.dir) };
            self.dir = std::ptr::null_mut();
        }
    }
}

/// `os_opendir`. `None` when `opendir` fails.
pub fn opendir(path: &[u8]) -> Option<Dir> {
    let path = c_path(path)?;
    // SAFETY: `path` is NUL-terminated.
    let dir = unsafe { libc::opendir(path.as_ptr()) };
    if dir.is_null() {
        return None;
    }
    Some(Dir { path, dir })
}

impl Dir {
    /// `os_readdir`. `None` at the end, or when the name does not fit in
    /// `d_name[256]` (that entry is consumed, as in C).
    pub fn read(&mut self) -> Option<DirEnt> {
        // SAFETY: `dir` came from `opendir` and is closed in `Drop`.
        let ent = unsafe { libc::readdir(self.dir) };
        if ent.is_null() {
            return None;
        }
        // SAFETY: `readdir` returned a live `dirent`.
        let name_ptr = unsafe { (*ent).d_name.as_ptr() };
        // SAFETY: `d_name` is a NUL-terminated name.
        let name = unsafe { CStr::from_ptr(name_ptr) }.to_bytes();
        if name.len() >= 256 {
            return None;
        }
        let mut full = Vec::with_capacity(self.path.as_bytes().len() + 1 + name.len());
        full.extend_from_slice(self.path.as_bytes());
        full.push(b'/');
        full.extend_from_slice(name);
        let directory = c_path(&full).is_some_and(|p| is_dir(&p));
        Some(DirEnt {
            name: name.to_vec(),
            directory,
        })
    }
}

/// `os_get_abs_path`. Writes into `dst` the way `snprintf` does and returns
/// the untruncated length. `0` when `realpath` fails or `dst` is absent.
pub fn abs_path_into(path: &[u8], dst: Option<&mut [u8]>) -> usize {
    let Some(dst) = dst else {
        return 0;
    };
    let Some(path) = c_path(path) else {
        return 0;
    };
    let mut buf = [0u8; libc::PATH_MAX as usize];
    // SAFETY: `buf` is PATH_MAX bytes, which is what `realpath` requires.
    let rc = unsafe { libc::realpath(path.as_ptr(), buf.as_mut_ptr().cast()) };
    if rc.is_null() {
        return 0;
    }
    // SAFETY: `realpath` wrote a NUL-terminated path into `buf`.
    let full = unsafe { CStr::from_ptr(rc) }.to_bytes();
    let limit = dst.len().min(libc::PATH_MAX as usize);
    if limit == 0 {
        return full.len();
    }
    let copy = full.len().min(limit - 1);
    dst[..copy].copy_from_slice(&full[..copy]);
    dst[copy] = 0;
    full.len()
}

/// `os_get_abs_path_ptr` as owned bytes, without the NUL. `None` on failure.
pub fn abs_path(path: &[u8]) -> Option<Vec<u8>> {
    let mut buf = [0u8; 512];
    let n = abs_path_into(path, Some(&mut buf));
    if n == 0 {
        return None;
    }
    let end = buf.iter().position(|b| *b == 0).unwrap_or(buf.len());
    Some(buf[..end].to_vec())
}

/// `os_glob` result entry.
pub struct GlobEnt {
    /// Path bytes without the NUL.
    pub path: Vec<u8>,
    /// `directory`.
    pub directory: bool,
}

/// `os_glob`. The `flags` argument is ignored, as in C. `Err` is the `glob`
/// return (`GLOB_NOMATCH` and the rest), and `Ok` is the matches.
pub fn glob(pattern: &[u8]) -> Result<Vec<GlobEnt>, i32> {
    let Some(pattern) = c_path(pattern) else {
        return Err(libc::GLOB_NOMATCH);
    };
    let mut gl = MaybeUninit::<libc::glob_t>::zeroed();
    // SAFETY: `gl` is zeroed and `pattern` is NUL-terminated. Flags are 0.
    let rc = unsafe { libc::glob(pattern.as_ptr(), 0, None, gl.as_mut_ptr()) };
    if rc != 0 {
        return Err(rc);
    }
    // SAFETY: `glob` succeeded, so `gl` is initialized.
    let gl = unsafe { gl.assume_init() };
    let mut out = Vec::with_capacity(gl.gl_pathc);
    for i in 0..gl.gl_pathc {
        // SAFETY: `i < gl_pathc`, so `gl_pathv[i]` is a live path.
        let p = unsafe { *gl.gl_pathv.add(i) };
        // SAFETY: `glob` paths are NUL-terminated.
        let bytes = unsafe { CStr::from_ptr(p) }.to_bytes().to_vec();
        let directory = is_dir(unsafe { CStr::from_ptr(p) });
        out.push(GlobEnt {
            path: bytes,
            directory,
        });
    }
    let mut gl = gl;
    // SAFETY: `gl` came from a successful `glob`.
    unsafe { libc::globfree(&mut gl) };
    Ok(out)
}

/// `os_copyfile`. `-1` when the destination exists or a read or open fails.
pub fn copy_file(src: &[u8], dst: &[u8]) -> i32 {
    if file_exists(dst) {
        return -1;
    }
    let (Some(src), Some(dst)) = (c_path(src), c_path(dst)) else {
        return -1;
    };
    // SAFETY: both paths are NUL-terminated. "rb" and "ab+" match the C modes.
    let file_in = unsafe { libc::fopen(src.as_ptr(), c"rb".as_ptr()) };
    if file_in.is_null() {
        return -1;
    }
    let file_out = unsafe { libc::fopen(dst.as_ptr(), c"ab+".as_ptr()) };
    if file_out.is_null() {
        // SAFETY: `file_in` came from `fopen`.
        unsafe { libc::fclose(file_in) };
        return -1;
    }
    let mut data = [0u8; 4096];
    let ret;
    loop {
        // SAFETY: `file_in` is open and `data` is 4096 bytes.
        let n = unsafe { libc::fread(data.as_mut_ptr().cast(), 1, data.len(), file_in) };
        let mut wrote = n;
        if n > 0 {
            // SAFETY: `file_out` is open and `data` holds `n` bytes.
            wrote = unsafe { libc::fwrite(data.as_ptr().cast(), 1, n, file_out) };
        }
        if wrote != data.len() {
            // SAFETY: `file_in` is open.
            ret = if unsafe { libc::feof(file_in) } != 0 {
                0
            } else {
                -1
            };
            break;
        }
    }
    // SAFETY: both FILEs came from `fopen`.
    unsafe {
        libc::fclose(file_out);
        libc::fclose(file_in);
    }
    ret
}

/// `os_getcwd` into a caller buffer. `false` when `getcwd` fails, including
/// a zero-length buffer (`ERANGE` in C).
pub fn getcwd_into(buf: &mut [u8]) -> bool {
    if buf.is_empty() {
        return false;
    }
    // SAFETY: `buf` is the caller storage `getcwd` writes.
    let rc = unsafe { libc::getcwd(buf.as_mut_ptr().cast(), buf.len()) };
    !rc.is_null()
}

/// `os_getcwd`. `None` on failure. The bytes have no trailing NUL.
pub fn getcwd() -> Option<Vec<u8>> {
    let mut buf = [0u8; libc::PATH_MAX as usize];
    if !getcwd_into(&mut buf) {
        return None;
    }
    let end = buf.iter().position(|b| *b == 0).unwrap_or(buf.len());
    Some(buf[..end].to_vec())
}

/// `snprintf(dst, size, "%s", bytes)`. Returns the untruncated length.
/// A missing buffer writes nothing, matching `snprintf(NULL, 0, ...)`.
pub fn snprintf_bytes(dst: Option<&mut [u8]>, bytes: &[u8]) -> i32 {
    // Longer than INT_MAX is EOVERFLOW: snprintf returns -1.
    let Ok(n) = i32::try_from(bytes.len()) else {
        return -1;
    };
    let Some(dst) = dst else {
        return n;
    };
    if dst.is_empty() {
        return n;
    }
    let copy = bytes.len().min(dst.len() - 1);
    dst[..copy].copy_from_slice(&bytes[..copy]);
    dst[copy] = 0;
    n
}

/// `dirname(3)` result, without the slash `os_get_executable_path_ptr` adds.
pub fn parent_dir(path: &[u8]) -> &[u8] {
    let mut end = path.len();
    while end > 1 && path[end - 1] == b'/' {
        end -= 1;
    }
    match path[..end].iter().rposition(|b| *b == b'/') {
        None => b".",
        Some(0) => b"/",
        Some(i) => &path[..i],
    }
}

fn atoi_c(s: &str) -> i32 {
    let s = s.trim_start_matches(|c: char| c.is_ascii_whitespace());
    let (neg, s) = if let Some(rest) = s.strip_prefix('-') {
        (true, rest)
    } else {
        (false, s.strip_prefix('+').unwrap_or(s))
    };
    let mut n: i32 = 0;
    for c in s.chars() {
        if !c.is_ascii_digit() {
            break;
        }
        n = n.wrapping_mul(10).wrapping_add((c as u8 - b'0') as i32);
    }
    if neg { n.wrapping_neg() } else { n }
}

fn after_colon(rest: &str) -> Option<&str> {
    let start = rest.find(':')?;
    let value = &rest[start + 1..];
    if value.is_empty() {
        return None;
    }
    Some(value)
}

/// Physical cores from a `/proc/cpuinfo` body. `logical` is the fallback
/// when no `cpu cores` line is counted. `dstr_find` is a substring search,
/// so an id of `1` is treated as already seen inside `10`.
pub fn cpuinfo_physical_cores(text: &str, logical: i32) -> i32 {
    let mut proc_phys_id = String::new();
    let mut seen = String::new();
    let mut core_count = 0i32;
    for line in text.split_inclusive('\n') {
        if let Some(rest) = line.strip_prefix("physical id")
            && let Some(value) = after_colon(rest)
        {
            proc_phys_id = atoi_c(value).to_string();
        }
        if let Some(rest) = line.strip_prefix("cpu cores")
            && let Some(value) = after_colon(rest)
            && (seen.is_empty() || !seen.contains(&proc_phys_id))
        {
            seen.push_str(&proc_phys_id);
            seen.push(' ');
            core_count = core_count.wrapping_add(atoi_c(value));
        }
    }
    if core_count == 0 { logical } else { core_count }
}

fn find_bytes(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

fn atoi_bytes(s: &[u8]) -> i32 {
    let text = String::from_utf8_lossy(s);
    atoi_c(&text)
}

/// Physical cores from a FreeBSD `/var/run/dmesg.boot` body. `logical` is
/// the fallback when the second `FreeBSD/SMP:` line is missing or names
/// zero packages.
pub fn freebsd_smp_cores(text: &[u8], logical: i32) -> i32 {
    let marker = b"\nFreeBSD/SMP: ";
    let Some(first) = find_bytes(text, marker) else {
        return logical;
    };
    let after_first = &text[first + 1..];
    let Some(second) = find_bytes(after_first, marker) else {
        return logical;
    };
    let line = &after_first[second..];
    let Some(colon) = find_bytes(line, b": ") else {
        return logical;
    };
    let mut rest = &line[colon + 2..];
    let pack_len = rest.iter().position(|b| *b == b' ').unwrap_or(rest.len());
    let packages = atoi_bytes(&rest[..pack_len]);
    let Some(px) = find_bytes(rest, b"package(s) x ") else {
        return if packages == 0 { logical } else { packages };
    };
    rest = &rest[px + b"package(s) x ".len()..];
    let core_len = rest.iter().position(|b| *b == b' ').unwrap_or(rest.len());
    let cores = atoi_bytes(&rest[..core_len]);
    if packages == 0 {
        logical
    } else if cores == 0 {
        packages
    } else {
        packages.saturating_mul(cores)
    }
}

/// `os_chdir`.
pub fn chdir(path: &[u8]) -> i32 {
    let Some(path) = c_path(path) else {
        return -1;
    };
    // SAFETY: `path` is NUL-terminated.
    unsafe { libc::chdir(path.as_ptr()) }
}

fn dylib_name(path: &[u8]) -> Option<CString> {
    let mut name = path.to_vec();
    let has = |needle: &[u8]| name.windows(needle.len()).any(|w| w == needle);
    let append = if cfg!(target_os = "macos") {
        !(has(b".framework") || has(b".plugin") || has(b".dylib") || has(b".so"))
    } else {
        !has(b".so")
    };
    if append {
        name.extend_from_slice(b".so");
    }
    CString::new(name).ok()
}

/// macOS adds `RTLD_FIRST`, which Linux and FreeBSD libc do not define.
/// `cfg!` would still name that constant on those targets.
#[cfg(target_os = "macos")]
fn dlopen_flags(name: &CStr) -> c_int {
    let mut flags = libc::RTLD_NOW | libc::RTLD_FIRST;
    if name.to_bytes().windows(6).any(|w| w == b"Python") {
        flags |= libc::RTLD_GLOBAL;
    } else {
        flags |= libc::RTLD_LOCAL;
    }
    flags
}

#[cfg(not(target_os = "macos"))]
fn dlopen_flags(_name: &CStr) -> c_int {
    libc::RTLD_NOW
}

/// `os_dlopen`. `None` on failure, after the C `blog(LOG_ERROR, ...)` line.
pub fn dlopen(path: &[u8]) -> Option<*mut c_void> {
    let name = dylib_name(path)?;
    let flags = dlopen_flags(&name);
    // SAFETY: `name` is NUL-terminated.
    let handle = unsafe { libc::dlopen(name.as_ptr(), flags) };
    if handle.is_null() {
        let orig = c_path(path).unwrap_or_else(|| CString::new("").unwrap());
        // SAFETY: `dlerror` returns a C string for the calling thread.
        let err = unsafe { dlerror() };
        // SAFETY: the pointers are live C strings. Format matches the C log.
        unsafe {
            blog(
                LOG_ERROR,
                c"os_dlopen(%s->%s): %s\n".as_ptr(),
                orig.as_ptr(),
                name.as_ptr(),
                err,
            );
        }
        return None;
    }
    Some(handle)
}

/// `os_dlsym`.
///
/// # Safety
/// `module` is a `dlopen` handle or null.
pub unsafe fn dlsym(module: *mut c_void, func: &[u8]) -> *mut c_void {
    let Some(func) = c_path(func) else {
        return std::ptr::null_mut();
    };
    // SAFETY: `module` is a `dlopen` handle or null, and `func` is NUL-terminated.
    unsafe { libc::dlsym(module, func.as_ptr()) }
}

/// `os_dlclose`. A null handle is ignored.
///
/// # Safety
/// `module` is a `dlopen` handle or null.
pub unsafe fn dlclose(module: *mut c_void) {
    if !module.is_null() {
        // SAFETY: `module` came from `dlopen`.
        unsafe { libc::dlclose(module) };
    }
}

/// `os_is_obs_plugin`. Always true on this platform.
pub fn is_obs_plugin(_path: &[u8]) -> bool {
    true
}

/// Monotonic nanoseconds. macOS uses `CLOCK_UPTIME_RAW`, the same clock
/// the Cocoa port returns and the clock `os_sleepto_ns` subtracts. Linux
/// and the BSDs use `CLOCK_MONOTONIC`, as this file does.
pub fn gettime_ns() -> u64 {
    #[cfg(target_os = "macos")]
    {
        // SAFETY: `CLOCK_UPTIME_RAW` is a valid clock id.
        unsafe { clock_gettime_nsec_np(libc::CLOCK_UPTIME_RAW) }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let mut ts = MaybeUninit::<libc::timespec>::uninit();
        // SAFETY: `ts` is a timespec buffer.
        let rc = unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, ts.as_mut_ptr()) };
        if rc != 0 {
            return 0;
        }
        // SAFETY: `clock_gettime` succeeded.
        let ts = unsafe { ts.assume_init() };
        (ts.tv_sec as u64) * 1_000_000_000 + (ts.tv_nsec as u64)
    }
}

/// `os_sleepto_ns`. `false` when `time_target` is already past.
pub fn sleepto_ns(time_target: u64) -> bool {
    let current = gettime_ns();
    if time_target < current {
        return false;
    }
    let mut remain_ns = time_target - current;
    while remain_ns > 0 {
        // Zero first. Some libc targets add private padding to `timespec`,
        // so a two-field literal does not compile there. C memsets both.
        // SAFETY: `timespec` is a C struct of integers. Zero is a valid value.
        let (mut req, mut left) = unsafe {
            (
                MaybeUninit::<libc::timespec>::zeroed().assume_init(),
                MaybeUninit::<libc::timespec>::zeroed().assume_init(),
            )
        };
        req.tv_sec = (remain_ns / 1_000_000_000) as libc::time_t;
        req.tv_nsec = (remain_ns % 1_000_000_000) as _;
        // SAFETY: both timespecs are initialized.
        let rc = unsafe { libc::nanosleep(&req, &mut left) };
        if rc == 0 {
            break;
        }
        remain_ns = (left.tv_sec as u64) * 1_000_000_000 + (left.tv_nsec as u64);
    }
    true
}

/// `os_sleepto_ns_fast`.
pub fn sleepto_ns_fast(time_target: u64) -> bool {
    let mut current = gettime_ns();
    if time_target < current {
        return false;
    }
    while time_target > current {
        let remain_us = (time_target - current).div_ceil(1000);
        let us = if remain_us >= 1_000_000 {
            999_999
        } else {
            remain_us as u32
        };
        // SAFETY: `us` is below 1_000_000, as the C clamps it.
        unsafe { libc::usleep(us) };
        current = gettime_ns();
    }
    true
}

/// `os_sleep_ms`. The multiply wraps, as the C `duration * 1000` does.
pub fn sleep_ms(duration: u32) {
    // SAFETY: the duration is the wrapped C product.
    unsafe { libc::usleep(duration.wrapping_mul(1000)) };
}

/// `os_breakpoint`.
pub fn breakpoint() {
    // SAFETY: `SIGTRAP` is a valid signal.
    unsafe { libc::raise(libc::SIGTRAP) };
}

/// `os_oom`. Same signal as `os_breakpoint` in this file.
pub fn oom() {
    breakpoint();
}

/// `os_generate_uuid`. 36 lowercase hex chars, no NUL in the `Vec`.
pub fn generate_uuid() -> [u8; 36] {
    let mut uuid = [0u8; 16];
    let mut text = [0u8; 37];
    // SAFETY: both buffers match the libuuid contract (16 and 37 bytes).
    unsafe {
        uuid_generate(uuid.as_mut_ptr());
        uuid_unparse_lower(uuid.as_ptr(), text.as_mut_ptr().cast());
    }
    let mut out = [0u8; 36];
    out.copy_from_slice(&text[..36]);
    out
}

/// `os_get_program_data_path` text, without a NUL. A missing name is empty.
/// `None` when the text is longer than `INT_MAX`, where `snprintf` fails.
pub fn program_data_path(name: Option<&[u8]>) -> Option<Vec<u8>> {
    const PREFIX: &[u8] = b"/usr/local/share/";
    if PREFIX.len() + name.map_or(0, <[u8]>::len) > i32::MAX as usize {
        return None;
    }
    let mut out = PREFIX.to_vec();
    if let Some(name) = name {
        out.extend_from_slice(name);
    }
    Some(out)
}

// The rest of platform-nix.c is under `#if !defined(__APPLE__)`. macOS
// implements those symbols in platform-cocoa.m, so defining them here would
// collide when both are linked. `os_inhibit_sleep_*` on Linux and FreeBSD is
// util/platform-nix-inhibit.c, ported in `super::nix_inhibit`.
#[cfg(not(target_os = "macos"))]
mod host {
    use super::*;
    use std::sync::Mutex;

    /// `os_cpu_usage_info`.
    pub struct CpuUsage {
        last_cpu: libc::clock_t,
        last_sys: libc::clock_t,
        last_user: libc::clock_t,
        cores: i32,
    }

    /// `os_cpu_usage_info_start`.
    pub fn cpu_start() -> CpuUsage {
        let mut sample = MaybeUninit::<libc::tms>::zeroed();
        // SAFETY: `sample` is a tms buffer.
        let cpu = unsafe { libc::times(sample.as_mut_ptr()) };
        // SAFETY: `times` wrote the tms, or left it zeroed on failure.
        let sample = unsafe { sample.assume_init() };
        let cores = unsafe { libc::sysconf(libc::_SC_NPROCESSORS_ONLN) } as i32;
        CpuUsage {
            last_cpu: cpu,
            last_sys: sample.tms_stime,
            last_user: sample.tms_utime,
            cores,
        }
    }

    impl CpuUsage {
        /// `os_cpu_usage_info_query`. `0.0` when the sample did not advance.
        pub fn query(&mut self) -> f64 {
            let mut sample = MaybeUninit::<libc::tms>::zeroed();
            // SAFETY: `sample` is a tms buffer.
            let cpu = unsafe { libc::times(sample.as_mut_ptr()) };
            // SAFETY: `times` wrote the tms.
            let sample = unsafe { sample.assume_init() };
            if cpu <= self.last_cpu
                || sample.tms_stime < self.last_sys
                || sample.tms_utime < self.last_user
            {
                return 0.0;
            }
            let mut percent =
                (sample.tms_stime - self.last_sys + (sample.tms_utime - self.last_user)) as f64;
            percent /= (cpu - self.last_cpu) as f64;
            percent /= self.cores as f64;
            self.last_cpu = cpu;
            self.last_sys = sample.tms_stime;
            self.last_user = sample.tms_utime;
            percent * 100.0
        }
    }

    fn env_bytes(key: &CStr) -> Option<Vec<u8>> {
        // SAFETY: `key` is NUL-terminated. The pointer is only read immediately.
        let p = unsafe { libc::getenv(key.as_ptr()) };
        if p.is_null() {
            return None;
        }
        // SAFETY: `getenv` returned a NUL-terminated value.
        Some(unsafe { CStr::from_ptr(p) }.to_bytes().to_vec())
    }

    fn join_path(parts: &[&[u8]]) -> Vec<u8> {
        let mut out = Vec::new();
        for (i, part) in parts.iter().enumerate() {
            if i > 0 {
                out.push(b'/');
            }
            out.extend_from_slice(part);
        }
        out
    }

    /// Where `os_get_config_path` should crash instead of returning.
    pub struct MissingHome;

    /// `os_get_config_path` body. `Err` is the missing-`$HOME` crash.
    pub fn config_path(name: Option<&[u8]>) -> Result<Vec<u8>, MissingHome> {
        if let Some(xdg) = env_bytes(c"XDG_CONFIG_HOME") {
            if name.is_none_or(|n| n.is_empty()) {
                return Ok(xdg);
            }
            return Ok(join_path(&[&xdg, name.unwrap_or(b"")]));
        }
        let Some(home) = env_bytes(c"HOME") else {
            return Err(MissingHome);
        };
        if name.is_none_or(|n| n.is_empty()) {
            let mut out = home;
            out.extend_from_slice(b"/.config");
            return Ok(out);
        }
        Ok(join_path(&[&home, b".config", name.unwrap_or(b"")]))
    }

    /// `os_get_config_path_ptr` body. Always inserts a slash before `name`.
    pub fn config_path_ptr(name: Option<&[u8]>) -> Result<Vec<u8>, MissingHome> {
        if let Some(xdg) = env_bytes(c"XDG_CONFIG_HOME") {
            let mut out = xdg;
            out.push(b'/');
            if let Some(name) = name {
                out.extend_from_slice(name);
            }
            return Ok(out);
        }
        let Some(home) = env_bytes(c"HOME") else {
            return Err(MissingHome);
        };
        let mut out = home;
        out.extend_from_slice(b"/.config/");
        if let Some(name) = name {
            out.extend_from_slice(name);
        }
        Ok(out)
    }

    /// `os_get_executable_path_ptr` body, without the NUL.
    pub fn executable_path(name: Option<&[u8]>) -> Option<Vec<u8>> {
        let exe = read_exe()?;
        let mut out = super::parent_dir(&exe).to_vec();
        out.push(b'/');
        if let Some(name) = name.filter(|n| !n.is_empty()) {
            out.extend_from_slice(name);
        }
        Some(out)
    }

    #[cfg(any(target_os = "freebsd", target_os = "dragonfly"))]
    fn read_exe() -> Option<Vec<u8>> {
        let mut mib = [
            libc::CTL_KERN,
            libc::KERN_PROC,
            libc::KERN_PROC_PATHNAME,
            -1,
        ];
        let mut buf = [0u8; libc::PATH_MAX as usize];
        let mut len = buf.len();
        // SAFETY: `mib` is the KERN_PROC_PATHNAME query and `buf` is PATH_MAX.
        let rc = unsafe {
            libc::sysctl(
                mib.as_mut_ptr(),
                mib.len() as libc::c_uint,
                buf.as_mut_ptr().cast(),
                &mut len,
                std::ptr::null(),
                0,
            )
        };
        if rc == -1 {
            let err = errno();
            // SAFETY: the format matches the C log.
            unsafe {
                blog(
                    LOG_ERROR,
                    c"sysctl(KERN_PROC_PATHNAME) failed, errno %d".as_ptr(),
                    err,
                );
            }
            return None;
        }
        let n = len.min(buf.len());
        let end = buf[..n].iter().position(|b| *b == 0).unwrap_or(n);
        Some(buf[..end].to_vec())
    }

    #[cfg(not(any(target_os = "freebsd", target_os = "dragonfly")))]
    fn read_exe() -> Option<Vec<u8>> {
        let mut buf = [0u8; libc::PATH_MAX as usize];
        // SAFETY: `buf` holds PATH_MAX bytes. The C passes PATH_MAX - 1.
        let n = unsafe {
            libc::readlink(
                c"/proc/self/exe".as_ptr(),
                buf.as_mut_ptr().cast(),
                (libc::PATH_MAX as usize) - 1,
            )
        };
        if n < 0 {
            return None;
        }
        Some(buf[..n as usize].to_vec())
    }

    /// `os_get_emulation_status`.
    pub fn emulation_status() -> bool {
        false
    }

    /// `os_get_free_space`. `-1` when `statvfs` fails, otherwise `f_bsize * f_bfree`.
    pub fn free_space(path: &[u8]) -> i64 {
        let Some(path) = c_path(path) else {
            return -1;
        };
        let mut info = MaybeUninit::<libc::statvfs>::uninit();
        // SAFETY: `info` is a statvfs buffer and `path` is NUL-terminated.
        let rc = unsafe { libc::statvfs(path.as_ptr(), info.as_mut_ptr()) };
        if rc != 0 {
            return rc as i64;
        }
        // SAFETY: `statvfs` succeeded.
        let info = unsafe { info.assume_init() };
        (info.f_bsize as i64) * (info.f_bfree as i64)
    }

    /// `os_get_free_disk_space`. `0` on failure.
    // f_frsize and f_bavail are u64 on 64-bit Linux and a narrower
    // unsigned type on 32-bit Linux and the BSDs, so the casts stay.
    #[allow(clippy::unnecessary_cast)]
    pub fn free_disk_space(path: &[u8]) -> u64 {
        let Some(path) = c_path(path) else {
            return 0;
        };
        let mut info = MaybeUninit::<libc::statvfs>::uninit();
        // SAFETY: `info` is a statvfs buffer and `path` is NUL-terminated.
        let rc = unsafe { libc::statvfs(path.as_ptr(), info.as_mut_ptr()) };
        if rc != 0 {
            return 0;
        }
        // SAFETY: `statvfs` succeeded.
        let info = unsafe { info.assume_init() };
        (info.f_frsize as u64) * (info.f_bavail as u64)
    }

    fn logical_cores() -> i32 {
        // SAFETY: `_SC_NPROCESSORS_ONLN` is a valid sysconf name.
        unsafe { libc::sysconf(libc::_SC_NPROCESSORS_ONLN) as i32 }
    }

    #[cfg(target_os = "linux")]
    fn physical_cores(logical: i32) -> i32 {
        // A missing file leaves the C static at 0. A file that opens but
        // contributes no cores falls back to the logical count.
        match std::fs::read_to_string("/proc/cpuinfo") {
            Ok(text) => super::cpuinfo_physical_cores(&text, logical),
            Err(_) => 0,
        }
    }

    #[cfg(target_os = "freebsd")]
    fn physical_cores(logical: i32) -> i32 {
        match std::fs::read("/var/run/dmesg.boot") {
            Ok(text) if !text.is_empty() => super::freebsd_smp_cores(&text, logical),
            _ => logical,
        }
    }

    #[cfg(not(any(target_os = "linux", target_os = "freebsd")))]
    fn physical_cores(logical: i32) -> i32 {
        logical
    }

    struct CoreCount {
        physical: i32,
        logical: i32,
    }

    fn cores() -> CoreCount {
        static COUNTS: Mutex<Option<CoreCount>> = Mutex::new(None);
        let mut guard = COUNTS.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(counts) = guard.as_ref() {
            return CoreCount {
                physical: counts.physical,
                logical: counts.logical,
            };
        }
        let logical = logical_cores();
        let physical = physical_cores(logical);
        *guard = Some(CoreCount { physical, logical });
        CoreCount { physical, logical }
    }

    /// `os_get_physical_cores`.
    pub fn physical() -> i32 {
        cores().physical
    }

    /// `os_get_logical_cores`.
    pub fn logical() -> i32 {
        cores().logical
    }

    /// `os_get_proc_memory_usage` sizes. `virtual_size` is the raw `statm`
    /// field, not multiplied by the page size, as in C.
    pub struct ProcMem {
        /// Resident bytes.
        pub resident: u64,
        /// Virtual field from `statm`, unscaled.
        pub virtual_size: u64,
    }

    #[cfg(target_os = "freebsd")]
    fn read_kinfo() -> Option<libc::kinfo_proc> {
        let mut mib = [
            libc::CTL_KERN,
            libc::KERN_PROC,
            libc::KERN_PROC_PID,
            // SAFETY: `getpid` has no inputs. The mib slot is a `c_int`.
            unsafe { libc::getpid() as libc::c_int },
        ];
        let mut info = MaybeUninit::<libc::kinfo_proc>::zeroed();
        let mut len = std::mem::size_of::<libc::kinfo_proc>();
        // SAFETY: `mib` is KERN_PROC_PID for this process and `info` is the out buffer.
        let rc = unsafe {
            libc::sysctl(
                mib.as_mut_ptr(),
                mib.len() as libc::c_uint,
                info.as_mut_ptr().cast(),
                &mut len,
                std::ptr::null(),
                0,
            )
        };
        if rc < 0 {
            return None;
        }
        // SAFETY: `sysctl` succeeded and wrote `kinfo_proc`.
        Some(unsafe { info.assume_init() })
    }

    #[cfg(target_os = "freebsd")]
    pub fn proc_memory() -> Option<ProcMem> {
        let info = read_kinfo()?;
        // SAFETY: `_SC_PAGESIZE` is a valid sysconf name.
        let page = unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as u64;
        Some(ProcMem {
            resident: (info.ki_rssize as u64).wrapping_mul(page),
            virtual_size: info.ki_size as u64,
        })
    }

    /// `/proc/self/statm` on Linux and the other non-FreeBSD hosts. C leaves
    /// `virtual_size` as the raw field.
    #[cfg(not(target_os = "freebsd"))]
    pub fn proc_memory() -> Option<ProcMem> {
        let text = std::fs::read_to_string("/proc/self/statm").ok()?;
        let fields: Vec<&str> = text.split_whitespace().collect();
        if fields.len() != 7 {
            return None;
        }
        let virtual_size: u64 = fields[0].parse().ok()?;
        let resident: u64 = fields[1].parse().ok()?;
        // SAFETY: `_SC_PAGESIZE` is a valid sysconf name.
        let page = unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as u64;
        Some(ProcMem {
            resident: resident * page,
            virtual_size,
        })
    }

    // freeram and bufferram are u64 on 64-bit Linux and c_ulong on
    // 32-bit. u64::from covers both. mem_unit is c_uint either way.
    #[cfg(target_os = "linux")]
    #[allow(clippy::useless_conversion)]
    pub fn sys_free() -> u64 {
        let mut info = MaybeUninit::<libc::sysinfo>::uninit();
        // SAFETY: `info` is a sysinfo buffer.
        let rc = unsafe { libc::sysinfo(info.as_mut_ptr()) };
        if rc < 0 {
            return 0;
        }
        // SAFETY: `sysinfo` succeeded.
        let info = unsafe { info.assume_init() };
        (u64::from(info.freeram) + u64::from(info.bufferram)) * u64::from(info.mem_unit)
    }

    /// Raw `vm.stats.vm.v_free_count`, not multiplied by the page size.
    #[cfg(target_os = "freebsd")]
    pub fn sys_free() -> u64 {
        let mut mem_free: u64 = 0;
        let mut length = std::mem::size_of::<u64>();
        // SAFETY: the name is a NUL-terminated sysctl and `mem_free` is the out value.
        let rc = unsafe {
            libc::sysctlbyname(
                c"vm.stats.vm.v_free_count".as_ptr(),
                (&mut mem_free as *mut u64).cast(),
                &mut length,
                std::ptr::null(),
                0,
            )
        };
        if rc < 0 { 0 } else { mem_free }
    }

    #[cfg(not(any(target_os = "linux", target_os = "freebsd")))]
    pub fn sys_free() -> u64 {
        0
    }

    // totalram is u64 on 64-bit Linux and c_ulong on 32-bit.
    #[cfg(target_os = "linux")]
    #[allow(clippy::useless_conversion)]
    pub fn sys_total() -> u64 {
        static TOTAL: Mutex<Option<u64>> = Mutex::new(None);
        let mut guard = TOTAL.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(total) = *guard {
            return total;
        }
        let mut info = MaybeUninit::<libc::sysinfo>::uninit();
        // SAFETY: `info` is a sysinfo buffer.
        let rc = unsafe { libc::sysinfo(info.as_mut_ptr()) };
        let total = if rc < 0 {
            0
        } else {
            // SAFETY: `sysinfo` succeeded.
            let info = unsafe { info.assume_init() };
            u64::from(info.totalram) * u64::from(info.mem_unit)
        };
        *guard = Some(total);
        total
    }

    /// `hw.physmem` bytes. C calls libsysinfo `sysinfo` for this number.
    #[cfg(target_os = "freebsd")]
    pub fn sys_total() -> u64 {
        static TOTAL: Mutex<Option<u64>> = Mutex::new(None);
        let mut guard = TOTAL.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(total) = *guard {
            return total;
        }
        let mut mib = [libc::CTL_HW, libc::HW_PHYSMEM];
        let mut value: libc::c_ulong = 0;
        let mut len = std::mem::size_of::<libc::c_ulong>();
        // SAFETY: `mib` is HW_PHYSMEM and `value` is the out long.
        let rc = unsafe {
            libc::sysctl(
                mib.as_mut_ptr(),
                mib.len() as libc::c_uint,
                (&mut value as *mut libc::c_ulong).cast(),
                &mut len,
                std::ptr::null(),
                0,
            )
        };
        let total = if rc < 0 { 0 } else { value as u64 };
        *guard = Some(total);
        total
    }

    #[cfg(not(any(target_os = "linux", target_os = "freebsd")))]
    pub fn sys_total() -> u64 {
        0
    }
}

#[cfg(not(target_os = "macos"))]
pub use host::{
    CpuUsage, MissingHome, ProcMem, config_path, config_path_ptr, cpu_start, emulation_status,
    executable_path, free_disk_space, free_space, logical, physical, proc_memory, sys_free,
    sys_total,
};
