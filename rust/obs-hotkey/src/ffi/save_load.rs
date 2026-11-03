//! C ABI for `libobs/obs-hotkey-save-load.c.h`, the hotkey save and load
//! functions, EXPORT through `libobs/obs-hotkey.h`.
//!
//! `load_bindings()` is not EXPORT: `obs_hotkey_register_internal()` in
//! `obs-hotkey.c.h` calls it for the bindings a new hotkey has stored, so it
//! stays hidden (`rust-hidden.d`).
//!
//! The registry stays C. These shims reach it only through the private seam
//! `libobs/obs-hotkey-seam.h` and the public getters, so `struct obs_hotkey`,
//! `struct obs_hotkey_pair`, `struct obs_hotkey_binding` and the object
//! layouts are never mirrored. The lock, the lookups, the obs_data calls and
//! the signal come in the C's order, so the saved JSON and its key order are
//! the C's, and the signal is still sent with the hotkey mutex held.
//!
//! Intentional differences from C, where C has undefined behavior: none of
//! these changes a defined result.
//! - `obs_hotkey_load_bindings()` reads `combinations` one element at a time,
//!   so a null pointer with `num` 0 forms no reference.
//! - The `obs_hotkeys_save_*` functions, like the C, take no null check of
//!   their object. The seam dereferences it.

use std::ffi::c_void;

use super::binding_press::obs_hotkey_binding;
use super::bindings::{
    obs_hotkey_combination_bindable, obs_hotkey_combination_from_data,
    obs_hotkey_combination_to_data,
};
use super::nix_hotkey_state::obs_key_combination;
use super::pair::{obs_hotkey_pair, obs_hotkey_t};
use crate::save_load as core;

type BindingEnumFunc =
    unsafe extern "C" fn(data: *mut c_void, idx: usize, binding: *mut obs_hotkey_binding) -> bool;
type HotkeyEnumFunc = unsafe extern "C" fn(data: *mut c_void, hotkey: *mut obs_hotkey_t) -> bool;

unsafe extern "C" {
    fn obs_hotkey_seam_lock() -> bool;
    fn obs_hotkey_seam_unlock();
    fn obs_hotkey_seam_find(id: usize) -> *mut obs_hotkey_t;
    fn obs_hotkey_seam_find_pair(id: usize) -> *mut obs_hotkey_pair;
    fn obs_hotkey_seam_pair_id(pair: *const obs_hotkey_pair, half: usize) -> usize;
    fn obs_hotkey_seam_add_binding(hotkey: *mut obs_hotkey_t, combo: obs_key_combination);
    fn obs_hotkey_seam_remove_bindings(id: usize) -> bool;
    fn obs_hotkey_seam_signal(signal: *const std::ffi::c_char, hotkey: *mut obs_hotkey_t);
    fn obs_hotkey_seam_enum_bindings(func: BindingEnumFunc, data: *mut c_void);
    fn obs_hotkey_seam_enum_context(context: *mut c_void, func: HotkeyEnumFunc, data: *mut c_void);
    fn obs_hotkey_seam_context_count(context: *const c_void) -> usize;
    fn obs_hotkey_seam_saved_volume_keys_swapped() -> bool;
    fn obs_hotkey_seam_encoder_context(encoder: *mut c_void) -> *mut c_void;
    fn obs_hotkey_seam_output_context(output: *mut c_void) -> *mut c_void;
    fn obs_hotkey_seam_service_context(service: *mut c_void) -> *mut c_void;
    fn obs_hotkey_seam_source_context(source: *mut c_void) -> *mut c_void;

    fn obs_hotkey_get_id(key: *const obs_hotkey_t) -> usize;
    fn obs_hotkey_get_name(key: *const obs_hotkey_t) -> *const std::ffi::c_char;
    fn obs_hotkey_binding_get_key_combination(
        binding: *mut obs_hotkey_binding,
    ) -> obs_key_combination;
    fn obs_hotkey_binding_get_hotkey_id(binding: *mut obs_hotkey_binding) -> usize;

    fn obs_data_create() -> *mut c_void;
    fn obs_data_release(data: *mut c_void);
    fn obs_data_get_bool(data: *mut c_void, name: *const std::ffi::c_char) -> bool;
    fn obs_data_set_bool(data: *mut c_void, name: *const std::ffi::c_char, val: bool);
    fn obs_data_get_array(data: *mut c_void, name: *const std::ffi::c_char) -> *mut c_void;
    fn obs_data_set_array(data: *mut c_void, name: *const std::ffi::c_char, array: *mut c_void);
    fn obs_data_array_create() -> *mut c_void;
    fn obs_data_array_release(array: *mut c_void);
    fn obs_data_array_count(array: *mut c_void) -> usize;
    fn obs_data_array_item(array: *mut c_void, idx: usize) -> *mut c_void;
    fn obs_data_array_push_back(array: *mut c_void, obj: *mut c_void) -> usize;
}

/// `create_binding()`.
///
/// # Safety
///
/// `hotkey` is a live hotkey; the caller holds the hotkey mutex.
unsafe fn create_binding(hotkey: *mut obs_hotkey_t, combo: obs_key_combination) {
    if !obs_hotkey_combination_bindable(combo) {
        return;
    }
    // SAFETY: per the caller.
    unsafe { obs_hotkey_seam_add_binding(hotkey, combo) };
}

/// `load_binding()`.
///
/// # Safety
///
/// `hotkey` is null or a live hotkey; `data` is null or a live `obs_data_t`;
/// the caller holds the hotkey mutex.
unsafe fn load_binding(hotkey: *mut obs_hotkey_t, data: *mut c_void, saved: bool) {
    if hotkey.is_null() || data.is_null() {
        return;
    }
    let mut combo = obs_key_combination::default();
    // SAFETY: a live obs_data, and `combo` is writable.
    if !unsafe { obs_hotkey_combination_from_data(data, &raw mut combo) } {
        return;
    }
    combo.key = core::loaded_key(
        combo.key,
        saved,
        // SAFETY: a live obs_data; a static name.
        || unsafe { obs_data_get_bool(data, core::VOLUME_KEYS_FIXED.as_ptr()) },
        // SAFETY: the caller holds the hotkey mutex, as in the C.
        || unsafe { obs_hotkey_seam_saved_volume_keys_swapped() },
    );
    // SAFETY: per the caller.
    unsafe { create_binding(hotkey, combo) };
}

/// `removed`: the caller's `remove_bindings` removed bindings of this hotkey
/// first, so an empty or missing array still changed them. `saved`: the data
/// is a scene collection's.
///
/// # Safety
///
/// `hotkey` is null or a live hotkey; `data` is null or a live
/// `obs_data_array_t`; the caller holds the hotkey mutex.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn load_bindings(
    hotkey: *mut obs_hotkey_t,
    data: *mut c_void,
    removed: bool,
    saved: bool,
) {
    // SAFETY: a live array or null, which obs_data ignores.
    let count = unsafe { obs_data_array_count(data) };
    for i in 0..count {
        // SAFETY: as above; `i` is in range, and the reference is released
        // below.
        let item = unsafe { obs_data_array_item(data, i) };
        // SAFETY: per the caller; a live item or null.
        unsafe {
            load_binding(hotkey, item, saved);
            obs_data_release(item);
        }
    }
    if core::load_signals(count, removed) {
        // SAFETY: a static name; per the caller.
        unsafe { obs_hotkey_seam_signal(core::BINDINGS_CHANGED.as_ptr(), hotkey) };
    }
}

/// `obs_hotkey_load_bindings()`.
///
/// # Safety
///
/// `combinations` is valid for `num` reads.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkey_load_bindings(
    id: usize,
    combinations: *const obs_key_combination,
    num: usize,
) {
    // SAFETY: no precondition.
    if !unsafe { obs_hotkey_seam_lock() } {
        return;
    }
    // SAFETY: the lock is held.
    let hotkey = unsafe { obs_hotkey_seam_find(id) };
    if !hotkey.is_null() {
        // SAFETY: the lock is held.
        let changed = unsafe { obs_hotkey_seam_remove_bindings(id) };
        for i in 0..num {
            // SAFETY: per the caller; a live hotkey, and the lock is held.
            unsafe { create_binding(hotkey, combinations.add(i).read()) };
        }
        if core::load_signals(num, changed) {
            // SAFETY: a static name; a live hotkey; the lock is held.
            unsafe { obs_hotkey_seam_signal(core::BINDINGS_CHANGED.as_ptr(), hotkey) };
        }
    }
    // SAFETY: taken above.
    unsafe { obs_hotkey_seam_unlock() };
}

/// `obs_hotkey_load()`.
///
/// # Safety
///
/// `data` is null or a live `obs_data_array_t`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkey_load(id: usize, data: *mut c_void) {
    // SAFETY: no precondition.
    if !unsafe { obs_hotkey_seam_lock() } {
        return;
    }
    // SAFETY: the lock is held.
    let hotkey = unsafe { obs_hotkey_seam_find(id) };
    if !hotkey.is_null() {
        // SAFETY: the lock is held; a live hotkey; per the caller.
        unsafe {
            let removed = obs_hotkey_seam_remove_bindings(id);
            load_bindings(hotkey, data, removed, false);
        }
    }
    // SAFETY: taken above.
    unsafe { obs_hotkey_seam_unlock() };
}

/// `enum_load_bindings()`: `data` is the object's hotkey data.
///
/// # Safety
///
/// `data` is a live `obs_data_t`; `hotkey` is a live hotkey; the caller holds
/// the hotkey mutex.
unsafe extern "C" fn enum_load_bindings(data: *mut c_void, hotkey: *mut obs_hotkey_t) -> bool {
    // SAFETY: per the caller; getters on a live hotkey and a live obs_data.
    let hotkey_data = unsafe { obs_data_get_array(data, obs_hotkey_get_name(hotkey)) };
    if hotkey_data.is_null() {
        return true;
    }
    // SAFETY: per the caller; the array reference is released here.
    unsafe {
        load_bindings(hotkey, hotkey_data, false, true);
        obs_data_array_release(hotkey_data);
    }
    true
}

/// The body of the four `obs_hotkeys_load_*` functions.
///
/// # Safety
///
/// `object` and `hotkeys` are live, and `context` returns the object's context.
unsafe fn load_object(
    object: *mut c_void,
    hotkeys: *mut c_void,
    context: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
) {
    if object.is_null() || hotkeys.is_null() {
        return;
    }
    // SAFETY: no precondition.
    if !unsafe { obs_hotkey_seam_lock() } {
        return;
    }
    // SAFETY: a live object and live data, per the caller; the lock is held.
    unsafe {
        obs_hotkey_seam_enum_context(context(object), enum_load_bindings, hotkeys);
        obs_hotkey_seam_unlock();
    }
}

/// `obs_hotkeys_load_encoder()`.
///
/// # Safety
///
/// `encoder` and `hotkeys` are null or live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkeys_load_encoder(encoder: *mut c_void, hotkeys: *mut c_void) {
    // SAFETY: per the caller.
    unsafe { load_object(encoder, hotkeys, obs_hotkey_seam_encoder_context) };
}

/// `obs_hotkeys_load_output()`.
///
/// # Safety
///
/// `output` and `hotkeys` are null or live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkeys_load_output(output: *mut c_void, hotkeys: *mut c_void) {
    // SAFETY: per the caller.
    unsafe { load_object(output, hotkeys, obs_hotkey_seam_output_context) };
}

/// `obs_hotkeys_load_service()`.
///
/// # Safety
///
/// `service` and `hotkeys` are null or live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkeys_load_service(service: *mut c_void, hotkeys: *mut c_void) {
    // SAFETY: per the caller.
    unsafe { load_object(service, hotkeys, obs_hotkey_seam_service_context) };
}

/// `obs_hotkeys_load_source()`.
///
/// # Safety
///
/// `source` and `hotkeys` are null or live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkeys_load_source(source: *mut c_void, hotkeys: *mut c_void) {
    // SAFETY: per the caller.
    unsafe { load_object(source, hotkeys, obs_hotkey_seam_source_context) };
}

/// `obs_hotkey_pair_load()`.
///
/// # Safety
///
/// `data0` and `data1` are null or live `obs_data_array_t`s.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkey_pair_load(id: usize, data0: *mut c_void, data1: *mut c_void) {
    if data0.is_null() && data1.is_null() {
        return;
    }
    // SAFETY: no precondition.
    if !unsafe { obs_hotkey_seam_lock() } {
        return;
    }
    // SAFETY: the lock is held.
    let pair = unsafe { obs_hotkey_seam_find_pair(id) };
    if !pair.is_null() {
        // SAFETY: a live pair; the lock is held. Both halves are looked up
        // before either is loaded, as in the C.
        unsafe {
            let id0 = obs_hotkey_seam_pair_id(pair, 0);
            let p1 = obs_hotkey_seam_find(id0);
            let id1 = obs_hotkey_seam_pair_id(pair, 1);
            let p2 = obs_hotkey_seam_find(id1);
            if !p1.is_null() {
                let id0 = obs_hotkey_seam_pair_id(pair, 0);
                let removed = obs_hotkey_seam_remove_bindings(id0);
                load_bindings(p1, data0, removed, false);
            }
            if !p2.is_null() {
                let id1 = obs_hotkey_seam_pair_id(pair, 1);
                let removed = obs_hotkey_seam_remove_bindings(id1);
                load_bindings(p2, data1, removed, false);
            }
        }
    }
    // SAFETY: taken above.
    unsafe { obs_hotkey_seam_unlock() };
}

/// `struct save_bindings_helper_t`.
struct SaveBindings {
    array: *mut c_void,
    hotkey: *mut obs_hotkey_t,
    saved: bool,
}

/// `save_bindings_helper()`.
///
/// # Safety
///
/// `data` is a live [`SaveBindings`]; `binding` is a live binding; the caller
/// holds the hotkey mutex.
unsafe extern "C" fn save_bindings_helper(
    data: *mut c_void,
    _idx: usize,
    binding: *mut obs_hotkey_binding,
) -> bool {
    // SAFETY: per the caller.
    let h = unsafe { &*data.cast::<SaveBindings>() };
    // SAFETY: getters on a live hotkey and a live binding.
    if unsafe { obs_hotkey_get_id(h.hotkey) != obs_hotkey_binding_get_hotkey_id(binding) } {
        return true;
    }
    // SAFETY: the new obs_data is pushed, which takes its own reference, and
    // released; the array is live; a live binding; a static name.
    unsafe {
        let hotkey = obs_data_create();
        let combo = obs_hotkey_binding_get_key_combination(binding);
        obs_hotkey_combination_to_data(combo, hotkey);
        if core::save_marks_fixed(h.saved, combo.key) {
            obs_data_set_bool(hotkey, core::VOLUME_KEYS_FIXED.as_ptr(), true);
        }
        obs_data_array_push_back(h.array, hotkey);
        obs_data_release(hotkey);
    }
    true
}

/// `save_hotkey()`.
///
/// # Safety
///
/// `hotkey` is a live hotkey; the caller holds the hotkey mutex.
unsafe fn save_hotkey(hotkey: *mut obs_hotkey_t, saved: bool) -> *mut c_void {
    // SAFETY: no precondition.
    let array = unsafe { obs_data_array_create() };
    let mut arg = SaveBindings {
        array,
        hotkey,
        saved,
    };
    // SAFETY: `arg` outlives the call, which uses it only through the helper;
    // the lock is held, per the caller.
    unsafe { obs_hotkey_seam_enum_bindings(save_bindings_helper, (&raw mut arg).cast()) };
    array
}

/// `obs_hotkey_save()`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_hotkey_save(id: usize) -> *mut c_void {
    let mut result = std::ptr::null_mut();
    // SAFETY: no precondition.
    if !unsafe { obs_hotkey_seam_lock() } {
        return result;
    }
    // SAFETY: the lock is held.
    let hotkey = unsafe { obs_hotkey_seam_find(id) };
    if !hotkey.is_null() {
        // SAFETY: a live hotkey; the lock is held.
        result = unsafe { save_hotkey(hotkey, false) };
    }
    // SAFETY: taken above.
    unsafe { obs_hotkey_seam_unlock() };
    result
}

/// `obs_hotkey_pair_save()`.
///
/// # Safety
///
/// `p_data0` and `p_data1` are null or valid for a write.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkey_pair_save(
    id: usize,
    p_data0: *mut *mut c_void,
    p_data1: *mut *mut c_void,
) {
    if p_data0.is_null() && p_data1.is_null() {
        return;
    }
    // SAFETY: no precondition.
    if !unsafe { obs_hotkey_seam_lock() } {
        return;
    }
    // SAFETY: the lock is held.
    let pair = unsafe { obs_hotkey_seam_find_pair(id) };
    if !pair.is_null() {
        for (half, out) in [p_data0, p_data1].into_iter().enumerate() {
            if out.is_null() {
                continue;
            }
            // SAFETY: a live pair; the lock is held.
            let hotkey = unsafe { obs_hotkey_seam_find(obs_hotkey_seam_pair_id(pair, half)) };
            if !hotkey.is_null() {
                // SAFETY: a live hotkey; the lock is held; `out` is writable,
                // per the caller.
                unsafe { out.write(save_hotkey(hotkey, false)) };
            }
        }
    }
    // SAFETY: taken above.
    unsafe { obs_hotkey_seam_unlock() };
}

/// `enum_save_hotkey()`: `data` is the object's new hotkey data.
///
/// # Safety
///
/// `data` is a live `obs_data_t`; `hotkey` is a live hotkey; the caller holds
/// the hotkey mutex.
unsafe extern "C" fn enum_save_hotkey(data: *mut c_void, hotkey: *mut obs_hotkey_t) -> bool {
    // SAFETY: per the caller; `obs_data_set_array` takes its own reference.
    unsafe {
        let hotkey_data = save_hotkey(hotkey, true);
        obs_data_set_array(data, obs_hotkey_get_name(hotkey), hotkey_data);
        obs_data_array_release(hotkey_data);
    }
    true
}

/// The body of the four `obs_hotkeys_save_*` functions.
///
/// # Safety
///
/// `object` is live, and `context` returns its context.
unsafe fn save_object(
    object: *mut c_void,
    context: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
) -> *mut c_void {
    let mut result = std::ptr::null_mut();
    // SAFETY: no precondition.
    if !unsafe { obs_hotkey_seam_lock() } {
        return result;
    }
    // SAFETY: a live object, per the caller; the lock is held.
    let context = unsafe { context(object) };
    // SAFETY: a live context; the lock is held.
    if unsafe { obs_hotkey_seam_context_count(context) } != 0 {
        // SAFETY: as above; the callback gets the new obs_data.
        unsafe {
            result = obs_data_create();
            obs_hotkey_seam_enum_context(context, enum_save_hotkey, result);
        }
    }
    // SAFETY: taken above.
    unsafe { obs_hotkey_seam_unlock() };
    result
}

/// `obs_hotkeys_save_encoder()`.
///
/// # Safety
///
/// `encoder` is live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkeys_save_encoder(encoder: *mut c_void) -> *mut c_void {
    // SAFETY: per the caller.
    unsafe { save_object(encoder, obs_hotkey_seam_encoder_context) }
}

/// `obs_hotkeys_save_output()`.
///
/// # Safety
///
/// `output` is live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkeys_save_output(output: *mut c_void) -> *mut c_void {
    // SAFETY: per the caller.
    unsafe { save_object(output, obs_hotkey_seam_output_context) }
}

/// `obs_hotkeys_save_service()`.
///
/// # Safety
///
/// `service` is live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkeys_save_service(service: *mut c_void) -> *mut c_void {
    // SAFETY: per the caller.
    unsafe { save_object(service, obs_hotkey_seam_service_context) }
}

/// `obs_hotkeys_save_source()`.
///
/// # Safety
///
/// `source` is live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkeys_save_source(source: *mut c_void) -> *mut c_void {
    // SAFETY: per the caller.
    unsafe { save_object(source, obs_hotkey_seam_source_context) }
}
