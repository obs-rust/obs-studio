//! macOS half of `libobs/util/platform-cocoa.m`.
//!
//! `unsafe` is the Mach, sysctl, CoreFoundation, IOKit, and Objective-C
//! calls. The safe signatures take byte strings and do not dereference null.
//!
//! Intentional differences from the C file, also listed in the parity test:
//! - `os_cpu_usage_info` and `os_inhibit_t` are `Box` values.
//! - Core-count and total-memory caches take a mutex.
//! - A null `os_get_proc_memory_usage` returns false. C writes through it.
//! - `cfstr_copy_dstr`'s slow path zeros the bytes past the first NUL.
//!   C leaves them uninitialized. `len` is still the encoding maximum.
//! - `cfstr_copy_cstr`'s slow path returns a buffer sized to the C string.
//!   C sizes it to the encoding maximum. The bytes up to the NUL match.
//! - A null `UTF8String` becomes an empty base path. C would `memcpy` it.
//! - A non-null inhibit reason becomes `CFStringCreateWithCString("")`.
//!   C uses `CFStringCreateCopy` of `CFSTR("")`.

#![allow(unsafe_code)]

use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::mem::{self, MaybeUninit};
use std::ptr;
use std::sync::{Mutex, OnceLock};

use crate::base::LOG_WARNING;
use crate::darray::grow_capacity;
use crate::dstr::Dstr;

const NS_APPLICATION_SUPPORT: usize = 14;
const NS_USER_DOMAIN: usize = 1;
const NS_LOCAL_DOMAIN: usize = 2;
/// `NSActivityUserInitiated`. Bit 20 is already inside `0x00FFFFFF`.
const NS_ACTIVITY_USER_INITIATED: u64 = 0x00FF_FFFF;
const CF_UTF8: u32 = 0x0800_0100;
const K_CF_NOT_FOUND: isize = -1;
const IOPM_ASSERTION_LEVEL_ON: u32 = 255;
const IOPM_USER_ACTIVE_LOCAL: i32 = 0;
const IO_RETURN_SUCCESS: i32 = 0;

#[cfg(target_arch = "aarch64")]
const TASK_BASIC_INFO_64: u32 = 18;
#[cfg(not(target_arch = "aarch64"))]
const TASK_BASIC_INFO_64: u32 = 5;

/// `struct task_basic_info_64` / `task_basic_info_64_2`. Same fields.
/// The SDK declares both under `#pragma pack(4)`, so `virtual_size` sits
/// at offset 4. arm64 passes flavor 18; x86_64 passes flavor 5.
/// Measured on macOS 27 (arm64): size 40, align 4, `user_time` at 20,
/// `system_time` at 28, `policy` at 36. `TASK_BASIC_INFO_64_COUNT` is 10.
#[repr(C, packed(4))]
struct TaskBasicInfo64 {
    suspend_count: i32,
    virtual_size: u64,
    resident_size: u64,
    user_time: libc::time_value_t,
    system_time: libc::time_value_t,
    policy: i32,
}

#[link(name = "objc")]
unsafe extern "C" {
    fn objc_getClass(name: *const c_char) -> *mut c_void;
    fn sel_registerName(name: *const c_char) -> *mut c_void;
    fn objc_msgSend();
    fn objc_retain(obj: *mut c_void) -> *mut c_void;
    fn objc_release(obj: *mut c_void);
    fn objc_autoreleasePoolPush() -> *mut c_void;
    fn objc_autoreleasePoolPop(pool: *mut c_void);
}

#[link(name = "Foundation", kind = "framework")]
unsafe extern "C" {
    fn NSSearchPathForDirectoriesInDomains(
        directory: usize,
        domain_mask: usize,
        expand_tilde: i8,
    ) -> *mut c_void;
    static NSURLVolumeAvailableCapacityKey: *mut c_void;
    static NSURLVolumeAvailableCapacityForImportantUsageKey: *mut c_void;
    static NSURLVolumeAvailableCapacityForOpportunisticUsageKey: *mut c_void;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFStringCreateWithCString(
        alloc: *const c_void,
        c_str: *const c_char,
        encoding: u32,
    ) -> *const c_void;
    fn CFStringGetCStringPtr(string: *const c_void, encoding: u32) -> *const c_char;
    fn CFStringGetLength(string: *const c_void) -> isize;
    fn CFStringGetMaximumSizeForEncoding(length: isize, encoding: u32) -> isize;
    fn CFStringGetCString(
        string: *const c_void,
        buffer: *mut c_char,
        buffer_size: isize,
        encoding: u32,
    ) -> u8;
    fn CFRelease(cf: *const c_void);
}

#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IOPMAssertionCreateWithName(
        assertion_type: *const c_void,
        assertion_level: u32,
        assertion_name: *const c_void,
        assertion_id: *mut u32,
    ) -> i32;
    fn IOPMAssertionDeclareUserActivity(
        assertion_name: *const c_void,
        user_type: i32,
        assertion_id: *mut u32,
    ) -> i32;
    fn IOPMAssertionRelease(assertion_id: u32) -> i32;
}

unsafe extern "C" {
    fn blog(level: c_int, fmt: *const c_char, ...);
    fn bcrash(format: *const c_char, ...) -> !;
}

fn sel(name: &CStr) -> *mut c_void {
    // SAFETY: `name` is a NUL-terminated selector name.
    unsafe { sel_registerName(name.as_ptr()) }
}

fn cls(name: &CStr) -> *mut c_void {
    // SAFETY: `name` is a NUL-terminated class name.
    unsafe { objc_getClass(name.as_ptr()) }
}

unsafe fn msg_id(recv: *mut c_void, sel: *mut c_void) -> *mut c_void {
    let f: unsafe extern "C" fn(*mut c_void, *mut c_void) -> *mut c_void =
        unsafe { mem::transmute(objc_msgSend as *const ()) };
    unsafe { f(recv, sel) }
}

unsafe fn msg_ptr(recv: *mut c_void, sel: *mut c_void) -> *const c_char {
    let f: unsafe extern "C" fn(*mut c_void, *mut c_void) -> *const c_char =
        unsafe { mem::transmute(objc_msgSend as *const ()) };
    unsafe { f(recv, sel) }
}

unsafe fn msg_usize(recv: *mut c_void, sel: *mut c_void) -> usize {
    let f: unsafe extern "C" fn(*mut c_void, *mut c_void) -> usize =
        unsafe { mem::transmute(objc_msgSend as *const ()) };
    unsafe { f(recv, sel) }
}

unsafe fn msg_i64(recv: *mut c_void, sel: *mut c_void) -> i64 {
    let f: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i64 =
        unsafe { mem::transmute(objc_msgSend as *const ()) };
    unsafe { f(recv, sel) }
}

unsafe fn msg_id_usize(recv: *mut c_void, sel: *mut c_void, arg: usize) -> *mut c_void {
    let f: unsafe extern "C" fn(*mut c_void, *mut c_void, usize) -> *mut c_void =
        unsafe { mem::transmute(objc_msgSend as *const ()) };
    unsafe { f(recv, sel, arg) }
}

unsafe fn msg_id_ptr(recv: *mut c_void, sel: *mut c_void, arg: *const c_char) -> *mut c_void {
    let f: unsafe extern "C" fn(*mut c_void, *mut c_void, *const c_char) -> *mut c_void =
        unsafe { mem::transmute(objc_msgSend as *const ()) };
    unsafe { f(recv, sel, arg) }
}

unsafe fn msg_id_ptr_usize(
    recv: *mut c_void,
    sel: *mut c_void,
    items: *const *mut c_void,
    count: usize,
) -> *mut c_void {
    let f: unsafe extern "C" fn(
        *mut c_void,
        *mut c_void,
        *const *mut c_void,
        usize,
    ) -> *mut c_void = unsafe { mem::transmute(objc_msgSend as *const ()) };
    unsafe { f(recv, sel, items, count) }
}

unsafe fn msg_id_id(recv: *mut c_void, sel: *mut c_void, arg: *mut c_void) -> *mut c_void {
    let f: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> *mut c_void =
        unsafe { mem::transmute(objc_msgSend as *const ()) };
    unsafe { f(recv, sel, arg) }
}

unsafe fn msg_id_id_ptr(
    recv: *mut c_void,
    sel: *mut c_void,
    arg: *mut c_void,
    err: *mut *mut c_void,
) -> *mut c_void {
    let f: unsafe extern "C" fn(
        *mut c_void,
        *mut c_void,
        *mut c_void,
        *mut *mut c_void,
    ) -> *mut c_void = unsafe { mem::transmute(objc_msgSend as *const ()) };
    unsafe { f(recv, sel, arg, err) }
}

unsafe fn msg_id_u64_id(
    recv: *mut c_void,
    sel: *mut c_void,
    options: u64,
    reason: *mut c_void,
) -> *mut c_void {
    let f: unsafe extern "C" fn(*mut c_void, *mut c_void, u64, *mut c_void) -> *mut c_void =
        unsafe { mem::transmute(objc_msgSend as *const ()) };
    unsafe { f(recv, sel, options, reason) }
}

unsafe fn msg_void_id(recv: *mut c_void, sel: *mut c_void, arg: *mut c_void) {
    let f: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) =
        unsafe { mem::transmute(objc_msgSend as *const ()) };
    unsafe { f(recv, sel, arg) };
}

struct Pool(*mut c_void);

impl Pool {
    fn new() -> Self {
        // SAFETY: pushes a pool this value pops in `Drop`.
        Self(unsafe { objc_autoreleasePoolPush() })
    }
}

impl Drop for Pool {
    fn drop(&mut self) {
        // SAFETY: `self.0` came from `objc_autoreleasePoolPush`.
        unsafe { objc_autoreleasePoolPop(self.0) };
    }
}

/// Bytes of `name` up to the first NUL. Null, empty, and a leading NUL
/// are `None`, which is what `dstr_cat` ignores.
fn c_bytes(name: Option<&[u8]>) -> Option<&[u8]> {
    let name = name?;
    if name.is_empty() || name[0] == 0 {
        return None;
    }
    let end = name.iter().position(|b| *b == 0).unwrap_or(name.len());
    Some(&name[..end])
}

/// `os_get_path_internal`: null or empty `name` is `base`, otherwise
/// `base/name`.
pub fn format_path(base: &[u8], name: Option<&[u8]>) -> Vec<u8> {
    match c_bytes(name) {
        None => base.to_vec(),
        Some(name) => {
            let mut out = Vec::with_capacity(base.len() + 1 + name.len());
            out.extend_from_slice(base);
            out.push(b'/');
            out.extend_from_slice(name);
            out
        }
    }
}

/// `os_get_path_ptr_internal`: always appends `/`, then `name` when it
/// is non-empty. `dstr_cat` ignores null and empty.
pub fn format_path_ptr(base: &[u8], name: Option<&[u8]>) -> Vec<u8> {
    let mut out = Vec::with_capacity(base.len() + 1);
    out.extend_from_slice(base);
    out.push(b'/');
    if let Some(name) = c_bytes(name) {
        out.extend_from_slice(name);
    }
    out
}

/// `(double)sys_delta * 100.0 / cpu_delta / core_count`.
/// A zero `cpu_delta` returns 0, as the query does before dividing.
/// A zero `core_count` is infinity.
pub fn cpu_percent(sys_delta: i64, cpu_delta: i64, core_count: i32) -> f64 {
    if cpu_delta == 0 {
        return 0.0;
    }
    crate::cpu_usage::cap((sys_delta as f64) * 100.0 / (cpu_delta as f64) / (core_count as f64))
}

/// `os_inhibit_sleep_create` checks `if (!reason)`.
/// True selects `CFStringCreateWithCString` of a null pointer.
/// False replaces the text with an empty string.
pub fn inhibit_uses_null_cstring(reason_is_null: bool) -> bool {
    reason_is_null
}

/// `os_get_free_disk_space` casts the signed free-space value.
pub fn free_disk_cast(free: i64) -> u64 {
    free as u64
}

fn support_base(domain: usize) -> Vec<u8> {
    let _pool = Pool::new();
    // SAFETY: the directory and domain constants match the NSSearchPath enums.
    // YES is 1. The array is autoreleased into `_pool`.
    let paths = unsafe { NSSearchPathForDirectoriesInDomains(NS_APPLICATION_SUPPORT, domain, 1) };
    // SAFETY: `paths` is the array just returned, or nil.
    let count = unsafe { msg_usize(paths, sel(c"count")) };
    if count == 0 {
        // SAFETY: the format string is a literal. `bcrash` does not return.
        unsafe { bcrash(c"Could not get home directory (platform-cocoa)".as_ptr()) };
    }
    // SAFETY: `count` is non-zero, so index 0 is in range.
    let app = unsafe { msg_id_usize(paths, sel(c"objectAtIndex:"), 0) };
    // SAFETY: `app` is an NSString or nil. A nil receiver returns nil.
    let utf8 = unsafe { msg_ptr(app, sel(c"UTF8String")) };
    if utf8.is_null() {
        return Vec::new();
    }
    // SAFETY: `UTF8String` is NUL-terminated and lives until `_pool` drops.
    unsafe { CStr::from_ptr(utf8) }.to_bytes().to_vec()
}

/// `os_get_config_path`. `dst` of `None` reports the untruncated length.
pub fn config_path_into(dst: Option<&mut [u8]>, name: Option<&[u8]>) -> i32 {
    let base = support_base(NS_USER_DOMAIN);
    let joined = format_path(&base, name);
    super::nix::snprintf_bytes(dst, &joined)
}

/// `os_get_config_path_ptr`, without the trailing NUL.
pub fn config_path_ptr(name: Option<&[u8]>) -> Vec<u8> {
    let base = support_base(NS_USER_DOMAIN);
    format_path_ptr(&base, name)
}

/// `os_get_program_data_path`.
pub fn program_data_path_into(dst: Option<&mut [u8]>, name: Option<&[u8]>) -> i32 {
    let base = support_base(NS_LOCAL_DOMAIN);
    let joined = format_path(&base, name);
    super::nix::snprintf_bytes(dst, &joined)
}

/// `os_get_program_data_path_ptr`, without the trailing NUL.
pub fn program_data_path_ptr(name: Option<&[u8]>) -> Vec<u8> {
    let base = support_base(NS_LOCAL_DOMAIN);
    format_path_ptr(&base, name)
}

/// `os_get_executable_path_ptr`, without the trailing NUL.
/// `None` when `_NSGetExecutablePath` or `realpath` fails. C does not
/// retry with a larger buffer.
#[allow(deprecated)] // libc::_NSGetExecutablePath; the C file calls the same symbol
pub fn executable_path(name: Option<&[u8]>) -> Option<Vec<u8>> {
    let cap = libc::PATH_MAX as usize;
    let mut exe = vec![0u8; cap];
    let mut size = libc::PATH_MAX as u32;
    // SAFETY: `exe` holds `size` bytes and `size` is that length.
    let rc = unsafe { libc::_NSGetExecutablePath(exe.as_mut_ptr().cast(), &mut size) };
    if rc != 0 {
        return None;
    }
    let mut abs = vec![0u8; cap];
    // SAFETY: `exe` is NUL-terminated on success. `abs` holds PATH_MAX bytes.
    let resolved = unsafe { libc::realpath(exe.as_ptr().cast(), abs.as_mut_ptr().cast()) };
    if resolved.is_null() {
        return None;
    }
    let end = abs.iter().position(|b| *b == 0).unwrap_or(abs.len());
    let abs = &abs[..end];
    let mut path = match abs.iter().rposition(|b| *b == b'/') {
        Some(slash) => abs[..=slash].to_vec(),
        None => abs.to_vec(),
    };
    if let Some(name) = c_bytes(name) {
        path.extend_from_slice(name);
    }
    Some(path)
}

/// `os_gettime_ns`. `CLOCK_UPTIME_RAW`, same clock `os_sleepto_ns` subtracts.
pub fn gettime_ns() -> u64 {
    super::nix::gettime_ns()
}

fn add_time(a_sec: i32, a_usec: i32, b_sec: i32, b_usec: i32) -> (i32, i32) {
    let mut microseconds = a_usec + b_usec;
    let mut seconds = a_sec + b_sec;
    if microseconds >= 1_000_000 {
        seconds += microseconds / 1_000_000;
        microseconds %= 1_000_000;
    }
    (seconds, microseconds)
}

#[allow(deprecated)]
fn mach_task() -> libc::mach_port_t {
    // SAFETY: `mach_task_self` returns this process's task port.
    unsafe { libc::mach_task_self() }
}

/// Thread times plus task times, in microseconds, and the uptime clock
/// in microseconds. `None` when `task_info` fails.
fn task_times() -> Option<(i64, i64)> {
    let task = mach_task();
    let mut threads = MaybeUninit::<libc::task_thread_times_info>::zeroed();
    let mut count = libc::TASK_THREAD_TIMES_INFO_COUNT;
    // SAFETY: `threads` is the out buffer and `count` is its natural_t count.
    let kr = unsafe {
        libc::task_info(
            task,
            libc::TASK_THREAD_TIMES_INFO,
            threads.as_mut_ptr().cast(),
            &mut count,
        )
    };
    if kr != libc::KERN_SUCCESS {
        return None;
    }
    // SAFETY: `task_info` succeeded.
    let threads = unsafe { threads.assume_init() };

    let mut task_info = MaybeUninit::<TaskBasicInfo64>::zeroed();
    let mut count = (mem::size_of::<TaskBasicInfo64>() / mem::size_of::<libc::natural_t>()) as u32;
    // SAFETY: `task_info` is the out buffer. The flavor is 18 on arm64 and 5
    // on x86_64, which is what `TASK_BASIC_INFO_64` expands to in the SDK.
    let kr = unsafe {
        libc::task_info(
            task,
            TASK_BASIC_INFO_64,
            task_info.as_mut_ptr().cast(),
            &mut count,
        )
    };
    if kr != libc::KERN_SUCCESS {
        return None;
    }
    // SAFETY: `task_info` succeeded. The struct is packed(4). Copy the
    // time fields; a field reference would not be aligned.
    let task_info = unsafe { task_info.assume_init() };
    let user = unsafe { ptr::addr_of!(task_info.user_time).read_unaligned() };
    let system = unsafe { ptr::addr_of!(task_info.system_time).read_unaligned() };

    let (sec, usec) = add_time(
        threads.user_time.seconds,
        threads.user_time.microseconds,
        threads.system_time.seconds,
        threads.system_time.microseconds,
    );
    let (sec, usec) = add_time(sec, usec, user.seconds, user.microseconds);
    let (sec, usec) = add_time(sec, usec, system.seconds, system.microseconds);
    let cpu_time = (gettime_ns() / 1000) as i64;
    Some((cpu_time, cpu_sys_micros(sec, usec)))
}

/// `seconds * 1000000 + microseconds` as C does it. Both factors are
/// `int`, and the product is widened only after that multiply.
fn cpu_sys_micros(seconds: i32, microseconds: i32) -> i64 {
    i64::from(seconds.wrapping_mul(1_000_000).wrapping_add(microseconds))
}

/// `os_cpu_usage_info`. A `Box` in the shim, not `bmalloc`.
pub struct CpuUsage {
    last_cpu: i64,
    last_sys: i64,
    cores: i32,
}

/// `os_cpu_usage_info_start`. `None` when the time read fails.
pub fn cpu_usage_start() -> Option<CpuUsage> {
    let (cpu, sys) = task_times()?;
    // SAFETY: `_SC_NPROCESSORS_ONLN` is a valid `sysconf` name.
    let cores = unsafe { libc::sysconf(libc::_SC_NPROCESSORS_ONLN) as i32 };
    Some(CpuUsage {
        last_cpu: cpu,
        last_sys: sys,
        cores,
    })
}

/// `os_cpu_usage_info_query`. Null `info`, a failed read, or a zero cpu
/// delta returns 0 and leaves the stored times alone.
pub fn cpu_usage_query(info: Option<&mut CpuUsage>) -> f64 {
    let Some(info) = info else {
        return 0.0;
    };
    let Some((cpu, sys)) = task_times() else {
        return 0.0;
    };
    let sys_delta = sys - info.last_sys;
    let cpu_delta = cpu - info.last_cpu;
    if cpu_delta == 0 {
        return 0.0;
    }
    info.last_sys = sys;
    info.last_cpu = cpu;
    cpu_percent(sys_delta, cpu_delta, info.cores)
}

fn ns_string(bytes: Option<&[u8]>) -> *mut c_void {
    let text = match bytes {
        None => CString::new("").unwrap(),
        Some(bytes) => {
            let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
            CString::new(&bytes[..end]).unwrap()
        }
    };
    // SAFETY: `text` is NUL-terminated. A bad UTF-8 sequence returns nil,
    // which is what `@()` does.
    unsafe {
        msg_id_ptr(
            cls(c"NSString"),
            sel(c"stringWithUTF8String:"),
            text.as_ptr(),
        )
    }
}

/// Retained `beginActivityWithOptions:` token. `None` when the call returns nil.
pub struct PerformanceToken(*mut c_void);

impl PerformanceToken {
    /// The retained object pointer `os_request_high_performance` returns.
    pub fn into_raw(self) -> *mut c_void {
        self.0
    }

    /// `None` for a null token. Does not retain.
    ///
    /// # Safety
    /// A non-null `raw` is the retained token from `into_raw`, and this
    /// call is its only remaining owner. `end_high_performance` releases it.
    pub unsafe fn from_raw(raw: *mut c_void) -> Option<Self> {
        if raw.is_null() { None } else { Some(Self(raw)) }
    }
}

/// `os_request_high_performance`. Null `reason` becomes an empty string.
pub fn request_high_performance(reason: Option<&[u8]>) -> Option<PerformanceToken> {
    let _pool = Pool::new();
    let reason = if reason.is_none() {
        ns_string(Some(b""))
    } else {
        ns_string(reason)
    };
    // SAFETY: `processInfo` is the shared NSProcessInfo. The activity is
    // autoreleased into `_pool`; `objc_retain` keeps it after the pop,
    // which is what `CFBridgingRetain` does under ARC.
    let process = unsafe { msg_id(cls(c"NSProcessInfo"), sel(c"processInfo")) };
    let activity = unsafe {
        msg_id_u64_id(
            process,
            sel(c"beginActivityWithOptions:reason:"),
            NS_ACTIVITY_USER_INITIATED,
            reason,
        )
    };
    let retained = unsafe { objc_retain(activity) };
    if retained.is_null() {
        None
    } else {
        Some(PerformanceToken(retained))
    }
}

/// `os_end_high_performance`. Null is `endActivity:` of nil and does not release.
pub fn end_high_performance(token: Option<PerformanceToken>) {
    let _pool = Pool::new();
    let raw = token.as_ref().map(|t| t.0).unwrap_or(ptr::null_mut());
    // SAFETY: `process` is the shared NSProcessInfo. A non-null token was
    // retained by `request_high_performance` and is released once here,
    // which is what `CFBridgingRelease` does.
    let process = unsafe { msg_id(cls(c"NSProcessInfo"), sel(c"processInfo")) };
    unsafe { msg_void_id(process, sel(c"endActivity:"), raw) };
    if let Some(token) = token {
        unsafe { objc_release(token.0) };
    }
}

fn no_display_sleep_type() -> *const c_void {
    // A raw pointer is not `Sync`. Store the address.
    static TYPE: OnceLock<usize> = OnceLock::new();
    let bits = *TYPE.get_or_init(|| {
        // SAFETY: the literal is the `CFSTR("NoDisplaySleepAssertion")` text.
        // The string is leaked for the process lifetime, as `CFSTR` is.
        unsafe {
            CFStringCreateWithCString(ptr::null(), c"NoDisplaySleepAssertion".as_ptr(), CF_UTF8)
                as usize
        }
    });
    bits as *const c_void
}

/// `os_inhibit_info`. A `Box` in the shim, not `bzalloc`.
pub struct Inhibit {
    reason: *const c_void,
    sleep_id: u32,
    user_id: u32,
    active: bool,
}

/// `os_inhibit_sleep_create`.
/// A null reason is passed to `CFStringCreateWithCString`. On current
/// macOS that faults, as the C call does. Any other reason is replaced
/// with an empty string. The text is ignored. That is the C condition,
/// inverted.
pub fn inhibit_create(reason: Option<&[u8]>) -> Inhibit {
    let cf = if inhibit_uses_null_cstring(reason.is_none()) {
        // SAFETY: the allocator is the default. A null cstring is what C
        // passes. On current macOS the call faults, matching C.
        unsafe { CFStringCreateWithCString(ptr::null(), ptr::null(), CF_UTF8) }
    } else {
        // SAFETY: the empty literal is NUL-terminated. The allocator is default.
        unsafe { CFStringCreateWithCString(ptr::null(), c"".as_ptr(), CF_UTF8) }
    };
    Inhibit {
        reason: cf,
        sleep_id: 0,
        user_id: 0,
        active: false,
    }
}

/// `os_inhibit_sleep_set_active`.
/// Null returns false. An unchanged state returns false.
/// Turning it on declares user activity and then creates the display-sleep
/// assertion. If that create fails, the user-activity id is left allocated,
/// as in C. Turning it off releases only the display-sleep id.
pub fn inhibit_set_active(info: Option<&mut Inhibit>, active: bool) -> bool {
    let Some(info) = info else {
        return false;
    };
    if info.active == active {
        return false;
    }
    if active {
        // SAFETY: `info.reason` is the CFString from `inhibit_create`, or null
        // when the reason pointer was null. The out ids are the struct fields.
        unsafe {
            IOPMAssertionDeclareUserActivity(
                info.reason,
                IOPM_USER_ACTIVE_LOCAL,
                &mut info.user_id,
            );
            let success = IOPMAssertionCreateWithName(
                no_display_sleep_type(),
                IOPM_ASSERTION_LEVEL_ON,
                info.reason,
                &mut info.sleep_id,
            );
            if success != IO_RETURN_SUCCESS {
                blog(LOG_WARNING, c"Failed to disable sleep".as_ptr());
                return false;
            }
        }
    } else {
        // SAFETY: `sleep_id` was stored when the assertion was created.
        unsafe { IOPMAssertionRelease(info.sleep_id) };
    }
    info.active = active;
    true
}

/// `os_inhibit_sleep_destroy`. Null is a no-op.
pub fn inhibit_destroy(info: Option<Inhibit>) {
    let Some(mut info) = info else {
        return;
    };
    inhibit_set_active(Some(&mut info), false);
    // SAFETY: `reason` is null or a CFString from `CFStringCreate*`.
    // `CFRelease` of null is a no-op.
    unsafe { CFRelease(info.reason) };
}

fn sysctl_name(name: &CStr, out: *mut c_void, size: &mut usize) -> i32 {
    // SAFETY: `name` is NUL-terminated. `out` holds `*size` bytes.
    unsafe { libc::sysctlbyname(name.as_ptr(), out, size, ptr::null_mut(), 0) }
}

struct CoreCache {
    initialized: bool,
    physical: i32,
    logical: i32,
}

fn cores() -> (i32, i32) {
    static CACHE: Mutex<CoreCache> = Mutex::new(CoreCache {
        initialized: false,
        physical: 0,
        logical: 0,
    });
    let mut guard = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if guard.initialized {
        return (guard.physical, guard.logical);
    }
    // C sets the flag before the calls, including when the first one fails.
    guard.initialized = true;
    let mut physical = 0i32;
    let mut size = mem::size_of::<i32>();
    let ret = sysctl_name(
        c"machdep.cpu.core_count",
        (&mut physical as *mut i32).cast(),
        &mut size,
    );
    if ret != 0 {
        return (0, 0);
    }
    let mut logical = 0i32;
    let _ = sysctl_name(
        c"machdep.cpu.thread_count",
        (&mut logical as *mut i32).cast(),
        &mut size,
    );
    guard.physical = physical;
    guard.logical = logical;
    (physical, logical)
}

/// `os_get_physical_cores`.
pub fn physical_cores() -> i32 {
    cores().0
}

/// `os_get_logical_cores`.
pub fn logical_cores() -> i32 {
    cores().1
}

/// `os_get_emulation_status`. Always false on arm64. Elsewhere,
/// `sysctl.proc_translated == 1`.
pub fn emulation_status() -> bool {
    #[cfg(target_arch = "aarch64")]
    {
        false
    }
    #[cfg(not(target_arch = "aarch64"))]
    {
        let mut value = 0i32;
        let mut size = mem::size_of::<i32>();
        let rc = sysctl_name(
            c"sysctl.proc_translated",
            (&mut value as *mut i32).cast(),
            &mut size,
        );
        rc != -1 && value == 1
    }
}

#[allow(deprecated)]
fn host_port() -> libc::mach_port_t {
    // SAFETY: `mach_host_self` returns the host port. C does not deallocate it.
    unsafe { libc::mach_host_self() }
}

/// `os_get_sys_free_size`. 0 when `host_statistics` fails.
pub fn sys_free_size() -> u64 {
    let mut vm = MaybeUninit::<libc::vm_statistics>::zeroed();
    let mut count = (mem::size_of::<libc::vm_statistics>() / mem::size_of::<libc::natural_t>())
        as libc::mach_msg_type_number_t;
    // SAFETY: `vm` is the out buffer and `count` is `HOST_VM_INFO_COUNT`.
    let kr = unsafe {
        libc::host_statistics(
            host_port(),
            libc::HOST_VM_INFO,
            vm.as_mut_ptr().cast(),
            &mut count,
        )
    };
    if kr != libc::KERN_SUCCESS {
        return 0;
    }
    // SAFETY: `host_statistics` succeeded.
    let vm = unsafe { vm.assume_init() };
    // SAFETY: `vm_page_size` is the kernel page size.
    let page = unsafe { libc::vm_page_size };
    u64::from(vm.free_count) * u64::try_from(page).unwrap_or(0)
}

/// `os_get_free_space`. Null is 0. An empty string still asks NSURL.
pub fn free_space(path: Option<&[u8]>) -> i64 {
    let Some(path) = path else {
        return 0;
    };
    let _pool = Pool::new();
    let ns = ns_string(Some(path));
    // SAFETY: class methods on NSURL and NSArray. Nil receivers return nil,
    // and `longValue` of nil is 0.
    let url = unsafe { msg_id_id(cls(c"NSURL"), sel(c"fileURLWithPath:"), ns) };
    let important = unsafe { NSURLVolumeAvailableCapacityForImportantUsageKey };
    let available = unsafe { NSURLVolumeAvailableCapacityKey };
    let opportunistic = unsafe { NSURLVolumeAvailableCapacityForOpportunisticUsageKey };
    let keys = [available, important, opportunistic];
    let array = unsafe {
        msg_id_ptr_usize(
            cls(c"NSArray"),
            sel(c"arrayWithObjects:count:"),
            keys.as_ptr(),
            keys.len(),
        )
    };
    let values = unsafe {
        msg_id_id_ptr(
            url,
            sel(c"resourceValuesForKeys:error:"),
            array,
            ptr::null_mut(),
        )
    };
    let important_n = unsafe { msg_id_id(values, sel(c"objectForKeyedSubscript:"), important) };
    let important_v = unsafe { msg_i64(important_n, sel(c"longValue")) };
    if important_v > 0 {
        return important_v;
    }
    let available_n = unsafe { msg_id_id(values, sel(c"objectForKeyedSubscript:"), available) };
    unsafe { msg_i64(available_n, sel(c"longValue")) }
}

/// `os_get_free_disk_space`.
pub fn free_disk_space(path: Option<&[u8]>) -> u64 {
    free_disk_cast(free_space(path))
}

fn sys_total_cached() -> u64 {
    static TOTAL: Mutex<(bool, u64)> = Mutex::new((false, 0));
    let mut guard = TOTAL.lock().unwrap_or_else(|e| e.into_inner());
    if guard.0 {
        return guard.1;
    }
    guard.0 = true;
    let mut total = 0u64;
    let mut size = mem::size_of::<u64>();
    // C ignores the sysctl result. A failure leaves the cached 0.
    let _ = sysctl_name(c"hw.memsize", (&mut total as *mut u64).cast(), &mut size);
    guard.1 = total;
    total
}

/// `os_get_sys_total_size`.
pub fn sys_total_size() -> u64 {
    sys_total_cached()
}

fn read_task_basic() -> Option<libc::mach_task_basic_info> {
    let mut info = MaybeUninit::<libc::mach_task_basic_info>::zeroed();
    let mut count = libc::MACH_TASK_BASIC_INFO_COUNT;
    // SAFETY: `info` is the out buffer and `count` is `MACH_TASK_BASIC_INFO_COUNT`.
    let kr = unsafe {
        libc::task_info(
            mach_task(),
            libc::MACH_TASK_BASIC_INFO,
            info.as_mut_ptr().cast(),
            &mut count,
        )
    };
    if kr != libc::KERN_SUCCESS {
        return None;
    }
    // SAFETY: `task_info` succeeded. The struct is packed(4); fields are copied
    // below with `read_unaligned` by the callers via the returned value's Copy.
    Some(unsafe { info.assume_init() })
}

fn task_sizes(info: &libc::mach_task_basic_info) -> (u64, u64) {
    // SAFETY: the fields are `u64` inside a packed struct. `read_unaligned`
    // does not require the field's natural alignment.
    let virtual_size = unsafe { ptr::addr_of!(info.virtual_size).read_unaligned() };
    let resident = unsafe { ptr::addr_of!(info.resident_size).read_unaligned() };
    (resident, virtual_size)
}

/// `os_get_proc_memory_usage` sizes. `None` when `task_info` fails.
pub struct ProcMem {
    /// Resident bytes.
    pub resident: u64,
    /// Virtual bytes.
    pub virtual_size: u64,
}

/// `os_get_proc_memory_usage`. The null-pointer case is the shim.
pub fn proc_memory() -> Option<ProcMem> {
    let info = read_task_basic()?;
    let (resident, virtual_size) = task_sizes(&info);
    Some(ProcMem {
        resident,
        virtual_size,
    })
}

/// `os_get_proc_resident_size`. 0 when the read fails.
pub fn proc_resident_size() -> u64 {
    proc_memory().map(|m| m.resident).unwrap_or(0)
}

/// `os_get_proc_virtual_size`. 0 when the read fails.
pub fn proc_virtual_size() -> u64 {
    proc_memory().map(|m| m.virtual_size).unwrap_or(0)
}

/// `cfstr_copy_cstr`. `None` for null, `kCFNotFound`, or a failed copy.
/// The `Vec` includes the terminating NUL.
///
/// # Safety
/// `cfstring` is null or a live `CFStringRef` for the duration of the call.
pub unsafe fn copy_cstr(cfstring: *const c_void, encoding: u32) -> Option<Vec<u8>> {
    if cfstring.is_null() {
        return None;
    }
    // SAFETY: `cfstring` is a live CFString, per the caller.
    let quick = unsafe { CFStringGetCStringPtr(cfstring, encoding) };
    if !quick.is_null() {
        // SAFETY: `CFStringGetCStringPtr` returned a NUL-terminated buffer.
        let bytes = unsafe { CStr::from_ptr(quick) }.to_bytes_with_nul();
        return Some(bytes.to_vec());
    }
    // SAFETY: `cfstring` is a live CFString.
    let length = unsafe { CFStringGetLength(cfstring) };
    let max_size = unsafe { CFStringGetMaximumSizeForEncoding(length, encoding) };
    if max_size == K_CF_NOT_FOUND {
        return None;
    }
    let max_size = max_size.checked_add(1)?;
    if max_size <= 0 {
        return None;
    }
    let mut buf = vec![0u8; max_size as usize];
    // SAFETY: `buf` holds `max_size` bytes and `cfstring` is a live CFString.
    let ok = unsafe { CFStringGetCString(cfstring, buf.as_mut_ptr().cast(), max_size, encoding) };
    if ok == 0 {
        return None;
    }
    let end = buf
        .iter()
        .position(|b| *b == 0)
        .unwrap_or(buf.len().saturating_sub(1));
    Some(buf[..=end].to_vec())
}

/// `cfstr_copy_dstr`. False for null, `kCFNotFound`, or a failed copy.
/// On a failed slow path the capacity may grow and the bytes stay.
/// On success `len` is the encoding maximum (including the extra NUL slot),
/// not `strlen`, when the quick pointer was unavailable.
///
/// # Safety
/// `cfstring` is null or a live `CFStringRef` for the duration of the call.
pub unsafe fn copy_into_dstr(cfstring: *const c_void, encoding: u32, dstr: &mut Dstr) -> bool {
    if cfstring.is_null() {
        return false;
    }
    // SAFETY: `cfstring` is a live CFString, per the caller.
    let quick = unsafe { CFStringGetCStringPtr(cfstring, encoding) };
    if !quick.is_null() {
        // SAFETY: the pointer is NUL-terminated.
        let bytes = unsafe { CStr::from_ptr(quick) }.to_bytes();
        dstr.copy(bytes);
        return true;
    }
    // SAFETY: `cfstring` is a live CFString.
    let length = unsafe { CFStringGetLength(cfstring) };
    let max_size = unsafe { CFStringGetMaximumSizeForEncoding(length, encoding) };
    if max_size < 0 {
        return false;
    }
    let Some(max_size) = usize::try_from(max_size)
        .ok()
        .and_then(|n| n.checked_add(1))
    else {
        return false;
    };
    let Ok(max_index) = isize::try_from(max_size) else {
        return false;
    };
    let (bytes, cap0) = mem::replace(dstr, Dstr::new()).into_parts();
    let cap = if max_size <= cap0 {
        cap0
    } else {
        grow_capacity(cap0, max_size)
    };
    let mut buf = vec![0u8; max_size];
    // SAFETY: `buf` holds `max_index` bytes and `cfstring` is a live CFString.
    let ok = unsafe { CFStringGetCString(cfstring, buf.as_mut_ptr().cast(), max_index, encoding) };
    if ok == 0 {
        *dstr = Dstr::from_parts(bytes, cap);
        return false;
    }
    // `dstr_resize` ensures `max_size + 1`. `max_size < cap` is that check
    // without overflowing the addition.
    let cap = if max_size < cap {
        cap
    } else {
        grow_capacity(cap, max_size + 1)
    };
    // Gap bytes past the first NUL are zero. C leaves them uninitialized.
    *dstr = Dstr::from_parts(buf, cap);
    true
}

#[cfg(test)]
mod tests {
    use std::mem::{align_of, offset_of, size_of};

    use super::TaskBasicInfo64;

    /// Offsets measured from `mach/task_info.h` on macOS 27 arm64.
    /// `struct task_basic_info_64` and `task_basic_info_64_2` match.
    #[test]
    fn task_basic_info_64_matches_the_packed_sdk_layout() {
        assert_eq!(size_of::<TaskBasicInfo64>(), 40);
        assert_eq!(align_of::<TaskBasicInfo64>(), 4);
        assert_eq!(offset_of!(TaskBasicInfo64, suspend_count), 0);
        assert_eq!(offset_of!(TaskBasicInfo64, virtual_size), 4);
        assert_eq!(offset_of!(TaskBasicInfo64, resident_size), 12);
        assert_eq!(offset_of!(TaskBasicInfo64, user_time), 20);
        assert_eq!(offset_of!(TaskBasicInfo64, system_time), 28);
        assert_eq!(offset_of!(TaskBasicInfo64, policy), 36);
        assert_eq!(
            size_of::<TaskBasicInfo64>() / size_of::<libc::natural_t>(),
            10
        );
    }

    #[test]
    fn cpu_sys_micros_wraps_in_int() {
        assert_eq!(super::cpu_sys_micros(1, 2), 1_000_002);
        // 3000 * 1000000 does not fit in int32. C wraps, then stores int64.
        assert_eq!(super::cpu_sys_micros(3000, 5), -1_294_967_291);
    }
}
