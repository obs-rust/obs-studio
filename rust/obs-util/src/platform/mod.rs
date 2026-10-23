//! The platform boundary (obs-rust/obs-studio#323): the only place in the
//! safe cores that selects on the host. Each OS child holds that host's
//! native code; the rest of the crate uses the neutral items re-exported
//! here. The `ban_platform_cfg_outside_boundary` Dylint rejects host `cfg`s
//! and native OS paths (`std::os::*`, `libc::`, Win32) outside this module
//! and [`crate::ffi`].

/// `enum os_event_type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventType {
    /// `OS_EVENT_TYPE_AUTO`: a successful wait consumes the signal.
    Auto,
    /// `OS_EVENT_TYPE_MANUAL`: the event stays signalled until `reset`.
    Manual,
}

/// The fields of a `struct tm`, for [`crt::LocalTime::from_fields`].
/// `gmtoff` and `zone` are `tm_gmtoff` and `tm_zone`. glibc's `strftime`
/// reads `%z` and `%Z` from them; macOS reads `%Z` from `tm_zone` but `%z`
/// from the C library's time zone, and the Windows CRT's `struct tm` has
/// neither field (both come from its time zone).
#[derive(Debug, Clone, Copy)]
pub struct TmFields {
    /// `tm_sec`.
    pub sec: i32,
    /// `tm_min`.
    pub min: i32,
    /// `tm_hour`.
    pub hour: i32,
    /// `tm_mday`.
    pub mday: i32,
    /// `tm_mon`, from 0.
    pub mon: i32,
    /// `tm_year`, years since 1900.
    pub year: i32,
    /// `tm_wday`, 0 for Sunday.
    pub wday: i32,
    /// `tm_yday`, from 0.
    pub yday: i32,
    /// `tm_isdst`.
    pub isdst: i32,
    /// `tm_gmtoff`: seconds east of UTC.
    pub gmtoff: CLong,
    /// `tm_zone`: the zone abbreviation.
    pub zone: &'static core::ffi::CStr,
}

cfg_select! {
    unix => {
        pub mod unix;
        pub use unix::{Event, Pipe, Sem, close_file, crt, path_from_bytes};
        pub use unix::file::{
            CFile, SEEK_CUR, SEEK_END, SEEK_SET, WChar, errno, mbs_to_wide, set_errno, utf8_to_wide,
            wide_to_mbs, wide_to_utf8,
        };
        pub use unix::nix::{gettime_ns, mkdir, safe_replace};
    }
    windows => {
        #[allow(unsafe_code)] // Win32 handle and string calls; see the module docs
        pub mod windows;
        pub use windows::{Event, Pipe, Sem, close_file, crt, path_from_bytes};
        pub use windows::file::{
            CFile, SEEK_CUR, SEEK_END, SEEK_SET, WChar, errno, mbs_to_wide, set_errno, utf8_to_wide,
            wide_to_mbs, wide_to_utf8,
        };
        pub use windows::host::{gettime_ns, mkdir, safe_replace};
    }
}

/// `os_strtod` from `libobs/util/platform-number.c`: copy the first 63
/// bytes, translate the first `.` to the current locale's decimal point,
/// then let the host C runtime parse and round the value.
pub fn os_strtod(bytes: &[u8]) -> f64 {
    let mut buf: Vec<u8> = bytes
        .iter()
        .copied()
        .take(63)
        .take_while(|&b| b != 0)
        .collect();
    let point = crt::decimal_point();
    if point != b'.'
        && let Some(pos) = buf.iter().position(|&b| b == b'.')
    {
        if point == 0 {
            buf.truncate(pos);
        } else {
            buf[pos] = point;
        }
    }
    // The buffer ends at its first NUL, so CString construction cannot fail.
    let value = std::ffi::CString::new(buf).unwrap_or_default();
    crt::strtod(&value)
}

cfg_select! {
    windows => {
        /// C `long` is 32-bit on Windows and 64-bit on the other supported targets.
        pub type CLong = i32;
        /// Atomic with the width of C `long` (`os_atomic_*_long`).
        pub type CLongAtomic = core::sync::atomic::AtomicI32;
        /// `config_save` writes a UTF-8 BOM first on Windows.
        pub const CONFIG_FILE_BOM: &[u8] = b"\xEF\xBB\xBF";
    }
    _ => {
        /// C `long` is 32-bit on Windows and 64-bit on the other supported targets.
        pub type CLong = i64;
        /// Atomic with the width of C `long` (`os_atomic_*_long`).
        pub type CLongAtomic = core::sync::atomic::AtomicI64;
        /// `config_save` writes a UTF-8 BOM first on Windows only.
        pub const CONFIG_FILE_BOM: &[u8] = b"";
    }
}

/// `util_mul_div64` uses `_umul128`/`_udiv128` on MSVC x64, which compute
/// the exact quotient (and fault when it overflows 64 bits); everywhere
/// else it uses the overflowing split formula.
pub const MUL_DIV64_IS_UDIV128: bool = cfg!(all(target_env = "msvc", target_arch = "x86_64"));

mod stdio;
pub use stdio::{DoubleFormat, snprintf_double, snprintf_ptr};

/// The `Sleep` argument `os_sleep_ms(ms)` uses on a Windows host whose
/// version is `winver` (`major << 8 | minor`). Windows 8+ (`0x0602`) sleeps
/// about 1 ms long, so a request above 1 ms is shortened by 1. A 1 ms
/// request stays 1: `Sleep(0)` only yields (obs-rust/obs-studio#1274).
pub fn win_sleep_duration(ms: u32, winver: u32) -> u32 {
    if winver >= 0x0602 && ms > 1 {
        ms - 1
    } else {
        ms
    }
}
