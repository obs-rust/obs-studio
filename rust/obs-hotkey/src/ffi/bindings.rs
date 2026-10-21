//! C ABI for `libobs/obs-hotkey-bindings.c.h`, declared in
//! `libobs/obs-internal.h` without `EXPORT`.
//!
//! `load_binding()`, `create_binding()` and `save_bindings_helper()` in
//! `obs-hotkey.c.h` call these inside libobs, so the symbols stay hidden
//! (`rust-hidden.txt`). The obs_data reads and writes are the C's calls in
//! the C's order: `obs_data_get_bool` for each modifier and
//! `obs_data_get_string` for the key; `obs_data_set_bool(name, true)` for
//! each set modifier and `obs_data_set_string` of `obs_key_to_name` for the
//! key. So the saved JSON, its key order included, is the C's byte for
//! byte. `obs_key_t` is a C enum, passed as `int`.
//!
//! Intentional difference from C, where C has undefined behavior: a null
//! `combo_out` returns false and writes nothing (C writes through it). A
//! null `obs_data_get_string` result reads as `""`, as it does in
//! `obs_key_from_name`. A null `data` or `hotkey` is passed on to obs_data,
//! which ignores it, as in C.

use std::ffi::{CStr, c_char, c_void};

use super::hotkey_name_map::obs_key_to_name;
use super::nix_hotkey_state::obs_key_combination;
use crate::bindings::{self as core, Field, KEY_FIELD, MODIFIERS};
use crate::nix_hotkey_state::KeyCombination;

unsafe extern "C" {
    fn obs_data_get_bool(data: *mut c_void, name: *const c_char) -> bool;
    fn obs_data_get_string(data: *mut c_void, name: *const c_char) -> *const c_char;
    fn obs_data_set_bool(data: *mut c_void, name: *const c_char, val: bool);
    fn obs_data_set_string(data: *mut c_void, name: *const c_char, val: *const c_char);
}

fn to_core(c: obs_key_combination) -> KeyCombination {
    KeyCombination {
        modifiers: c.modifiers,
        key: c.key,
    }
}

/// The parse of `load_binding()`: false where it returns without a
/// binding (no modifiers and no known key), as in C.
///
/// # Safety
///
/// `data` is null or a live `obs_data_t`; `combo_out` is null or valid for
/// a write.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkey_combination_from_data(
    data: *mut c_void,
    combo_out: *mut obs_key_combination,
) -> bool {
    // SAFETY: the caller passes a live obs_data or null, which obs_data
    // ignores; the names are static NUL-terminated strings.
    let modifiers = MODIFIERS.map(|m| unsafe { obs_data_get_bool(data, m.name.as_ptr()) });
    // SAFETY: as above.
    let name = unsafe { obs_data_get_string(data, KEY_FIELD.as_ptr()) };
    let name = if name.is_null() {
        c""
    } else {
        // SAFETY: obs_data returns a NUL-terminated string that lives as
        // long as its item, which outlives this call.
        unsafe { CStr::from_ptr(name) }
    };
    let Some(combo) = core::combo_from_fields(modifiers, name) else {
        return false;
    };
    if combo_out.is_null() {
        return false;
    }
    // SAFETY: the caller vouches that a non-null `combo_out` is writable.
    unsafe {
        combo_out.write(obs_key_combination {
            modifiers: combo.modifiers,
            key: combo.key,
        });
    }
    true
}

/// The rule of `create_binding()`, for JSON and API bindings alike.
#[unsafe(no_mangle)]
pub extern "C" fn obs_hotkey_combination_bindable(combo: obs_key_combination) -> bool {
    core::combo_bindable(to_core(combo))
}

/// The per-binding body of `save_bindings_helper()`.
///
/// # Safety
///
/// `hotkey` is null or a live `obs_data_t`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkey_combination_to_data(
    combo: obs_key_combination,
    hotkey: *mut c_void,
) {
    for field in core::combo_to_fields(to_core(combo)) {
        match field {
            // SAFETY: a live obs_data or null; static names.
            Field::Modifier(name) => unsafe { obs_data_set_bool(hotkey, name.as_ptr(), true) },
            // SAFETY: as above; obs_key_to_name returns a static string
            // (and logs an unknown code, as the C does).
            Field::Key(key) => unsafe {
                obs_data_set_string(hotkey, KEY_FIELD.as_ptr(), obs_key_to_name(key));
            },
        }
    }
}
