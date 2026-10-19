//! C ABI shim for `os_mkdirs` from `libobs/util/platform-mkdirs.c`
//! (`libobs/util/platform.h`), over the safe core in
//! [`crate::platform_mkdirs`].
//!
//! A NULL or empty `dir` returns `MKDIR_ERROR`.

use core::ffi::{CStr, c_char, c_int};

use crate::platform_mkdirs::{MKDIR_ERROR, mkdirs};

/// # Safety
/// `dir` is a C string or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn os_mkdirs(dir: *const c_char) -> c_int {
    if dir.is_null() {
        return MKDIR_ERROR;
    }
    // SAFETY: the caller guarantees a valid NUL-terminated string.
    mkdirs(unsafe { CStr::from_ptr(dir) }.to_bytes())
}
