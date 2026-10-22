//! C ABI for `libobs/obs-output-protocols.c.h`.
//!
//! `obs_output_protocol_list_count` is declared in `obs-internal.h` without
//! `EXPORT`, so it stays hidden (`rust-hidden.txt`):
//! `obs_enum_output_types_with_protocol()` in `obs-output.c` calls it once
//! per registered service output and calls its callback that many times for
//! the output. The shim takes the bytes of the two NUL-terminated strings
//! before their NULs and hands them to [`crate::protocols::count_matches`];
//! it reads each string once, to its NUL, as the C's `strlen()` and
//! `strchr()` do.
//!
//! NULL: a NULL list counts 0, as the C does (`while (substr && ...)`). The
//! C defines nothing for a NULL protocol: its `strlen()` crashes on it,
//! with or without a list. The shim returns 0 for it, the count a NULL
//! protocol gets from every lookup since #1828: no output supports it.
//! libobs never passes one, because `obs_enum_output_types_with_protocol()`
//! returns for a NULL protocol before the match. That is the one
//! intentional difference from the C, and nothing in libobs can see it.

use core::ffi::{CStr, c_char};

use crate::protocols;

/// `size_t obs_output_protocol_list_count(const char *protocols,
/// const char *protocol)`.
///
/// # Safety
///
/// As the C: each of `protocols` and `protocol` is NULL or points to a
/// NUL-terminated string that nothing writes during the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_protocol_list_count(
    protocols: *const c_char,
    protocol: *const c_char,
) -> usize {
    if protocols.is_null() || protocol.is_null() {
        return 0;
    }
    // SAFETY: both are non-NULL NUL-terminated strings (caller contract).
    let (list, protocol) = unsafe { (CStr::from_ptr(protocols), CStr::from_ptr(protocol)) };
    protocols::count_matches(list.to_bytes(), protocol.to_bytes())
}
