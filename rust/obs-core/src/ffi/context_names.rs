//! C ABI for `libobs/obs-context-names.c.h`.
//!
//! `obs.c` calls the two functions inside libobs, for every context it
//! names. They are declared in `obs-internal.h` without `EXPORT`, so the
//! shims stay hidden (`libobs/cmake/rust-hidden.txt`). The names they return
//! are `bmalloc`'d copies that the caller `bfree`s, as the C's `bstrdup` and
//! `dstr_printf` strings are.
//!
//! `exists` is called exactly as the C calls it: first with the caller's
//! own `name` pointer (NULL included), then with each candidate, in order,
//! until one is free. A candidate is a NUL-terminated buffer that is valid
//! for the call only, as the C's `dstr` is (the next `dstr_printf` may move
//! it). The counter is the caller's `long long`, read and counted up only
//! for an unnamed name.
//!
//! Intentional differences, for pointers `obs.c` never passes: a null
//! `exists` returns NULL without a call, and a null `unnamed_index` returns
//! NULL where an unnamed name is needed (C calls and dereferences them).

use core::ffi::{CStr, c_char, c_longlong, c_void};
use core::ptr;

use crate::context_names::{Lookup, deduplicate, name_dup};

/// `obs_context_name_exists_t` (`obs-internal.h`): whether `name` is taken.
#[allow(non_camel_case_types)]
pub type obs_context_name_exists_t =
    Option<unsafe extern "C" fn(param: *mut c_void, name: *const c_char) -> bool>;

unsafe extern "C" {
    fn bmalloc(size: usize) -> *mut c_void;
}

/// A `bmalloc`'d, NUL-terminated copy of bytes that have no NUL.
fn bstrdup(bytes: &[u8]) -> *mut c_char {
    let Some(size) = bytes.len().checked_add(1) else {
        return ptr::null_mut();
    };
    // SAFETY: `bmalloc` returns `size` writable bytes (it aborts on
    // failure), which hold the copy and its NUL.
    unsafe {
        let out = bmalloc(size).cast::<u8>();
        ptr::copy_nonoverlapping(bytes.as_ptr(), out, bytes.len());
        *out.add(bytes.len()) = 0;
        out.cast()
    }
}

/// The bytes of `name`, `None` for NULL.
///
/// # Safety
///
/// `name` is null or NUL-terminated, and outlives `'a`.
unsafe fn name_bytes<'a>(name: *const c_char) -> Option<&'a [u8]> {
    // SAFETY: the caller guarantees a NUL-terminated string.
    (!name.is_null()).then(|| unsafe { CStr::from_ptr(name) }.to_bytes())
}

/// Same contract as C `obs_context_name_dup`: NULL for a private context
/// with a NULL name; `"__unnamed%04lld"` from `*unnamed_index`, which is
/// counted up, for a NULL or empty name; else a copy of `name`.
///
/// # Safety
///
/// `name` is null or NUL-terminated. `unnamed_index` is null or valid for
/// reads and writes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_context_name_dup(
    name: *const c_char,
    private: bool,
    unnamed_index: *mut c_longlong,
) -> *mut c_char {
    // SAFETY: the caller guarantees `name` is null or NUL-terminated.
    let bytes = unsafe { name_bytes(name) };
    // SAFETY: the caller guarantees `unnamed_index` is null or valid.
    let result = match unsafe { unnamed_index.as_mut() } {
        Some(counter) => name_dup(bytes, private, counter),
        None => {
            // C dereferences the counter only for an unnamed name; without
            // one, that name is NULL.
            let mut unused: i64 = 0;
            name_dup(bytes, private, &mut unused).filter(|_| unused == 0)
        }
    };
    result.map_or(ptr::null_mut(), |name| bstrdup(&name))
}

/// Same contract as C `obs_context_name_deduplicate`: NULL when
/// `exists(param, name)` is false, else `"%s %d"` from 2 up, the first one
/// `exists` says is free.
///
/// # Safety
///
/// `name` is null or NUL-terminated and stays valid during the call;
/// `exists` is null or safe to call with `param` and a name (or `name`
/// itself, NULL included).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_context_name_deduplicate(
    name: *const c_char,
    exists: obs_context_name_exists_t,
    param: *mut c_void,
) -> *mut c_char {
    let Some(exists) = exists else {
        return ptr::null_mut();
    };
    // SAFETY: the caller guarantees `name` is null or NUL-terminated.
    let bytes = unsafe { name_bytes(name) };
    let mut candidate: Vec<u8> = Vec::new();
    let result = deduplicate(bytes, |lookup| match lookup {
        // SAFETY: the caller's `exists`, with the caller's own `name`.
        Lookup::Name(_) => unsafe { exists(param, name) },
        Lookup::Candidate(new_name) => {
            candidate.clear();
            candidate.extend_from_slice(new_name);
            candidate.push(0);
            // SAFETY: the caller's `exists`, with a NUL-terminated name that
            // lives until the next candidate.
            unsafe { exists(param, candidate.as_ptr().cast()) }
        }
    });
    result.map_or(ptr::null_mut(), |name| bstrdup(&name))
}
