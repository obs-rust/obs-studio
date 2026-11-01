//! Windows half of `libobs/util/platform-windows.c`.
//!
//! Win32 is called the way that file calls it. `unsafe` is those calls.
//! The safe signatures take byte paths and do not dereference null.
//!
//! The mapped-image plugin scan faults on a short file. That `__try` stays
//! in `plugin_seh.c` (`obs_util_image_is_obs_plugin`).
//!
//! `DllMain`'s `PTW32_STATIC_LIB` pthread hooks are not part of the libobs
//! build. This keeps `timeBeginPeriod(1)` / `timeEndPeriod(1)` only.
//!
//! The version, core-count, and total-memory caches are taken under a mutex.
//! C used unsynchronized statics. The stored values are the same.

use core::cell::UnsafeCell;
use core::ffi::{c_char, c_int, c_void};
use core::ptr;
use std::sync::Mutex;

use crate::base::{LOG_ERROR, LOG_INFO};
use crate::platform::windows::utf8;
use crate::util_uint64::mul_div64;

const MAX_PATH: usize = 260;
const INVALID_HANDLE: isize = -1;
const ERROR_FILE_NOT_FOUND: u32 = 2;
const ERROR_INSUFFICIENT_BUFFER: u32 = 122;
const ERROR_ALREADY_EXISTS: u32 = 183;
const ERROR_PROC_NOT_FOUND: u32 = 127;
const ERROR_SUCCESS: i32 = 0;
const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;
const MOVEFILE_REPLACE_EXISTING: u32 = 0x1;
const GENERIC_READ: u32 = 0x8000_0000;
const FILE_SHARE_READ: u32 = 1;
const OPEN_EXISTING: u32 = 3;
const FILE_ATTRIBUTE_NORMAL: u32 = 0x80;
const PAGE_READONLY: u32 = 0x02;
const FILE_MAP_READ: u32 = 0x04;
const LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR: u32 = 0x100;
const LOAD_LIBRARY_SEARCH_DEFAULT_DIRS: u32 = 0x1000;
const CSIDL_APPDATA: i32 = 0x1a;
const CSIDL_COMMON_APPDATA: i32 = 0x23;
const ES_AWAYMODE_REQUIRED: u32 = 0x40;
const ES_DISPLAY_REQUIRED: u32 = 0x2;
const ES_SYSTEM_REQUIRED: u32 = 0x1;
const ES_CONTINUOUS: u32 = 0x8000_0000;
const RPC_S_OK: i32 = 0;
const RPC_S_UUID_LOCAL_ONLY: i32 = 1824;
const RELATION_PROCESSOR_CORE: i32 = 0;
const IMAGE_FILE_MACHINE_ARM64: u16 = 0xAA64;
const KEY_READ: u32 = 0x20019;
const HKEY_LOCAL_MACHINE: isize = 0x8000_0002u32 as i32 as isize;
const RRF_RT_REG_SZ: u32 = 2;
const FORMAT_MESSAGE_ALLOCATE_BUFFER: u32 = 0x100;
const FORMAT_MESSAGE_IGNORE_INSERTS: u32 = 0x200;
const FORMAT_MESSAGE_FROM_SYSTEM: u32 = 0x1000;

pub const MKDIR_SUCCESS: i32 = 0;
pub const MKDIR_EXISTS: i32 = 1;
pub const MKDIR_ERROR: i32 = -1;

type Handle = *mut c_void;
type Hkey = *mut c_void;

#[repr(C)]
struct FileTime {
    lo: u32,
    hi: u32,
}

impl FileTime {
    fn ticks(self) -> u64 {
        (u64::from(self.hi) << 32) | u64::from(self.lo)
    }
}

#[repr(C)]
struct FindDataW {
    attributes: u32,
    creation: FileTime,
    last_access: FileTime,
    last_write: FileTime,
    size_high: u32,
    size_low: u32,
    reserved0: u32,
    reserved1: u32,
    file_name: [u16; MAX_PATH],
    alternate: [u16; 14],
}

const _: () = assert!(size_of::<FindDataW>() == 592);

#[repr(C)]
struct MemoryStatusEx {
    length: u32,
    memory_load: u32,
    total_phys: u64,
    avail_phys: u64,
    total_page: u64,
    avail_page: u64,
    total_virtual: u64,
    avail_virtual: u64,
    avail_extended: u64,
}

const _: () = assert!(size_of::<MemoryStatusEx>() == 64);

#[repr(C)]
struct ProcessMemoryCounters {
    cb: u32,
    page_faults: u32,
    peak_working_set: usize,
    working_set: usize,
    quota_peak_paged: usize,
    quota_paged: usize,
    quota_peak_nonpaged: usize,
    quota_nonpaged: usize,
    pagefile: usize,
    peak_pagefile: usize,
}

#[repr(C)]
struct Uuid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

#[repr(C)]
struct LogicalProcessor {
    mask: usize,
    relationship: i32,
    _pad: u32,
    _rest: [u64; 2],
}

const _: () = assert!(size_of::<LogicalProcessor>() == 32);

#[link(name = "kernel32")]
#[link(name = "shell32")]
#[link(name = "ole32")]
#[link(name = "advapi32")]
#[link(name = "rpcrt4")]
#[link(name = "winmm")]
unsafe extern "system" {
    fn GetLastError() -> u32;
    fn FindFirstFileW(name: *const u16, data: *mut FindDataW) -> Handle;
    fn FindNextFileW(find: Handle, data: *mut FindDataW) -> i32;
    fn FindClose(find: Handle) -> i32;
    fn DeleteFileW(name: *const u16) -> i32;
    fn RemoveDirectoryW(name: *const u16) -> i32;
    fn CreateDirectoryW(name: *const u16, security: *mut c_void) -> i32;
    fn MoveFileExW(from: *const u16, to: *const u16, flags: u32) -> i32;
    fn ReplaceFileW(
        replaced: *const u16,
        replacement: *const u16,
        backup: *const u16,
        flags: u32,
        exclude: *mut c_void,
        reserved: *mut c_void,
    ) -> i32;
    fn CopyFileW(from: *const u16, to: *const u16, fail_if_exists: i32) -> i32;
    fn GetCurrentDirectoryW(len: u32, buf: *mut u16) -> u32;
    fn SetCurrentDirectoryW(path: *const u16) -> i32;
    fn GetDiskFreeSpaceExW(
        dir: *const u16,
        free: *mut u64,
        total: *mut u64,
        total_free: *mut u64,
    ) -> i32;
    fn GetFullPathNameW(file: *const u16, len: u32, buf: *mut u16, file_part: *mut *mut u16)
    -> u32;
    fn LoadLibraryExW(name: *const u16, file: Handle, flags: u32) -> Handle;
    fn LoadLibraryW(name: *const u16) -> Handle;
    fn GetModuleHandleW(name: *const u16) -> Handle;
    fn GetProcAddress(module: Handle, name: *const c_char) -> *const c_void;
    fn FreeLibrary(module: Handle) -> i32;
    fn CreateFileW(
        name: *const u16,
        access: u32,
        share: u32,
        security: *mut c_void,
        disposition: u32,
        flags: u32,
        template: Handle,
    ) -> Handle;
    fn CreateFileMappingW(
        file: Handle,
        security: *mut c_void,
        protect: u32,
        max_high: u32,
        max_low: u32,
        name: *const u16,
    ) -> Handle;
    fn MapViewOfFile(
        mapping: Handle,
        access: u32,
        off_high: u32,
        off_low: u32,
        bytes: usize,
    ) -> *mut c_void;
    fn UnmapViewOfFile(base: *const c_void) -> i32;
    fn CloseHandle(handle: Handle) -> i32;
    fn QueryPerformanceFrequency(freq: *mut i64) -> i32;
    fn QueryPerformanceCounter(count: *mut i64) -> i32;
    fn Sleep(ms: u32);
    fn GetSystemTimeAsFileTime(out: *mut FileTime);
    fn GetProcessTimes(
        process: Handle,
        create: *mut FileTime,
        exit: *mut FileTime,
        kernel: *mut FileTime,
        user: *mut FileTime,
    ) -> i32;
    fn GetSystemInfo(info: *mut SystemInfo);
    fn GetCurrentProcess() -> Handle;
    fn SHGetFolderPathW(
        owner: Handle,
        folder: i32,
        token: Handle,
        flags: u32,
        path: *mut u16,
    ) -> i32;
    fn GetModuleFileNameW(module: Handle, buf: *mut u16, size: u32) -> u32;
    fn SetThreadExecutionState(flags: u32) -> u32;
    fn GlobalMemoryStatusEx(status: *mut MemoryStatusEx) -> i32;
    // <psapi.h> maps GetProcessMemoryInfo to the kernel32 export
    // K32GetProcessMemoryInfo (PSAPI_VERSION 2, Windows 7+), so the C build
    // never links psapi.lib. A staticlib's #[link] does not reach obs.dll's
    // CMake link line, so name the kernel32 symbol here (LNK2019 on #707).
    #[link_name = "K32GetProcessMemoryInfo"]
    fn GetProcessMemoryInfo(process: Handle, counters: *mut ProcessMemoryCounters, cb: u32) -> i32;
    fn GetLogicalProcessorInformation(buf: *mut LogicalProcessor, len: *mut u32) -> i32;
    fn UuidCreate(uuid: *mut Uuid) -> i32;
    fn timeBeginPeriod(period: u32) -> u32;
    fn timeEndPeriod(period: u32) -> u32;
    fn IsWow64Process(process: Handle, wow64: *mut i32) -> i32;
    fn IsWow64Process2(process: Handle, process_machine: *mut u16, native_machine: *mut u16)
    -> i32;
    fn RegOpenKeyExW(key: Hkey, sub: *const u16, options: u32, sam: u32, out: *mut Hkey) -> i32;
    fn RegQueryValueExW(
        key: Hkey,
        name: *const u16,
        reserved: *mut u32,
        kind: *mut u32,
        data: *mut u8,
        len: *mut u32,
    ) -> i32;
    fn RegCloseKey(key: Hkey) -> i32;
    fn RegOpenKeyW(key: Hkey, sub: *const u16, out: *mut Hkey) -> i32;
    fn RegGetValueW(
        key: Hkey,
        sub: *const u16,
        name: *const u16,
        flags: u32,
        kind: *mut u32,
        data: *mut u8,
        len: *mut u32,
    ) -> i32;
    fn FormatMessageA(
        flags: u32,
        source: *const c_void,
        code: u32,
        lang: u32,
        buf: *mut c_char,
        size: u32,
        args: *mut c_void,
    ) -> u32;
    fn LocalFree(mem: *mut c_void) -> *mut c_void;
}

unsafe extern "C" {
    fn _wfullpath(abs: *mut u16, rel: *const u16, max: usize) -> *mut u16;
    fn malloc(size: usize) -> *mut c_void;
    fn free(ptr: *mut c_void);
    fn blog(level: c_int, fmt: *const c_char, ...);
    fn bcrash(fmt: *const c_char, ...) -> !;
    fn obs_util_image_is_obs_plugin(base: *const u8) -> i32;
}

#[repr(C)]
struct SystemInfo {
    oem_id: u32,
    page_size: u32,
    min_app: *mut c_void,
    max_app: *mut c_void,
    mask: usize,
    processor_count: u32,
    processor_type: u32,
    allocation_granularity: u32,
    processor_level: u16,
    processor_revision: u16,
}

fn invalid(handle: Handle) -> bool {
    handle.is_null() || handle as isize == INVALID_HANDLE
}

fn wide_len(buf: &[u16]) -> usize {
    buf.iter().position(|c| *c == 0).unwrap_or(buf.len())
}

/// `os_utf8_to_wcs_ptr` length: 0 means the pointer form failed the caller's
/// `> 0` / `!` check. Empty input does that.
fn to_wide(bytes: &[u8]) -> Option<Vec<u16>> {
    let n = utf8::utf8_to_wchar(bytes, None);
    if n == 0 {
        return None;
    }
    let mut buf = vec![0u16; n + 1];
    let n = utf8::utf8_to_wchar(bytes, Some(&mut buf));
    if n < buf.len() {
        buf[n] = 0;
    }
    buf.truncate(n + 1);
    Some(buf)
}

/// `os_utf8_to_wcs` into a fixed buffer. Returns the wide length, or 0.
fn utf8_into_wide(bytes: &[u8], dst: &mut [u16]) -> usize {
    if dst.is_empty() {
        return 0;
    }
    let n = utf8::utf8_to_wchar(bytes, Some(dst));
    if n < dst.len() {
        dst[n] = 0;
    }
    n
}

/// `os_wcs_to_utf8` into `dst` (`dst_size` includes the NUL). Returns the
/// byte length, or 0, and writes the NUL.
fn wcs_to_utf8(wide: &[u16], dst: &mut [u8]) -> usize {
    if dst.is_empty() {
        return 0;
    }
    let room = dst.len() - 1;
    if room == 0 {
        dst[0] = 0;
        return 0;
    }
    let n = utf8::wchar_to_utf8(wide, Some(&mut dst[..room]));
    dst[n] = 0;
    n
}

fn wcs_to_utf8_vec(wide: &[u16]) -> Vec<u8> {
    let n = utf8::wchar_to_utf8(wide, None);
    if n == 0 {
        return Vec::new();
    }
    let mut buf = vec![0u8; n + 1];
    let n = wcs_to_utf8(wide, &mut buf);
    buf.truncate(n);
    buf
}

fn wide_literal(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(core::iter::once(0)).collect()
}

fn is_dir(attributes: u32) -> bool {
    attributes & FILE_ATTRIBUTE_DIRECTORY != 0
}

/// `os_file_exists`.
pub fn file_exists(path: &[u8]) -> bool {
    let Some(wide) = to_wide(path) else {
        return false;
    };
    let mut data = unsafe { core::mem::zeroed::<FindDataW>() };
    // SAFETY: `wide` is NUL-terminated. `data` is a full `WIN32_FIND_DATAW`.
    let found = unsafe { FindFirstFileW(wide.as_ptr(), &mut data) };
    if !invalid(found) {
        // SAFETY: `found` is a find handle.
        unsafe { FindClose(found) };
    }
    !invalid(found)
}

/// `os_unlink`.
pub fn unlink(path: &[u8]) -> i32 {
    let Some(wide) = to_wide(path) else {
        return -1;
    };
    // SAFETY: `wide` is NUL-terminated.
    let ok = unsafe { DeleteFileW(wide.as_ptr()) };
    if ok != 0 { 0 } else { -1 }
}

/// `os_rmdir`.
pub fn rmdir(path: &[u8]) -> i32 {
    let Some(wide) = to_wide(path) else {
        return -1;
    };
    // SAFETY: `wide` is NUL-terminated.
    let ok = unsafe { RemoveDirectoryW(wide.as_ptr()) };
    if ok != 0 { 0 } else { -1 }
}

/// `os_mkdir`.
pub fn mkdir(path: &[u8]) -> i32 {
    let Some(wide) = to_wide(path) else {
        return MKDIR_ERROR;
    };
    // SAFETY: `wide` is NUL-terminated. A null security descriptor matches C.
    let ok = unsafe { CreateDirectoryW(wide.as_ptr(), ptr::null_mut()) };
    if ok != 0 {
        MKDIR_SUCCESS
    } else if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        MKDIR_EXISTS
    } else {
        MKDIR_ERROR
    }
}

/// `os_rename`. `MOVEFILE_REPLACE_EXISTING`, as in C.
pub fn rename(from: &[u8], to: &[u8]) -> i32 {
    let Some(from) = to_wide(from) else {
        return -1;
    };
    let Some(to) = to_wide(to) else {
        return -1;
    };
    // SAFETY: both paths are NUL-terminated.
    let ok = unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), MOVEFILE_REPLACE_EXISTING) };
    if ok != 0 { 0 } else { -1 }
}

/// `os_safe_replace`. `None` is a null backup pointer.
pub fn safe_replace(target: &[u8], from: &[u8], backup: Option<&[u8]>) -> i32 {
    let Some(target) = to_wide(target) else {
        return -1;
    };
    let Some(from) = to_wide(from) else {
        return -1;
    };
    let backup = backup.and_then(to_wide);
    let backup_ptr = backup.as_ref().map_or(ptr::null(), |b| b.as_ptr());
    // SAFETY: the wide paths are NUL-terminated. A missing backup is null.
    let replaced = unsafe {
        ReplaceFileW(
            target.as_ptr(),
            from.as_ptr(),
            backup_ptr,
            0,
            ptr::null_mut(),
            ptr::null_mut(),
        )
    };
    if replaced != 0 {
        return 0;
    }
    if unsafe { GetLastError() } == ERROR_FILE_NOT_FOUND {
        // SAFETY: both paths are NUL-terminated.
        let ok = unsafe { MoveFileExW(from.as_ptr(), target.as_ptr(), MOVEFILE_REPLACE_EXISTING) };
        if ok != 0 { 0 } else { -1 }
    } else {
        -1
    }
}

/// `os_copyfile`. Fails when the destination exists.
pub fn copy_file(from: &[u8], to: &[u8]) -> i32 {
    let Some(from) = to_wide(from) else {
        return -1;
    };
    let Some(to) = to_wide(to) else {
        return -1;
    };
    // SAFETY: both paths are NUL-terminated. `bFailIfExists` is TRUE.
    let ok = unsafe { CopyFileW(from.as_ptr(), to.as_ptr(), 1) };
    if ok != 0 { 0 } else { -1 }
}

pub struct DirEnt {
    pub name: Vec<u8>,
    pub directory: bool,
}

pub struct Dir {
    handle: Handle,
    pending: Option<FindDataW>,
}

impl Dir {
    /// `os_readdir`. The name is the UTF-8 C writes into `d_name[256]`.
    pub fn read(&mut self) -> Option<DirEnt> {
        let data = if let Some(first) = self.pending.take() {
            first
        } else {
            let mut data = unsafe { core::mem::zeroed::<FindDataW>() };
            // SAFETY: `handle` is the find handle from `FindFirstFileW`.
            let ok = unsafe { FindNextFileW(self.handle, &mut data) };
            if ok == 0 {
                return None;
            }
            data
        };
        let wide = &data.file_name[..wide_len(&data.file_name)];
        let mut capped = [0u8; 256];
        let n = wcs_to_utf8(wide, &mut capped);
        Some(DirEnt {
            name: capped[..n].to_vec(),
            directory: is_dir(data.attributes),
        })
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        if !invalid(self.handle) {
            // SAFETY: `handle` is a find handle, closed once.
            unsafe { FindClose(self.handle) };
        }
    }
}

/// `os_opendir`. The pattern is `path` plus `/*.*`.
pub fn opendir(path: &[u8]) -> Option<Dir> {
    let mut pattern = path.to_vec();
    pattern.extend_from_slice(b"/*.*");
    let wide = to_wide(&pattern)?;
    let mut data = unsafe { core::mem::zeroed::<FindDataW>() };
    // SAFETY: `wide` is NUL-terminated.
    let handle = unsafe { FindFirstFileW(wide.as_ptr(), &mut data) };
    if invalid(handle) {
        return None;
    }
    Some(Dir {
        handle,
        pending: Some(data),
    })
}

/// `os_get_abs_path` through a `MAX_PATH` buffer, as `os_get_abs_path_ptr` does.
pub fn abs_path(path: &[u8]) -> Option<Vec<u8>> {
    let mut wpath = [0u16; MAX_PATH];
    if utf8_into_wide(path, &mut wpath) == 0 {
        return None;
    }
    let mut wabs = [0u16; MAX_PATH];
    // SAFETY: both buffers hold `MAX_PATH` wide units and `wpath` is terminated.
    let rc = unsafe { _wfullpath(wabs.as_mut_ptr(), wpath.as_ptr(), MAX_PATH) };
    if rc.is_null() {
        return None;
    }
    let wide = &wabs[..wide_len(&wabs)];
    let mut out = [0u8; MAX_PATH];
    let n = wcs_to_utf8(wide, &mut out);
    if n == 0 {
        return None;
    }
    Some(out[..n].to_vec())
}

pub struct GlobEnt {
    pub path: Vec<u8>,
    pub directory: bool,
}

/// `make_globent`: keep the pattern through its last `/`, or only the name.
fn glob_path(pattern: &[u8], name: &[u8]) -> Vec<u8> {
    let mut path = pattern.to_vec();
    if let Some(slash) = path.iter().rposition(|b| *b == b'/') {
        path.truncate(slash + 1);
    } else {
        path.clear();
    }
    path.extend_from_slice(name);
    path
}

/// `os_glob`. `flags` are ignored, as in C.
pub fn glob(pattern: &[u8]) -> Option<Vec<GlobEnt>> {
    let wide = to_wide(pattern)?;
    let mut data = unsafe { core::mem::zeroed::<FindDataW>() };
    // SAFETY: `wide` is NUL-terminated.
    let handle = unsafe { FindFirstFileW(wide.as_ptr(), &mut data) };
    if invalid(handle) {
        return None;
    }
    let mut out = Vec::new();
    loop {
        let name_w = &data.file_name[..wide_len(&data.file_name)];
        let name = wcs_to_utf8_vec(name_w);
        if !name.is_empty() {
            out.push(GlobEnt {
                path: glob_path(pattern, &name),
                directory: is_dir(data.attributes),
            });
        }
        data = unsafe { core::mem::zeroed() };
        // SAFETY: `handle` is still open.
        if unsafe { FindNextFileW(handle, &mut data) } == 0 {
            break;
        }
    }
    // SAFETY: `handle` is the find handle.
    unsafe { FindClose(handle) };
    Some(out)
}

/// `os_getcwd`.
pub fn getcwd() -> Option<Vec<u8>> {
    // SAFETY: a null buffer asks for the required size, including the NUL.
    let len = unsafe { GetCurrentDirectoryW(0, ptr::null_mut()) };
    if len == 0 {
        return None;
    }
    let mut wide = vec![0u16; (len as usize) + 1];
    // SAFETY: `wide` holds `len + 1` units, which is what C allocates.
    unsafe { GetCurrentDirectoryW(len + 1, wide.as_mut_ptr()) };
    let n = wide_len(&wide);
    Some(wcs_to_utf8_vec(&wide[..n]))
}

/// `os_chdir`.
pub fn chdir(path: &[u8]) -> i32 {
    let Some(wide) = to_wide(path) else {
        return -1;
    };
    // SAFETY: `wide` is NUL-terminated.
    if unsafe { SetCurrentDirectoryW(wide.as_ptr()) } != 0 {
        0
    } else {
        -1
    }
}

fn clock_freq() -> u64 {
    static FREQ: Mutex<u64> = Mutex::new(0);
    let mut slot = FREQ.lock().unwrap_or_else(|e| e.into_inner());
    if *slot == 0 {
        let mut freq = 0i64;
        // SAFETY: `freq` is a writable `LARGE_INTEGER`.
        unsafe { QueryPerformanceFrequency(&mut freq) };
        *slot = freq as u64;
    }
    *slot
}

/// `os_gettime_ns`.
pub fn gettime_ns() -> u64 {
    let mut count = 0i64;
    // SAFETY: `count` is a writable `LARGE_INTEGER`.
    unsafe { QueryPerformanceCounter(&mut count) };
    mul_div64(count as u64, 1_000_000_000, clock_freq())
}

/// `os_sleepto_ns`.
pub fn sleepto_ns(time_target: u64) -> bool {
    let freq = clock_freq();
    let count_target = mul_div64(time_target, freq, 1_000_000_000) as i64;
    let mut count = 0i64;
    // SAFETY: `count` is a writable `LARGE_INTEGER`.
    unsafe { QueryPerformanceCounter(&mut count) };
    let stall = count < count_target;
    if stall {
        let milliseconds = ((count_target - count) as f64 * 1000.0 / freq as f64) as u32;
        if milliseconds > 1 {
            // SAFETY: `Sleep` takes a millisecond count.
            unsafe { Sleep(milliseconds - 1) };
        }
        loop {
            // SAFETY: `count` is a writable `LARGE_INTEGER`.
            unsafe { QueryPerformanceCounter(&mut count) };
            if count >= count_target {
                break;
            }
            // `YieldProcessor` is `_mm_pause` on x64 and `yield` on AArch64.
            #[cfg(target_arch = "x86_64")]
            core::arch::x86_64::_mm_pause();
            #[cfg(target_arch = "x86")]
            core::arch::x86::_mm_pause();
            #[cfg(target_arch = "aarch64")]
            // SAFETY: `yield` is the AArch64 pause hint C's `YieldProcessor` uses.
            unsafe {
                core::arch::asm!("yield");
            }
        }
    }
    stall
}

/// `os_sleepto_ns_fast`.
pub fn sleepto_ns_fast(time_target: u64) -> bool {
    let mut current = gettime_ns();
    if time_target < current {
        return false;
    }
    loop {
        let mut remain_ms = time_target.saturating_sub(current) / 1_000_000;
        if remain_ms == 0 {
            remain_ms = 1;
        }
        // SAFETY: `Sleep` takes a millisecond count.
        unsafe { Sleep(remain_ms as u32) };
        current = gettime_ns();
        if time_target <= current {
            break;
        }
    }
    true
}

fn winver() -> u32 {
    static VER: Mutex<u32> = Mutex::new(0);
    let mut slot = VER.lock().unwrap_or_else(|e| e.into_inner());
    if *slot == 0 {
        let ver = win_ver();
        *slot = ((ver.major as u32) << 8) | (ver.minor as u32);
    }
    *slot
}

/// `os_sleep_ms`. Windows 8+ (`0x0602`) shortens a sleep above 1 ms by 1.
pub fn sleep_ms(duration: u32) {
    let duration = super::super::win_sleep_duration(duration, winver());
    // SAFETY: `Sleep` takes a millisecond count.
    unsafe { Sleep(duration) };
}

fn folder_wide(folder: i32) -> Option<Vec<u16>> {
    let mut path = [0u16; MAX_PATH];
    // SAFETY: `path` holds `MAX_PATH` wide units. A null token matches C.
    let hr = unsafe {
        SHGetFolderPathW(
            ptr::null_mut(),
            folder,
            ptr::null_mut(),
            0,
            path.as_mut_ptr(),
        )
    };
    if hr != 0 && wide_len(&path) == 0 {
        return None;
    }
    Some(path[..wide_len(&path)].to_vec())
}

fn append_fit(dst: &mut Vec<u8>, cap: usize, extra: &[u8]) -> bool {
    if dst.len() + extra.len() + 1 > cap {
        return false;
    }
    dst.extend_from_slice(extra);
    true
}

fn path_into(folder: i32, name: &[u8], cap: usize) -> Option<Vec<u8>> {
    let wide = folder_wide(folder)?;
    let mut dst = vec![0u8; cap];
    let n = wcs_to_utf8(&wide, &mut dst);
    if n == 0 {
        return None;
    }
    dst.truncate(n);
    if name.is_empty() {
        return Some(dst);
    }
    if !append_fit(&mut dst, cap, b"\\") || !append_fit(&mut dst, cap, name) {
        return None;
    }
    Some(dst)
}

fn path_ptr(folder: i32, name: &[u8]) -> Vec<u8> {
    let Some(wide) = folder_wide(folder) else {
        let mut out = b"\\".to_vec();
        out.extend_from_slice(name);
        return out;
    };
    let mut out = wcs_to_utf8_vec(&wide);
    out.extend_from_slice(b"\\");
    out.extend_from_slice(name);
    out
}

/// `os_get_config_path`. `None` when the result does not fit `cap`.
pub fn config_path(name: &[u8], cap: usize) -> Option<Vec<u8>> {
    path_into(CSIDL_APPDATA, name, cap)
}

/// `os_get_config_path_ptr`.
pub fn config_path_ptr(name: &[u8]) -> Vec<u8> {
    path_ptr(CSIDL_APPDATA, name)
}

/// `os_get_program_data_path`.
pub fn program_data_path(name: &[u8], cap: usize) -> Option<Vec<u8>> {
    path_into(CSIDL_COMMON_APPDATA, name, cap)
}

/// `os_get_program_data_path_ptr`.
pub fn program_data_path_ptr(name: &[u8]) -> Vec<u8> {
    path_ptr(CSIDL_COMMON_APPDATA, name)
}

/// `os_get_executable_path_ptr`. Slashes are `/`. A name is appended.
pub fn executable_path_ptr(name: &[u8]) -> Vec<u8> {
    let mut wide = [0u16; MAX_PATH];
    // SAFETY: `wide` holds `MAX_PATH` units. A null module is this process.
    unsafe { GetModuleFileNameW(ptr::null_mut(), wide.as_mut_ptr(), MAX_PATH as u32) };
    let mut path = wcs_to_utf8_vec(&wide[..wide_len(&wide)]);
    for byte in &mut path {
        if *byte == b'\\' {
            *byte = b'/';
        }
    }
    if let Some(slash) = path.iter().rposition(|b| *b == b'/') {
        path.truncate(slash + 1);
    }
    path.extend_from_slice(name);
    path
}

/// `os_get_free_space`. `-1` on failure.
pub fn free_space(path: &[u8]) -> i64 {
    let Some(abs) = abs_path(path) else {
        return -1;
    };
    let mut wabs = [0u16; 512];
    if utf8_into_wide(&abs, &mut wabs) == 0 {
        return -1;
    }
    let mut free = 0u64;
    // SAFETY: `wabs` is terminated. `free` receives the available-byte count.
    let ok =
        unsafe { GetDiskFreeSpaceExW(wabs.as_ptr(), &mut free, ptr::null_mut(), ptr::null_mut()) };
    if ok != 0 { free as i64 } else { -1 }
}

/// `os_get_free_disk_space`. `0` on failure.
pub fn free_disk_space(dir: &[u8]) -> u64 {
    let Some(wide) = to_wide(dir) else {
        return 0;
    };
    let mut free = 0u64;
    // SAFETY: `wide` is terminated.
    let ok =
        unsafe { GetDiskFreeSpaceExW(wide.as_ptr(), &mut free, ptr::null_mut(), ptr::null_mut()) };
    if ok != 0 { free } else { 0 }
}

fn dll_name(path: &[u8]) -> Vec<u8> {
    let mut name = path.to_vec();
    for byte in &mut name {
        if *byte == b'\\' {
            *byte = b'/';
        }
    }
    if !name.windows(4).any(|w| w == b".dll") {
        name.extend_from_slice(b".dll");
    }
    name
}

fn log_load_library(path: &[u8]) {
    // C frees the wide path before `GetLastError`. Match that order.
    let error = unsafe { GetLastError() };
    if error == ERROR_PROC_NOT_FOUND {
        return;
    }
    let mut message: *mut c_char = ptr::null_mut();
    let lang = (1u32 << 10) | 9; // SUBLANG_ENGLISH_US, LANG_ENGLISH
    // SAFETY: `ALLOCATE_BUFFER` writes the pointer through `message`.
    unsafe {
        FormatMessageA(
            FORMAT_MESSAGE_FROM_SYSTEM
                | FORMAT_MESSAGE_IGNORE_INSERTS
                | FORMAT_MESSAGE_ALLOCATE_BUFFER,
            ptr::null(),
            error,
            lang,
            (&mut message as *mut *mut c_char).cast(),
            0,
            ptr::null_mut(),
        );
    }
    let owned = std::ffi::CString::new(path).ok();
    let shown = owned.as_ref().map_or(c"<path>", |s| s.as_c_str());
    // SAFETY: the format string is a literal. `shown` and `message` are C strings
    // or null. `blog` is the C variadic from `base-variadic.c`.
    unsafe {
        blog(
            LOG_INFO,
            c"LoadLibrary failed for '%s': %s (%lu)".as_ptr(),
            shown.as_ptr(),
            if message.is_null() {
                c"".as_ptr()
            } else {
                message
            },
            error,
        );
    }
    if !message.is_null() {
        // SAFETY: `FormatMessageA` allocated `message`.
        unsafe { LocalFree(message.cast()) };
    }
}

/// `os_dlopen`. A path with a `/` is loaded from its own directory.
pub fn dlopen(path: &[u8]) -> Option<*mut c_void> {
    let name = dll_name(path);
    let wide = to_wide(&name)?;
    let slash = wide.iter().rposition(|c| *c == u16::from(b'/'));
    let module = if slash.is_some() {
        let mut full = [0u16; MAX_PATH];
        // SAFETY: `wide` is terminated and `full` holds `MAX_PATH` units.
        let n = unsafe {
            GetFullPathNameW(
                wide.as_ptr(),
                MAX_PATH as u32,
                full.as_mut_ptr(),
                ptr::null_mut(),
            )
        };
        if n == 0 || n as usize >= MAX_PATH {
            ptr::null_mut()
        } else {
            // SAFETY: `full` is a full path. The flags match C.
            unsafe {
                LoadLibraryExW(
                    full.as_ptr(),
                    ptr::null_mut(),
                    LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS,
                )
            }
        }
    } else {
        // SAFETY: `wide` is a bare file name. The flag matches C.
        unsafe {
            LoadLibraryExW(
                wide.as_ptr(),
                ptr::null_mut(),
                LOAD_LIBRARY_SEARCH_DEFAULT_DIRS,
            )
        }
    };
    // Drop `wide` before reading the error, after the call, matching `bfree` then `GetLastError`.
    if module.is_null() {
        log_load_library(path);
        None
    } else {
        Some(module)
    }
}

/// `os_dlsym`. `module` is a `LoadLibrary` handle or null.
///
/// # Safety
/// `module` is not dereferenced. Null is a valid handle for `GetProcAddress`.
pub unsafe fn dlsym(module: *mut c_void, func: &[u8]) -> *mut c_void {
    let Ok(name) = std::ffi::CString::new(func) else {
        return ptr::null_mut();
    };
    // SAFETY: `module` is a `LoadLibrary` handle or null. `name` is terminated.
    unsafe { GetProcAddress(module, name.as_ptr()).cast_mut() }
}

/// `os_dlclose`. `module` is a `LoadLibrary` handle or null.
///
/// # Safety
/// `module` is not dereferenced. Null is ignored.
pub unsafe fn dlclose(module: *mut c_void) {
    if !module.is_null() {
        // SAFETY: `module` is a `LoadLibrary` handle.
        unsafe { FreeLibrary(module) };
    }
}

/// `os_is_obs_plugin`.
pub fn is_obs_plugin(path: &[u8]) -> bool {
    let name = dll_name(path);
    let Some(wide) = to_wide(&name) else {
        return false;
    };
    // SAFETY: `wide` is a NUL-terminated path.
    let file = unsafe {
        CreateFileW(
            wide.as_ptr(),
            GENERIC_READ,
            FILE_SHARE_READ,
            ptr::null_mut(),
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            ptr::null_mut(),
        )
    };
    if invalid(file) {
        return false;
    }
    // SAFETY: `file` is an open file handle.
    let mapping =
        unsafe { CreateFileMappingW(file, ptr::null_mut(), PAGE_READONLY, 0, 0, ptr::null()) };
    if mapping.is_null() {
        unsafe { CloseHandle(file) };
        return false;
    }
    // SAFETY: `mapping` is a file mapping.
    let base = unsafe { MapViewOfFile(mapping, FILE_MAP_READ, 0, 0, 0) };
    let plugin = if base.is_null() {
        false
    } else {
        // SAFETY: `base` is a mapped image. The C helper catches a fault on MSVC.
        unsafe { obs_util_image_is_obs_plugin(base.cast()) != 0 }
    };
    if !base.is_null() {
        // SAFETY: `base` came from `MapViewOfFile`.
        unsafe { UnmapViewOfFile(base) };
    }
    // SAFETY: both handles are open.
    unsafe {
        CloseHandle(mapping);
        CloseHandle(file);
    }
    plugin
}

pub struct CpuInfo {
    last_time: u64,
    last_sys: u64,
    last_user: u64,
    cores: u32,
}

/// `os_cpu_usage_info_start`.
pub fn cpu_start() -> CpuInfo {
    let mut info = unsafe { core::mem::zeroed::<SystemInfo>() };
    let mut now = FileTime { lo: 0, hi: 0 };
    let mut kernel = FileTime { lo: 0, hi: 0 };
    let mut user = FileTime { lo: 0, hi: 0 };
    let mut dummy = FileTime { lo: 0, hi: 0 };
    // SAFETY: the out-pointers are writable structs. The process handle is current.
    unsafe {
        GetSystemInfo(&mut info);
        GetSystemTimeAsFileTime(&mut now);
        GetProcessTimes(
            GetCurrentProcess(),
            &mut dummy,
            &mut dummy,
            &mut kernel,
            &mut user,
        );
    }
    CpuInfo {
        last_time: now.ticks(),
        last_sys: kernel.ticks(),
        last_user: user.ticks(),
        cores: info.processor_count,
    }
}

/// `os_cpu_usage_info_query`.
pub fn cpu_query(info: &mut CpuInfo) -> f64 {
    let mut now = FileTime { lo: 0, hi: 0 };
    let mut kernel = FileTime { lo: 0, hi: 0 };
    let mut user = FileTime { lo: 0, hi: 0 };
    let mut dummy = FileTime { lo: 0, hi: 0 };
    // SAFETY: the out-pointers are writable. The process handle is current.
    unsafe {
        GetSystemTimeAsFileTime(&mut now);
        GetProcessTimes(
            GetCurrentProcess(),
            &mut dummy,
            &mut dummy,
            &mut kernel,
            &mut user,
        );
    }
    let sys = kernel.ticks();
    let usr = user.ticks();
    let time = now.ticks();
    let percent = crate::cpu_usage::percent(
        (sys - info.last_sys + (usr - info.last_user)) as f64,
        (time - info.last_time) as f64,
        f64::from(info.cores),
    );
    info.last_time = time;
    info.last_sys = sys;
    info.last_user = usr;
    percent
}

/// `os_request_high_performance` ignores the reason and returns null.
pub fn high_performance_is_null() {}

pub struct Inhibit {
    active: bool,
}

/// `os_inhibit_sleep_create`. The reason is ignored, as in C.
pub fn inhibit_create() -> Inhibit {
    Inhibit { active: false }
}

/// `os_inhibit_sleep_set_active`.
pub fn inhibit_set(info: &mut Inhibit, active: bool) -> bool {
    if info.active == active {
        return false;
    }
    let flags = if active {
        ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_AWAYMODE_REQUIRED | ES_DISPLAY_REQUIRED
    } else {
        ES_CONTINUOUS
    };
    // SAFETY: the flags are the constants C passes.
    unsafe { SetThreadExecutionState(flags) };
    info.active = active;
    true
}

/// `os_inhibit_sleep_destroy` clears the request first.
pub fn inhibit_destroy(mut info: Inhibit) {
    inhibit_set(&mut info, false);
}

/// `os_breakpoint`. `__debugbreak` is `int3` on x86 and `brk #0` on AArch64.
pub fn breakpoint() {
    // SAFETY: the instruction is the same trap C's `__debugbreak` raises.
    unsafe {
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        core::arch::asm!("int3");
        #[cfg(target_arch = "aarch64")]
        core::arch::asm!("brk #0");
    }
}

/// `os_oom`. Debug breaks. Otherwise `RaiseException(ERROR_OUTOFMEMORY)`.
pub fn oom() {
    #[cfg(debug_assertions)]
    breakpoint();
    #[cfg(not(debug_assertions))]
    {
        // SAFETY: `RaiseException` is `ERROR_OUTOFMEMORY`, non-continuable, as in C.
        unsafe extern "system" {
            fn RaiseException(code: u32, flags: u32, args: u32, arguments: *mut usize) -> !;
        }
        unsafe { RaiseException(14, 1, 0, ptr::null_mut()) };
    }
}

fn bit_count(mask: usize) -> i32 {
    let mut count = 0i32;
    let mut bit = 1usize << (usize::BITS - 1);
    for _ in 0..usize::BITS {
        if mask & bit != 0 {
            count += 1;
        }
        bit /= 2;
    }
    count
}

struct CoreCounts {
    ready: bool,
    physical: i32,
    logical: i32,
}

fn cores() -> (i32, i32) {
    static CORES: Mutex<CoreCounts> = Mutex::new(CoreCounts {
        ready: false,
        physical: 0,
        logical: 0,
    });
    let mut slot = CORES.lock().unwrap_or_else(|e| e.into_inner());
    if slot.ready {
        return (slot.physical, slot.logical);
    }
    slot.ready = true;
    let mut len = 0u32;
    // SAFETY: a null buffer writes the required size.
    unsafe { GetLogicalProcessorInformation(ptr::null_mut(), &mut len) };
    if unsafe { GetLastError() } != ERROR_INSUFFICIENT_BUFFER || len == 0 {
        return (0, 0);
    }
    // SAFETY: `malloc` of the size the API just reported.
    let info = unsafe { malloc(len as usize) }.cast::<LogicalProcessor>();
    if info.is_null() {
        return (0, 0);
    }
    // SAFETY: `info` holds `len` bytes.
    let ok = unsafe { GetLogicalProcessorInformation(info, &mut len) };
    if ok != 0 {
        let num = (len as usize) / size_of::<LogicalProcessor>();
        for i in 0..num {
            // SAFETY: `i < num` and `num` is the entry count in `info`.
            let entry = unsafe { &*info.add(i) };
            if entry.relationship == RELATION_PROCESSOR_CORE {
                slot.physical += 1;
                slot.logical += bit_count(entry.mask);
            }
        }
    }
    // SAFETY: `info` came from `malloc`.
    unsafe { free(info.cast()) };
    (slot.physical, slot.logical)
}

/// `os_get_physical_cores`.
pub fn physical_cores() -> i32 {
    cores().0
}

/// `os_get_logical_cores`.
pub fn logical_cores() -> i32 {
    cores().1
}

fn memory_status() -> Option<MemoryStatusEx> {
    let mut status = MemoryStatusEx {
        length: size_of::<MemoryStatusEx>() as u32,
        memory_load: 0,
        total_phys: 0,
        avail_phys: 0,
        total_page: 0,
        avail_page: 0,
        total_virtual: 0,
        avail_virtual: 0,
        avail_extended: 0,
    };
    // SAFETY: `length` is `sizeof(MEMORYSTATUSEX)` and the rest is writable.
    let ok = unsafe { GlobalMemoryStatusEx(&mut status) };
    (ok != 0).then_some(status)
}

/// `os_get_sys_free_size`.
pub fn sys_free() -> u64 {
    memory_status().map_or(0, |s| s.avail_phys)
}

/// `os_get_sys_total_size`. Cached after the first call, as in C.
pub fn sys_total() -> u64 {
    static TOTAL: Mutex<Option<u64>> = Mutex::new(None);
    let mut slot = TOTAL.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(total) = *slot {
        return total;
    }
    let total = memory_status().map_or(0, |s| s.total_phys);
    *slot = Some(total);
    total
}

fn process_memory() -> Option<ProcessMemoryCounters> {
    let mut pmc = unsafe { core::mem::zeroed::<ProcessMemoryCounters>() };
    pmc.cb = size_of::<ProcessMemoryCounters>() as u32;
    // SAFETY: `cb` is the struct size. The process handle is current.
    let ok = unsafe { GetProcessMemoryInfo(GetCurrentProcess(), &mut pmc, pmc.cb) };
    (ok != 0).then_some(pmc)
}

/// `os_get_proc_memory_usage`: resident is the working set, virtual is the pagefile usage.
pub fn proc_memory() -> Option<(u64, u64)> {
    process_memory().map(|pmc| (pmc.working_set as u64, pmc.pagefile as u64))
}

/// `os_get_proc_resident_size`.
pub fn proc_resident() -> u64 {
    process_memory().map_or(0, |pmc| pmc.working_set as u64)
}

/// `os_get_proc_virtual_size`.
pub fn proc_virtual() -> u64 {
    process_memory().map_or(0, |pmc| pmc.pagefile as u64)
}

/// `os_generate_uuid`, without the braces, lowercase.
pub fn generate_uuid() -> String {
    let mut uuid = unsafe { core::mem::zeroed::<Uuid>() };
    // SAFETY: `uuid` is a writable `UUID`.
    let status = unsafe { UuidCreate(&mut uuid) };
    if status != RPC_S_OK && status != RPC_S_UUID_LOCAL_ONLY {
        // SAFETY: `bcrash` is the C variadic. `%l` is the format C uses.
        unsafe { bcrash(c"Failed to get UUID, RPC_STATUS: %l".as_ptr(), status) };
    }
    format!(
        "{:08x}-{:04x}-{:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        uuid.data1,
        uuid.data2,
        uuid.data3,
        uuid.data4[0],
        uuid.data4[1],
        uuid.data4[2],
        uuid.data4[3],
        uuid.data4[4],
        uuid.data4[5],
        uuid.data4[6],
        uuid.data4[7]
    )
}

/// `is_64_bit_windows`.
pub fn is_64_bit_windows() -> bool {
    if cfg!(target_pointer_width = "64") {
        return true;
    }
    let mut wow = 0i32;
    // SAFETY: the process handle is current. `wow` is writable.
    unsafe { IsWow64Process(GetCurrentProcess(), &mut wow) != 0 && wow != 0 }
}

/// `is_arm64_windows`.
pub fn is_arm64_windows() -> bool {
    if cfg!(any(target_arch = "aarch64", target_arch = "arm64ec")) {
        return true;
    }
    let mut process = 0u16;
    let mut native = 0u16;
    // SAFETY: the process handle is current. Both machine words are writable.
    let ok = unsafe { IsWow64Process2(GetCurrentProcess(), &mut process, &mut native) };
    ok != 0 && native == IMAGE_FILE_MACHINE_ARM64
}

/// `os_get_emulation_status`.
pub fn emulation_status() -> bool {
    if cfg!(any(target_arch = "aarch64", target_arch = "arm64ec")) {
        false
    } else {
        is_arm64_windows()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WinVer {
    pub major: i32,
    pub minor: i32,
    pub build: i32,
    pub revis: i32,
}

impl WinVer {
    fn higher(self, new: Self) -> Self {
        let replace = new.major > self.major
            || (new.major == self.major
                && (new.minor > self.minor
                    || (new.minor == self.minor
                        && (new.build > self.build
                            || (new.build == self.build && new.revis > self.revis)))));
        if replace { new } else { self }
    }
}

struct ReleaseId(UnsafeCell<[u8; 256]>);
// The C file publishes this buffer for the rest of the process.
unsafe impl Sync for ReleaseId {}

static RELEASE_ID: ReleaseId = ReleaseId(UnsafeCell::new(unavailable()));

const fn unavailable() -> [u8; 256] {
    let mut buf = [0u8; 256];
    let text = b"unavailable";
    let mut i = 0;
    while i < text.len() {
        buf[i] = text[i];
        i += 1;
    }
    buf
}

fn write_release_id(text: &[u8]) {
    // SAFETY: writers hold `VER_LOCK`. Readers see a C string, as they do in C.
    let buf = unsafe { &mut *RELEASE_ID.0.get() };
    buf.fill(0);
    let n = text.len().min(buf.len() - 1);
    buf[..n].copy_from_slice(&text[..n]);
}

/// `get_win_release_id`. The pointer is the process-lifetime buffer.
pub fn win_release_id() -> *const u8 {
    // Make sure a first read still fills the id when version lookup has run.
    let _ = win_ver();
    RELEASE_ID.0.get().cast()
}

type GetFileVersionInfoSizeW = unsafe extern "system" fn(*const u16, *mut u32) -> u32;
type GetFileVersionInfoW = unsafe extern "system" fn(*const u16, u32, u32, *mut c_void) -> i32;
type VerQueryValueW =
    unsafe extern "system" fn(*const c_void, *const u16, *mut *mut c_void, *mut u32) -> i32;

struct VerFns {
    size: GetFileVersionInfoSizeW,
    info: GetFileVersionInfoW,
    query: VerQueryValueW,
}

static VER_LOCK: Mutex<Option<VerFns>> = Mutex::new(None);
static VER_TRIED: Mutex<bool> = Mutex::new(false);

fn version_fns() -> Option<VerFns> {
    let mut tried = VER_TRIED.lock().unwrap_or_else(|e| e.into_inner());
    let mut slot = VER_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    if *tried {
        return slot.clone();
    }
    *tried = true;
    let version = wide_literal("version");
    // SAFETY: `version` is the wide literal C passes to `GetModuleHandleW`.
    let mut module = unsafe { GetModuleHandleW(version.as_ptr()) };
    if module.is_null() {
        // SAFETY: the same literal. C does not free this module.
        module = unsafe { LoadLibraryW(version.as_ptr()) };
        if module.is_null() {
            // SAFETY: `blog` is the C variadic. The text matches the concatenated literal.
            unsafe {
                blog(
                    LOG_ERROR,
                    c"Failed to load windows version library".as_ptr(),
                )
            };
            return None;
        }
    }
    // SAFETY: `module` is version.dll. The names are the exports C loads.
    let size = unsafe { GetProcAddress(module, c"GetFileVersionInfoSizeW".as_ptr()) };
    let info = unsafe { GetProcAddress(module, c"GetFileVersionInfoW".as_ptr()) };
    let query = unsafe { GetProcAddress(module, c"VerQueryValueW".as_ptr()) };
    if size.is_null() || info.is_null() || query.is_null() {
        unsafe {
            blog(
                LOG_ERROR,
                c"Failed to load windows version functions".as_ptr(),
            )
        };
        return None;
    }
    let fns = VerFns {
        // SAFETY: the proc addresses are those three version.dll exports.
        size: unsafe { core::mem::transmute::<*const c_void, GetFileVersionInfoSizeW>(size) },
        info: unsafe { core::mem::transmute::<*const c_void, GetFileVersionInfoW>(info) },
        query: unsafe { core::mem::transmute::<*const c_void, VerQueryValueW>(query) },
    };
    *slot = Some(fns.clone());
    Some(fns)
}

impl Clone for VerFns {
    fn clone(&self) -> Self {
        Self {
            size: self.size,
            info: self.info,
            query: self.query,
        }
    }
}

/// `get_dll_ver`.
pub fn dll_ver(lib: &[u16]) -> Option<WinVer> {
    let fns = version_fns()?;
    let mut utf8 = [0u8; 512];
    let n = wcs_to_utf8(&lib[..wide_len(lib)], &mut utf8);
    let shown =
        std::ffi::CString::new(&utf8[..n]).unwrap_or_else(|_| std::ffi::CString::new("").unwrap());
    // SAFETY: `lib` is the wide path the caller passed. `fns.size` is the loaded export.
    let size = unsafe { (fns.size)(lib.as_ptr(), ptr::null_mut()) };
    if size == 0 {
        unsafe {
            blog(
                LOG_ERROR,
                c"Failed to get %s version info size".as_ptr(),
                shown.as_ptr(),
            )
        };
        return None;
    }
    let mut data = vec![0u8; size as usize];
    // SAFETY: `data` holds `size` bytes. `fns.info` is the loaded export.
    let ok = unsafe { (fns.info)(lib.as_ptr(), 0, size, data.as_mut_ptr().cast()) };
    if ok == 0 {
        unsafe {
            blog(
                LOG_ERROR,
                c"Failed to get %s version info".as_ptr(),
                shown.as_ptr(),
            )
        };
        return None;
    }
    let mut info = ptr::null_mut();
    let mut len = 0u32;
    let root = wide_literal("\\");
    // SAFETY: `data` is the version block. `root` is `L"\\"`.
    let queried = unsafe { (fns.query)(data.as_ptr().cast(), root.as_ptr(), &mut info, &mut len) };
    if queried == 0 || info.is_null() || len == 0 {
        unsafe {
            blog(
                LOG_ERROR,
                c"Failed to get %s version info value".as_ptr(),
                shown.as_ptr(),
            )
        };
        return None;
    }
    // `dwFileVersionMS` is at byte 8, `dwFileVersionLS` at byte 12.
    if (len as usize) < 16 {
        return None;
    }
    // SAFETY: `info` points at a `VS_FIXEDFILEINFO` of at least 16 bytes.
    let ms = unsafe { info.cast::<u32>().add(2).read_unaligned() };
    let ls = unsafe { info.cast::<u32>().add(3).read_unaligned() };
    Some(WinVer {
        major: (ms >> 16) as i32,
        minor: (ms & 0xffff) as i32,
        build: (ls >> 16) as i32,
        revis: (ls & 0xffff) as i32,
    })
}

fn reg_sz(key: Hkey, name: &[u16], buf: &mut [u16]) -> bool {
    let mut size = (buf.len() * 2) as u32;
    // SAFETY: `key` is open. `name` is terminated. `buf` holds `size` bytes.
    let status = unsafe {
        RegGetValueW(
            key,
            ptr::null(),
            name.as_ptr(),
            RRF_RT_REG_SZ,
            ptr::null_mut(),
            buf.as_mut_ptr().cast(),
            &mut size,
        )
    };
    status == ERROR_SUCCESS
}

fn wcstol(text: &[u16]) -> i32 {
    let mut i = 0;
    while i < text.len() && matches!(text[i], 0x20 | 0x09) {
        i += 1;
    }
    let neg = if i < text.len() && text[i] == u16::from(b'-') {
        i += 1;
        true
    } else {
        false
    };
    let mut n: i32 = 0;
    while i < text.len() && (u16::from(b'0')..=u16::from(b'9')).contains(&text[i]) {
        n = n
            .saturating_mul(10)
            .saturating_add(i32::from(text[i] - u16::from(b'0')));
        i += 1;
    }
    if neg { n.saturating_neg() } else { n }
}

fn reg_ver() -> WinVer {
    let mut ver = WinVer {
        major: 0,
        minor: 0,
        build: 0,
        revis: 0,
    };
    let sub = wide_literal("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion");
    let mut key: Hkey = ptr::null_mut();
    // SAFETY: `sub` is the wide key C opens under `HKEY_LOCAL_MACHINE`.
    let status = unsafe { RegOpenKeyW(HKEY_LOCAL_MACHINE as Hkey, sub.as_ptr(), &mut key) };
    if status != ERROR_SUCCESS {
        return ver;
    }
    let mut dw = 0u32;
    let mut size = 4u32;
    let major = wide_literal("CurrentMajorVersionNumber");
    // SAFETY: `key` is open and `dw` is a writable DWORD.
    if unsafe {
        RegQueryValueExW(
            key,
            major.as_ptr(),
            ptr::null_mut(),
            ptr::null_mut(),
            (&mut dw as *mut u32).cast(),
            &mut size,
        )
    } == ERROR_SUCCESS
    {
        ver.major = dw as i32;
    }
    size = 4;
    let minor = wide_literal("CurrentMinorVersionNumber");
    if unsafe {
        RegQueryValueExW(
            key,
            minor.as_ptr(),
            ptr::null_mut(),
            ptr::null_mut(),
            (&mut dw as *mut u32).cast(),
            &mut size,
        )
    } == ERROR_SUCCESS
    {
        ver.minor = dw as i32;
    }
    size = 4;
    let ubr = wide_literal("UBR");
    if unsafe {
        RegQueryValueExW(
            key,
            ubr.as_ptr(),
            ptr::null_mut(),
            ptr::null_mut(),
            (&mut dw as *mut u32).cast(),
            &mut size,
        )
    } == ERROR_SUCCESS
    {
        ver.revis = dw as i32;
    }
    let mut text = [0u16; 256];
    let build = wide_literal("CurrentBuildNumber");
    if reg_sz(key, &build, &mut text) {
        ver.build = wcstol(&text[..wide_len(&text)]);
    }
    let release_name = if ver.build > 19041 {
        wide_literal("DisplayVersion")
    } else {
        wide_literal("ReleaseId")
    };
    text.fill(0);
    if reg_sz(key, &release_name, &mut text) {
        let mut utf8 = [0u8; 256];
        let n = wcs_to_utf8(&text[..wide_len(&text)], &mut utf8);
        write_release_id(&utf8[..n]);
    }
    // SAFETY: `key` is the key just opened.
    unsafe { RegCloseKey(key) };
    ver
}

fn rtl_ver() -> WinVer {
    let mut ver = WinVer {
        major: 0,
        minor: 0,
        build: 0,
        revis: 0,
    };
    let ntdll = wide_literal("ntdll");
    // SAFETY: `ntdll` is the wide literal C passes.
    let module = unsafe { GetModuleHandleW(ntdll.as_ptr()) };
    if module.is_null() {
        return ver;
    }
    // SAFETY: `module` is ntdll.
    let get_ver = unsafe { GetProcAddress(module, c"RtlGetVersion".as_ptr()) };
    if get_ver.is_null() {
        return ver;
    }
    #[repr(C)]
    struct OsVersion {
        size: u32,
        major: u32,
        minor: u32,
        build: u32,
        platform: u32,
        csd: [u16; 128],
        sp_major: u16,
        sp_minor: u16,
        suite: u16,
        product: u8,
        _reserved: u8,
    }
    let mut os = unsafe { core::mem::zeroed::<OsVersion>() };
    os.size = size_of::<OsVersion>() as u32;
    type RtlGetVersion = unsafe extern "system" fn(*mut OsVersion) -> i32;
    // SAFETY: `get_ver` is `RtlGetVersion`. `os.size` is the struct size.
    let status = unsafe { core::mem::transmute::<*const c_void, RtlGetVersion>(get_ver)(&mut os) };
    if status < 0 {
        return ver;
    }
    ver.major = os.major as i32;
    ver.minor = os.minor as i32;
    ver.build = os.build as i32;
    ver
}

static WIN_VER: Mutex<Option<WinVer>> = Mutex::new(None);

/// `get_win_ver`.
pub fn win_ver() -> WinVer {
    let mut slot = WIN_VER.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(ver) = *slot {
        return ver;
    }
    let reg = reg_ver();
    let rtl = rtl_ver();
    let nto = dll_ver(&wide_literal("ntoskrnl.exe")).unwrap_or(WinVer {
        major: 0,
        minor: 0,
        build: 0,
        revis: 0,
    });
    let ver = reg.higher(rtl).higher(nto);
    *slot = Some(ver);
    ver
}

/// `get_win_ver_int`.
pub fn win_ver_int() -> u32 {
    winver()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RegDword {
    pub status: i32,
    pub size: u32,
    pub value: u32,
}

/// `get_reg_dword`.
pub fn reg_dword(key: isize, sub_key: &[u16], value: &[u16]) -> RegDword {
    let mut opened: Hkey = ptr::null_mut();
    // SAFETY: `sub_key` is terminated. `key` is an `HKEY` the caller passed.
    let status = unsafe { RegOpenKeyExW(key as Hkey, sub_key.as_ptr(), 0, KEY_READ, &mut opened) };
    if status != ERROR_SUCCESS {
        return RegDword {
            status,
            size: 0,
            value: 0,
        };
    }
    let mut info = RegDword {
        status: 0,
        size: 4,
        value: 0,
    };
    // SAFETY: `opened` is the key just opened. `value` is a writable DWORD.
    info.status = unsafe {
        RegQueryValueExW(
            opened,
            value.as_ptr(),
            ptr::null_mut(),
            ptr::null_mut(),
            (&mut info.value as *mut u32).cast(),
            &mut info.size,
        )
    };
    // SAFETY: `opened` is the key just opened.
    unsafe { RegCloseKey(opened) };
    info
}

/// `DllMain`: `timeBeginPeriod(1)` on attach, `timeEndPeriod(1)` on detach.
pub fn dllmain(reason: u32) {
    if reason == 1 {
        // SAFETY: `timeBeginPeriod(1)` is the call C makes on `DLL_PROCESS_ATTACH`.
        unsafe { timeBeginPeriod(1) };
    } else if reason == 0 {
        // SAFETY: `timeEndPeriod(1)` is the call C makes on `DLL_PROCESS_DETACH`.
        unsafe { timeEndPeriod(1) };
    }
}
