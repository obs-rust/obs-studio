//! C locale, numeric conversion and local time calls used by the platform boundary.

#![allow(unsafe_code)]

use std::ffi::CStr;

/// First byte of `localeconv()->decimal_point`.
pub fn decimal_point() -> u8 {
    // SAFETY: localeconv returns a non-null pointer to a struct whose
    // decimal_point member is a non-null NUL-terminated string.
    unsafe { *(*libc::localeconv()).decimal_point as u8 }
}

/// `strtod(s, NULL)`.
pub fn strtod(s: &CStr) -> f64 {
    // SAFETY: `s` is NUL-terminated for the duration of the call; a null
    // endptr is permitted by strtod.
    unsafe { libc::strtod(s.as_ptr(), std::ptr::null_mut()) }
}

/// `time(NULL)`.
#[allow(clippy::useless_conversion)] // `time_t` is 32-bit on some Unix targets
pub fn time_now() -> i64 {
    // SAFETY: `time` accepts a NULL out-pointer.
    i64::from(unsafe { libc::time(std::ptr::null_mut()) })
}

/// A `struct tm` as `localtime` returned it, copied out of the C library's
/// static buffer right away.
#[derive(Clone, Copy)]
pub struct LocalTime(libc::tm);

impl LocalTime {
    /// `localtime(&time)`, or `None` when it returns NULL. It is
    /// `localtime`, not `localtime_r`, as in `platform-filename.c`: it
    /// re-reads `TZ` on every call and shares its buffer with other C
    /// `localtime` callers in the process.
    pub fn of(time: i64) -> Option<Self> {
        let time = libc::time_t::try_from(time).ok()?;
        // SAFETY: `time` is a valid `time_t`. `localtime` returns NULL or its
        // static `struct tm`, which is copied before anything else runs here.
        let tm = unsafe { libc::localtime(&time) };
        // SAFETY: a non-null `tm` points to the C library's `struct tm`.
        (!tm.is_null()).then(|| Self(unsafe { *tm }))
    }

    /// A `struct tm` with the given fields.
    pub fn from_fields(fields: &super::super::TmFields) -> Self {
        // SAFETY: `struct tm` is plain data; all zeros (a NULL `tm_zone`)
        // is a valid value, and every field that strftime reads is set below.
        let mut tm: libc::tm = unsafe { std::mem::zeroed() };
        tm.tm_sec = fields.sec;
        tm.tm_min = fields.min;
        tm.tm_hour = fields.hour;
        tm.tm_mday = fields.mday;
        tm.tm_mon = fields.mon;
        tm.tm_year = fields.year;
        tm.tm_wday = fields.wday;
        tm.tm_yday = fields.yday;
        tm.tm_isdst = fields.isdst;
        tm.tm_gmtoff = fields.gmtoff;
        tm.tm_zone = fields.zone.as_ptr().cast_mut();
        Self(tm)
    }

    /// `strftime(buf, 128, format, tm)`: the bytes it wrote, or none when it
    /// returns 0 (an empty result, or one that does not fit).
    pub fn strftime(&self, format: &CStr) -> Vec<u8> {
        let mut buf = [0u8; 128];
        // SAFETY: `buf` is writable for its length, `format` is
        // NUL-terminated and `self.0` is a `struct tm` that lives for the call.
        let len =
            unsafe { libc::strftime(buf.as_mut_ptr().cast(), buf.len(), format.as_ptr(), &self.0) };
        buf[..len.min(buf.len())].to_vec()
    }
}
