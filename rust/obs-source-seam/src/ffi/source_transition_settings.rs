//! C ABI for `libobs/obs-source-transition-settings.c.h`, the settings of a
//! transition: `obs_transition_is_active()`, the scale type, alignment and
//! size setters and getters, `obs_transition_save()` and the fixed duration
//! switch. Nine are `EXPORT` through `obs.h` (`rust-exports.d`);
//! `obs_transition_save()` is declared in `obs-internal.h` without it and stays
//! hidden (`rust-hidden.d`).
//!
//! `struct obs_source` is never mirrored. Every read and write of a
//! transition field goes through the transition seam
//! (`libobs/obs-source-transition-seam.h`, still C in `libobs.core-source`):
//! `transition_valid()`, `transition_active()` and `valid_scale_type()`,
//! one accessor per field, the active child's `context.name`
//! (`obs_source_transition_seam_get_child_name()`), and the lock and unlock
//! of `transition_mutex`. The locking is the C's: the setters and getters
//! touch the fields without the lock, and the save reads the active child,
//! then its name and each saved field just before writing it, all between one
//! lock and one unlock, with `obs_data_set_string()` and `obs_data_set_int()` in the C's
//! order. The active index is [`obs_source_transition_start_active_index`],
//! as in the C. The safe core
//! ([`crate::transition_settings`]) gives the log names, the invalid-source
//! answers and the save record.
//!
//! As in the C, `obs_transition_is_active()` and `obs_transition_save()` do
//! not check their source, and `obs_transition_get_size()` writes both
//! outputs without checking them.
//!
//! The enums cross as `int`, as libobs passes them (`enum
//! obs_transition_scale_type` and `enum obs_transition_mode` are `int`-sized,
//! checked in `obs-source-transition.c.h`).
//!
//! One intentional difference from the C: `obs_transition_get_size()` hands
//! `transition_valid()` its own name, so the debug line for a NULL source
//! names the getter (the C names `obs_transition_set_size`).

use std::ffi::{CStr, c_char, c_int, c_void};
use std::ptr;

use crate::ffi::source_transition_start::obs_source_transition_start_active_index;
use crate::transition_settings::{self as core, Child, Func, SaveReads, Settings, Value};

unsafe extern "C" {
    fn transition_valid(transition: *const c_void, func: *const c_char) -> bool;
    fn transition_active(transition: *mut c_void) -> bool;
    fn valid_scale_type(value: i64) -> c_int;

    fn obs_source_transition_seam_get_sources(transition: *const c_void, idx: usize)
    -> *mut c_void;
    fn obs_source_transition_seam_get_child_name(child: *const c_void) -> *const c_char;
    fn obs_source_transition_seam_get_mode(transition: *const c_void) -> c_int;
    fn obs_source_transition_seam_get_scale_type(transition: *const c_void) -> c_int;
    fn obs_source_transition_seam_set_scale_type(transition: *mut c_void, value: c_int);
    fn obs_source_transition_seam_get_alignment(transition: *const c_void) -> u32;
    fn obs_source_transition_seam_set_alignment(transition: *mut c_void, value: u32);
    fn obs_source_transition_seam_get_cx(transition: *const c_void) -> u32;
    fn obs_source_transition_seam_set_cx(transition: *mut c_void, value: u32);
    fn obs_source_transition_seam_get_cy(transition: *const c_void) -> u32;
    fn obs_source_transition_seam_set_cy(transition: *mut c_void, value: u32);
    fn obs_source_transition_seam_get_use_fixed_duration(transition: *const c_void) -> bool;
    fn obs_source_transition_seam_set_use_fixed_duration(transition: *mut c_void, value: bool);
    fn obs_source_transition_seam_set_fixed_duration(transition: *mut c_void, value: u32);
    fn obs_source_transition_seam_lock(transition: *mut c_void) -> c_int;
    fn obs_source_transition_seam_unlock(transition: *mut c_void) -> c_int;

    fn obs_data_set_string(data: *mut c_void, name: *const c_char, val: *const c_char);
    fn obs_data_set_int(data: *mut c_void, name: *const c_char, val: i64);
}

/// `transition_valid(transition, <func's name>)`.
///
/// # Safety
/// `transition` is NULL or a live `obs_source_t`.
unsafe fn valid(transition: *const c_void, func: Func) -> bool {
    // SAFETY: the caller's pointer, and a NUL-terminated static name.
    unsafe { transition_valid(transition, core::log_name(func).as_ptr()) }
}

/// `bool obs_transition_is_active(obs_source_t *transition)`.
///
/// # Safety
/// `transition` is a live `obs_source_t` (the C does not check it).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_transition_is_active(transition: *mut c_void) -> bool {
    // SAFETY: the caller's live source.
    unsafe { transition_active(transition) }
}

/// `void obs_transition_set_scale_type(obs_source_t *transition, enum
/// obs_transition_scale_type type)`.
///
/// # Safety
/// `transition` is NULL or a live `obs_source_t`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_transition_set_scale_type(transition: *mut c_void, scale_type: c_int) {
    // SAFETY: the caller's pointer; written only once it is a transition.
    unsafe {
        if !valid(transition, Func::SetScaleType) {
            return;
        }
        obs_source_transition_seam_set_scale_type(
            transition,
            valid_scale_type(i64::from(scale_type)),
        );
    }
}

/// `enum obs_transition_scale_type obs_transition_get_scale_type(const
/// obs_source_t *transition)`.
///
/// # Safety
/// `transition` is NULL or a live `obs_source_t`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_transition_get_scale_type(transition: *const c_void) -> c_int {
    // SAFETY: the caller's pointer; read only once it is a transition.
    unsafe {
        if valid(transition, Func::GetScaleType) {
            obs_source_transition_seam_get_scale_type(transition)
        } else {
            Settings::INVALID.scale_type
        }
    }
}

/// `void obs_transition_set_alignment(obs_source_t *transition, uint32_t
/// alignment)`.
///
/// # Safety
/// `transition` is NULL or a live `obs_source_t`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_transition_set_alignment(transition: *mut c_void, alignment: u32) {
    // SAFETY: the caller's pointer; written only once it is a transition.
    unsafe {
        if !valid(transition, Func::SetAlignment) {
            return;
        }
        obs_source_transition_seam_set_alignment(transition, alignment);
    }
}

/// `uint32_t obs_transition_get_alignment(const obs_source_t *transition)`.
///
/// # Safety
/// `transition` is NULL or a live `obs_source_t`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_transition_get_alignment(transition: *const c_void) -> u32 {
    // SAFETY: the caller's pointer; read only once it is a transition.
    unsafe {
        if valid(transition, Func::GetAlignment) {
            obs_source_transition_seam_get_alignment(transition)
        } else {
            Settings::INVALID.alignment
        }
    }
}

/// `void obs_transition_set_size(obs_source_t *transition, uint32_t cx,
/// uint32_t cy)`.
///
/// # Safety
/// `transition` is NULL or a live `obs_source_t`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_transition_set_size(transition: *mut c_void, cx: u32, cy: u32) {
    // SAFETY: the caller's pointer; written only once it is a transition.
    unsafe {
        if !valid(transition, Func::SetSize) {
            return;
        }
        obs_source_transition_seam_set_cx(transition, cx);
        obs_source_transition_seam_set_cy(transition, cy);
    }
}

/// `void obs_transition_get_size(const obs_source_t *transition, uint32_t
/// *cx, uint32_t *cy)`.
///
/// # Safety
/// `transition` is NULL or a live `obs_source_t`; `cx` and `cy` are
/// writable (the C writes both without checking them).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_transition_get_size(
    transition: *const c_void,
    cx: *mut u32,
    cy: *mut u32,
) {
    // SAFETY: the caller's pointers; the fields are read only once it is a
    // transition.
    unsafe {
        if !valid(transition, Func::GetSize) {
            *cx = Settings::INVALID.cx;
            *cy = Settings::INVALID.cy;
            return;
        }
        *cx = obs_source_transition_seam_get_cx(transition);
        *cy = obs_source_transition_seam_get_cy(transition);
    }
}

/// The save's reads of a locked transition `tr` whose active child is
/// `child`, each one seam call.
struct Reads {
    tr: *mut c_void,
    child: *mut c_void,
}

// SAFETY (every method): `tr` is the live transition `obs_transition_save()`
// holds locked while `save_each` runs, and `child` NULL or its live child.
impl<'a> SaveReads<'a> for Reads {
    fn source_a(&mut self) -> Child<'a> {
        if self.child.is_null() {
            return Child::Empty;
        }
        // SAFETY: a live child; its name lives while the lock is held and is
        // written before the unlock.
        let name = unsafe { obs_source_transition_seam_get_child_name(self.child) };
        if name.is_null() {
            Child::Unnamed
        } else {
            // SAFETY: a NUL-terminated name, as libobs keeps it.
            Child::Named(unsafe { CStr::from_ptr(name) })
        }
    }
    fn alignment(&mut self) -> u32 {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_get_alignment(self.tr) }
    }
    fn mode(&mut self) -> i32 {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_get_mode(self.tr) }
    }
    fn scale_type(&mut self) -> i32 {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_get_scale_type(self.tr) }
    }
    fn cx(&mut self) -> u32 {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_get_cx(self.tr) }
    }
    fn cy(&mut self) -> u32 {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_get_cy(self.tr) }
    }
}

/// `void obs_transition_save(obs_source_t *tr, obs_data_t *data)`.
///
/// # Safety
/// `tr` is a live transition (the C does not check it) and `data` a live
/// `obs_data_t`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_transition_save(tr: *mut c_void, data: *mut c_void) {
    // SAFETY: the caller's live transition and data; the child and its name
    // are read under the transition's lock, as the C does, and the name is
    // used before the unlock.
    unsafe {
        obs_source_transition_seam_lock(tr);
        let child = obs_source_transition_seam_get_sources(
            tr,
            obs_source_transition_start_active_index(transition_active(tr)),
        );
        let mut reads = Reads { tr, child };
        core::save_each(&mut reads, |key, value| match value {
            Value::Str(s) => obs_data_set_string(data, key.as_ptr(), s.as_ptr()),
            Value::Null => obs_data_set_string(data, key.as_ptr(), ptr::null()),
            Value::Int(n) => obs_data_set_int(data, key.as_ptr(), n),
        });
        obs_source_transition_seam_unlock(tr);
    }
}

/// `void obs_transition_enable_fixed(obs_source_t *transition, bool enable,
/// uint32_t duration)`.
///
/// # Safety
/// `transition` is NULL or a live `obs_source_t`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_transition_enable_fixed(
    transition: *mut c_void,
    enable: bool,
    duration: u32,
) {
    // SAFETY: the caller's pointer; written only once it is a transition.
    unsafe {
        if !valid(transition, Func::EnableFixed) {
            return;
        }
        obs_source_transition_seam_set_use_fixed_duration(transition, enable);
        obs_source_transition_seam_set_fixed_duration(transition, duration);
    }
}

/// `bool obs_transition_fixed(obs_source_t *transition)`.
///
/// # Safety
/// `transition` is NULL or a live `obs_source_t`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_transition_fixed(transition: *mut c_void) -> bool {
    // SAFETY: the caller's pointer; read only once it is a transition.
    unsafe {
        if valid(transition, Func::Fixed) {
            obs_source_transition_seam_get_use_fixed_duration(transition)
        } else {
            Settings::INVALID.use_fixed_duration
        }
    }
}
