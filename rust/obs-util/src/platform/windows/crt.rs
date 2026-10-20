//! C locale, numeric conversion and local time calls used by the platform boundary.
//! These resolve against the UCRT or MinGW CRT used by libobs.

use core::ffi::{CStr, c_char, c_int};

/// The leading member of the CRT's `struct lconv`.
#[repr(C)]
struct LconvPrefix {
    decimal_point: *mut c_char,
}

unsafe extern "C" {
    fn localeconv() -> *mut LconvPrefix;
}

/// First byte of `localeconv()->decimal_point`.
pub fn decimal_point() -> u8 {
    // SAFETY: localeconv returns a non-null pointer to a struct whose first
    // member is a non-null NUL-terminated decimal point string.
    unsafe { *(*localeconv()).decimal_point as u8 }
}

/// `strtod(s, NULL)`.
pub fn strtod(s: &CStr) -> f64 {
    // SAFETY: `s` is NUL-terminated for the duration of the call; a null
    // endptr is permitted by strtod.
    unsafe { libc::strtod(s.as_ptr(), core::ptr::null_mut()) }
}

/// The CRT's `struct tm`.
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Tm {
    tm_sec: c_int,
    tm_min: c_int,
    tm_hour: c_int,
    tm_mday: c_int,
    tm_mon: c_int,
    tm_year: c_int,
    tm_wday: c_int,
    tm_yday: c_int,
    tm_isdst: c_int,
}

// `time` and `localtime` are inline functions in the CRT's <time.h> over
// the 64-bit `_time64` and `_localtime64`, which libobs's C calls; `strftime`
// is a plain CRT export.
unsafe extern "C" {
    #[link_name = "_time64"]
    fn time64(time: *mut i64) -> i64;
    #[link_name = "_localtime64"]
    fn localtime64(time: *const i64) -> *mut Tm;
    fn strftime(buf: *mut c_char, size: usize, format: *const c_char, tm: *const Tm) -> usize;
}

/// `time(NULL)`.
pub fn time_now() -> i64 {
    // SAFETY: `_time64` accepts a NULL out-pointer.
    unsafe { time64(core::ptr::null_mut()) }
}

/// A `struct tm` as `localtime` returned it, copied out of the CRT's
/// buffer right away.
#[derive(Clone, Copy)]
pub struct LocalTime(Tm);

impl LocalTime {
    /// `localtime(&time)` (`_localtime64`), or `None` when it returns NULL,
    /// as `platform-filename.c` calls it.
    pub fn of(time: i64) -> Option<Self> {
        // SAFETY: `time` is a valid `__time64_t`. `_localtime64` returns NULL
        // or its `struct tm`, which is copied before anything else runs here.
        let tm = unsafe { localtime64(&time) };
        // SAFETY: a non-null `tm` points to the CRT's `struct tm`.
        (!tm.is_null()).then(|| Self(unsafe { *tm }))
    }

    /// A `struct tm` with the given fields. The CRT's `struct tm` has no
    /// offset or zone name: its `%z` and `%Z` read the CRT's time zone.
    pub fn from_fields(fields: &super::super::TmFields) -> Self {
        Self(Tm {
            tm_sec: fields.sec,
            tm_min: fields.min,
            tm_hour: fields.hour,
            tm_mday: fields.mday,
            tm_mon: fields.mon,
            tm_year: fields.year,
            tm_wday: fields.wday,
            tm_yday: fields.yday,
            tm_isdst: fields.isdst,
        })
    }

    /// `strftime(buf, 128, format, tm)`: the bytes it wrote, or none when it
    /// returns 0 (an empty result, or one that does not fit).
    pub fn strftime(&self, format: &CStr) -> Vec<u8> {
        let mut buf = [0u8; 128];
        // SAFETY: `buf` is writable for its length, `format` is
        // NUL-terminated and `self.0` is a `struct tm` that lives for the call.
        let len = unsafe { strftime(buf.as_mut_ptr().cast(), buf.len(), format.as_ptr(), &self.0) };
        buf[..len.min(buf.len())].to_vec()
    }
}
