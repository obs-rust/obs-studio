//! C ABI for `parse_decl_string`.
//!
//! `decl_param_free` and `decl_info_free` stay inline in `decl.h`. They
//! `bfree` the names and `da_free` the parameters, so names are `bmalloc`
//! copies and the parameter array is a `bmalloc` darray, grown one push at
//! a time as `da_push_back` grows it.
//!
//! As in C, a success overwrites `decl->name` and appends to
//! `decl->params`, and a failure frees and zeroes the whole `decl_info`,
//! `decl_string` included. The errors are logged at `LOG_WARNING` with the
//! C text. Intentional differences: a null `decl` returns false (C writes
//! through it); the parse differences are listed in [`crate::decl`].

use std::ffi::{CStr, c_char, c_int};
use std::mem::size_of;
use std::ptr;

use obs_util::ffi::darray::{bfree, bmalloc, darray, darray_ensure_capacity};

use crate::decl::{DeclParam, parse_decl};

const LOG_WARNING: c_int = 200;

unsafe extern "C" {
    fn blog(level: c_int, fmt: *const c_char, ...);
}

/// Layout of `struct decl_param` in `libobs/callback/decl.h`.
#[repr(C)]
pub struct decl_param {
    pub name: *mut c_char,
    /// `enum call_param_type`.
    pub r#type: c_int,
    pub flags: u32,
}

/// Layout of `struct decl_info` in `libobs/callback/decl.h`. `params` is
/// `DARRAY(struct decl_param)`.
#[repr(C)]
pub struct decl_info {
    pub name: *mut c_char,
    pub decl_string: *const c_char,
    pub params: darray,
}

/// # Safety
///
/// `decl` is null or a valid `decl_info` whose `name` and `params` are null
/// or come from `bmalloc`, as `decl_info_free` expects. `decl_string` is
/// null or NUL-terminated, and stays valid as long as `decl` keeps it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn parse_decl_string(
    decl: *mut decl_info,
    decl_string: *const c_char,
) -> bool {
    // SAFETY: the caller guarantees `decl` is null or valid.
    let Some(info) = (unsafe { decl.as_mut() }) else {
        return false;
    };
    info.decl_string = decl_string;
    let text = if decl_string.is_null() {
        &[][..]
    } else {
        // SAFETY: the caller guarantees a NUL-terminated string.
        unsafe { CStr::from_ptr(decl_string) }.to_bytes()
    };

    let parsed = parse_decl(text);
    let success = match &parsed.decl {
        Some(found) => {
            info.name = bstrdup(&found.name);
            for param in &found.params {
                // SAFETY: `params` is a darray of `decl_param`.
                unsafe { push_param(&mut info.params, param) };
            }
            true
        }
        None => {
            // SAFETY: `info` is a valid `decl_info` with `bmalloc` memory.
            unsafe { free_info(info) };
            false
        }
    };

    if let Some(mut warning) = parsed.warning(text) {
        warning.push(0);
        // SAFETY: "%s" takes one NUL-terminated string, and `warning` is one.
        unsafe { blog(LOG_WARNING, c"%s".as_ptr(), warning.as_ptr()) };
    }
    success
}

/// `bstrdup` of bytes that have no NUL.
fn bstrdup(bytes: &[u8]) -> *mut c_char {
    // SAFETY: `bmalloc` returns `len + 1` writable bytes (it aborts on
    // failure), which hold the copy and its NUL.
    unsafe {
        let out = bmalloc(bytes.len() + 1).cast::<u8>();
        ptr::copy_nonoverlapping(bytes.as_ptr(), out, bytes.len());
        *out.add(bytes.len()) = 0;
        out.cast()
    }
}

/// `da_push_back(decl->params, &param)`.
///
/// # Safety
///
/// `params` is a valid darray of `decl_param`.
unsafe fn push_param(params: &mut darray, param: &DeclParam) {
    let item = decl_param {
        name: bstrdup(&param.name),
        r#type: param.kind as c_int,
        flags: param.flags,
    };
    let num = params.num;
    // SAFETY: the caller guarantees a darray of `decl_param`; after the
    // ensure, `array` holds at least `num + 1` items.
    unsafe {
        darray_ensure_capacity(size_of::<decl_param>(), params, num + 1);
        params.array.cast::<decl_param>().add(num).write(item);
    }
    params.num = num + 1;
}

/// `decl_info_free`.
///
/// # Safety
///
/// `info` is a valid `decl_info` with `bmalloc` memory.
unsafe fn free_info(info: &mut decl_info) {
    let items = info.params.array.cast::<decl_param>();
    for i in 0..info.params.num {
        // SAFETY: the first `num` items are valid `decl_param`s.
        let name = unsafe { (*items.add(i)).name };
        if !name.is_null() {
            // SAFETY: parameter names are `bmalloc` copies.
            unsafe { bfree(name.cast()) };
        }
    }
    // SAFETY: `array` and `name` are null or `bmalloc` memory; `bfree`
    // accepts null as C's does.
    unsafe {
        bfree(info.params.array);
        bfree(info.name.cast());
    }
    info.name = ptr::null_mut();
    info.decl_string = ptr::null();
    info.params = darray {
        array: ptr::null_mut(),
        num: 0,
        capacity: 0,
    };
}
