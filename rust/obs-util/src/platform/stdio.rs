//! ISO C `<stdio.h>` `snprintf` for the `double` conversions libobs uses.
//! The `libc` crate declares it on every supported target (on MSVC against
//! `legacy_stdio_definitions`), so the C runtime chooses the spelling of
//! values it alone decides, such as `inf` and `nan`.

#![allow(unsafe_code)]

use core::ffi::{CStr, c_char, c_int};

// Declared here rather than through `libc::snprintf`: the `libc` crate
// declares it as a DLL import, but on MSVC `snprintf` is an inline function
// that libobs's own objects (platform.obj) already define, so linking libobs
// failed with LNK4217 (a warning treated as error). A plain declaration
// links to that definition, or to `legacy_stdio_definitions`, which `libc`
// links on MSVC; elsewhere it is the C library's `snprintf`.
unsafe extern "C" {
    fn snprintf(s: *mut c_char, n: usize, format: *const c_char, ...) -> c_int;
}

/// A `printf` conversion of one `double`, as libobs spells it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoubleFormat {
    /// `"%.17g"`, from `os_dtostr` (`libobs/util/platform.c`).
    G17,
    /// `"%g"`, from `config_set_default_double` (`libobs/util/config-file.c`).
    G,
    /// `"%.2f"`, from `os_generate_formatted_filename`'s `%FPS`
    /// (`libobs/util/platform-filename.c`).
    F2,
}

impl DoubleFormat {
    /// The precision: significant digits for `%g`, decimals for `%.2f`.
    pub fn precision(self) -> usize {
        match self {
            Self::G17 => 17,
            Self::G => 6,
            Self::F2 => 2,
        }
    }

    fn c_format(self) -> &'static CStr {
        match self {
            Self::G17 => c"%.17g",
            Self::G => c"%g",
            Self::F2 => c"%.2f",
        }
    }
}

/// `snprintf(buf, 64, format, value)` from the C library, returning the
/// written bytes (`os_dtostr` formats into the same 64-byte buffer).
pub fn snprintf_double(format: DoubleFormat, value: f64) -> Vec<u8> {
    let mut buf = [0u8; 64];
    // SAFETY: `buf` is writable for `buf.len()` bytes and `snprintf` writes
    // at most that many, NUL included. The format is a NUL-terminated
    // literal with exactly one `double` conversion, matched by `value`.
    let ret = unsafe {
        snprintf(
            buf.as_mut_ptr().cast::<c_char>(),
            buf.len(),
            format.c_format().as_ptr(),
            value,
        )
    };
    // A negative return is an encoding error; a return >= 64 was cut to 63.
    let len = usize::try_from(ret).unwrap_or(0).min(buf.len() - 1);
    buf[..len].to_vec()
}
