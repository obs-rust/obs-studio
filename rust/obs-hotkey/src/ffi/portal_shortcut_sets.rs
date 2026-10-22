//! C ABI for `libobs/obs-nix-portal-shortcut-sets.c.h`, declared in
//! `libobs/obs-nix-portal-shortcuts.h`.
//!
//! The GlobalShortcuts portal client (`obs-nix-portal-shortcuts.c`) calls
//! these inside libobs on Linux and FreeBSD with `ENABLE_WAYLAND` and GIO,
//! on its portal thread and in the Activated handler. They are declared
//! without `EXPORT`, so the symbols stay hidden (`rust-hidden.txt`).
//! Elsewhere the shims are built but unused, and hidden all the same. They
//! call nothing from GIO, so they need no feature.
//!
//! The arrays are the caller's, read through two views,
//! `struct obs_nix_portal_combos` and `struct obs_nix_portal_descriptions`
//! (pointer and count), and the results are positions in them written to
//! the caller's buffers. Nothing is allocated, copied or kept. A
//! description is compared as bytes up to its NUL, as `strcmp` does, and
//! only when the C would compare it.
//!
//! Intentional differences from C, where C has undefined behavior: a null
//! view, or a view with a null array, is an empty set (C reads `num`
//! entries through it); a null count out-pointer is not written, and a null
//! position buffer receives nothing (C writes through them). With the
//! inputs the C defines, the results, the positions and every entry
//! written are the same.

use std::ffi::{CStr, c_char};
use std::ptr::NonNull;

use super::nix_hotkey_state::obs_key_combination;
use crate::nix_hotkey_state::KeyCombination;
use crate::portal_shortcut_sets::{self as core, Combination};

/// `struct obs_nix_portal_combos`: a combination array the caller owns.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
#[allow(non_camel_case_types)]
pub struct obs_nix_portal_combos {
    pub array: *const obs_key_combination,
    pub num: usize,
}

/// `struct obs_nix_portal_descriptions`: the description of each
/// combination, parallel to an `obs_nix_portal_combos`; each may be NULL.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
#[allow(non_camel_case_types)]
pub struct obs_nix_portal_descriptions {
    pub array: *const *const c_char,
    pub num: usize,
}

impl Combination for obs_key_combination {
    fn combination(self) -> KeyCombination {
        KeyCombination {
            modifiers: self.modifiers,
            key: self.key,
        }
    }
}

/// A non-null description of the caller's: a NUL-terminated string that
/// lives for the call. `Option<Description>` has the layout of
/// `const char *`, NULL being `None`.
#[repr(transparent)]
#[derive(Debug, Clone, Copy)]
pub struct Description(NonNull<c_char>);

impl PartialEq for Description {
    /// `strcmp(a, b) == 0`: the same bytes up to the NUL.
    fn eq(&self, other: &Self) -> bool {
        // SAFETY: a Description only comes from a descriptions view, whose
        // strings the caller keeps NUL-terminated and alive for the call.
        unsafe { CStr::from_ptr(self.0.as_ptr()) == CStr::from_ptr(other.0.as_ptr()) }
    }
}

/// The caller's combinations, empty for a null view or array.
///
/// # Safety
///
/// `view` is null or valid, and its array is null or holds `num` entries
/// that live, unchanged, for `'a`.
unsafe fn combos<'a>(view: *const obs_nix_portal_combos) -> &'a [obs_key_combination] {
    // SAFETY: null or a valid view, per the contract.
    match unsafe { view.as_ref() } {
        // SAFETY: `num` entries at a non-null array.
        Some(v) if !v.array.is_null() => unsafe { std::slice::from_raw_parts(v.array, v.num) },
        _ => &[],
    }
}

/// The caller's descriptions, empty for a null view or array.
///
/// # Safety
///
/// As [`combos`], and each entry is NULL or a NUL-terminated string that
/// lives for `'a`.
unsafe fn descriptions<'a>(view: *const obs_nix_portal_descriptions) -> &'a [Option<Description>] {
    // SAFETY: null or a valid view, per the contract.
    match unsafe { view.as_ref() } {
        // SAFETY: `num` pointers at a non-null array; `Option<Description>`
        // is a transparent `NonNull<c_char>` with NULL as `None`, the layout
        // of `const char *`.
        Some(v) if !v.array.is_null() => unsafe {
            std::slice::from_raw_parts(v.array.cast::<Option<Description>>(), v.num)
        },
        _ => &[],
    }
}

/// The caller's position buffer with room for `num`, empty for null.
///
/// # Safety
///
/// `out` is null or writable for `num` positions, not used through any
/// other path for `'a`.
unsafe fn positions<'a>(out: *mut usize, num: usize) -> &'a mut [usize] {
    if out.is_null() {
        return &mut [];
    }
    // SAFETY: writable for `num` positions, per the contract.
    unsafe { std::slice::from_raw_parts_mut(out, num) }
}

/// Writes `value` to `out` unless it is null.
///
/// # Safety
///
/// `out` is null or writable.
unsafe fn put(out: *mut usize, value: usize) {
    // SAFETY: null or writable, per the contract.
    if let Some(out) = unsafe { out.as_mut() } {
        *out = value;
    }
}

/// Same contract as C `obs_nix_portal_sets_has`.
///
/// # Safety
///
/// `combos` is null or a valid view.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_nix_portal_sets_has(
    combos: *const obs_nix_portal_combos,
    combo: obs_key_combination,
) -> bool {
    // SAFETY: per the contract.
    core::has(unsafe { self::combos(combos) }, combo.combination())
}

/// Same contract as C `obs_nix_portal_sets_unchanged`.
///
/// # Safety
///
/// Each view is null or valid, with descriptions that are NULL or
/// NUL-terminated strings.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn obs_nix_portal_sets_unchanged(
    session: bool,
    retry: bool,
    partly_declined: bool,
    current: *const obs_nix_portal_combos,
    current_descriptions: *const obs_nix_portal_descriptions,
    bound: *const obs_nix_portal_combos,
    bound_descriptions: *const obs_nix_portal_descriptions,
    declined: *const obs_nix_portal_combos,
) -> bool {
    // SAFETY: per the contract.
    unsafe {
        core::unchanged(
            session,
            retry,
            partly_declined,
            combos(current),
            descriptions(current_descriptions),
            combos(bound),
            descriptions(bound_descriptions),
            combos(declined),
        )
    }
}

/// Same contract as C `obs_nix_portal_sets_request`.
///
/// # Safety
///
/// `current` and `declined` are null or valid views; `request` is null or
/// has room for `current->num` positions; `num_request` is null or
/// writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_nix_portal_sets_request(
    current: *const obs_nix_portal_combos,
    partly_declined: bool,
    declined: *const obs_nix_portal_combos,
    request: *mut usize,
    num_request: *mut usize,
) -> bool {
    // SAFETY: per the contract.
    unsafe {
        let current = combos(current);
        let got = core::request(
            current,
            partly_declined,
            combos(declined),
            positions(request, current.len()),
        );
        put(num_request, got.num);
        got.partly
    }
}

/// Same contract as C `obs_nix_portal_sets_partition`.
///
/// # Safety
///
/// `current`, `approved` and `request` are null or valid views; `part` and
/// `declined` are null or have room for `current->num` positions each, and
/// do not overlap; `num_part` and `num_declined` are null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_nix_portal_sets_partition(
    current: *const obs_nix_portal_combos,
    approved: *const obs_nix_portal_combos,
    request: *const obs_nix_portal_combos,
    part: *mut usize,
    num_part: *mut usize,
    declined: *mut usize,
    num_declined: *mut usize,
) -> bool {
    // SAFETY: per the contract.
    unsafe {
        let current = combos(current);
        let got = core::partition(
            current,
            combos(approved),
            combos(request),
            positions(part, current.len()),
            positions(declined, current.len()),
        );
        put(num_part, got.num_part);
        put(num_declined, got.num_declined);
        got.bind
    }
}
