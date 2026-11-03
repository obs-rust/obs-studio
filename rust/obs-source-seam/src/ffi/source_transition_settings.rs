//! C ABI for `libobs/obs-source-transition-settings.c.h`, the settings of a
//! transition: `obs_transition_is_active()`, the scale type, alignment and
//! size setters and getters, `obs_transition_save()` and the fixed duration
//! switch. Nine are `EXPORT` through `obs.h` (`rust-exports.d`);
//! `obs_transition_save()` is declared in `obs-internal.h` without it and stays
//! hidden (`rust-hidden.d`).
//!
//! The functions are the safe core's (`obs_source::transition_settings`),
//! run on [`Seam`], which implements its `Transition` over the transition
//! seam (`libobs/obs-source-transition-seam.h`, still C in
//! `libobs.core-source`): `transition_valid()`, `transition_active()`, one
//! accessor per field, the child's `context.name` and the lock and unlock of
//! `transition_mutex`, one seam call per method. `struct obs_source` is
//! never mirrored. The save writes through [`Data`], `obs_data_set_string()`
//! and `obs_data_set_int()`. The locking is the C's: the setters and getters
//! touch the fields without the lock, and the save finds the active child,
//! then reads its name and each saved field just before writing it, all
//! between one lock and one unlock.
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

use obs_source::transition_settings::{self as core, SaveData, Transition};

unsafe extern "C" {
    fn transition_valid(transition: *const c_void, func: *const c_char) -> bool;
    fn transition_active(transition: *mut c_void) -> bool;

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

/// A non-NULL child of a transition, read from one of its slots.
#[derive(Clone, Copy)]
pub struct ChildSource(*mut c_void);

/// A live, non-NULL `obs_source_t` a shim was handed. NULL never becomes a
/// `Seam`: [`source`] and [`source_mut`] give `None` for it.
pub struct Seam(*mut c_void);

// SAFETY (every method): `self.0` is the live source the C caller passed,
// and each call is the one seam accessor or lock call that names it.
impl Transition for Seam {
    type Child = ChildSource;

    fn valid(source: Option<&Self>, func: &'static CStr) -> bool {
        let ptr = source.map_or(ptr::null(), |s| s.0.cast_const());
        // SAFETY: NULL or the caller's live source, and a static name.
        unsafe { transition_valid(ptr, func.as_ptr()) }
    }
    fn active(&self) -> bool {
        // SAFETY: see the impl.
        unsafe { transition_active(self.0) }
    }
    fn source(&self, idx: usize) -> Option<Self::Child> {
        // SAFETY: see the impl; idx is 0 or 1 (`active_index`).
        let child = unsafe { obs_source_transition_seam_get_sources(self.0, idx) };
        (!child.is_null()).then_some(ChildSource(child))
    }
    fn child_name(&self, child: Self::Child) -> Option<&CStr> {
        // SAFETY: a live child of the locked transition; its name lives
        // while the lock is held, and the save writes it before the unlock.
        let name = unsafe { obs_source_transition_seam_get_child_name(child.0) };
        // SAFETY: a NUL-terminated name, as libobs keeps it.
        (!name.is_null()).then(|| unsafe { CStr::from_ptr(name) })
    }
    fn mode(&self) -> i32 {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_get_mode(self.0) }
    }
    fn scale_type(&self) -> i32 {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_get_scale_type(self.0) }
    }
    fn set_scale_type(&mut self, value: i32) {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_set_scale_type(self.0, value) }
    }
    fn alignment(&self) -> u32 {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_get_alignment(self.0) }
    }
    fn set_alignment(&mut self, value: u32) {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_set_alignment(self.0, value) }
    }
    fn cx(&self) -> u32 {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_get_cx(self.0) }
    }
    fn set_cx(&mut self, value: u32) {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_set_cx(self.0, value) }
    }
    fn cy(&self) -> u32 {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_get_cy(self.0) }
    }
    fn set_cy(&mut self, value: u32) {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_set_cy(self.0, value) }
    }
    fn use_fixed_duration(&self) -> bool {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_get_use_fixed_duration(self.0) }
    }
    fn set_use_fixed_duration(&mut self, value: bool) {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_set_use_fixed_duration(self.0, value) }
    }
    fn set_fixed_duration(&mut self, value: u32) {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_set_fixed_duration(self.0, value) }
    }
    fn lock(&self) {
        // SAFETY: see the impl. The C ignores the result too.
        unsafe { obs_source_transition_seam_lock(self.0) };
    }
    fn unlock(&self) {
        // SAFETY: see the impl. The C ignores the result too.
        unsafe { obs_source_transition_seam_unlock(self.0) };
    }
}

/// A live `obs_data_t` the save writes to.
pub struct Data(*mut c_void);

impl SaveData for Data {
    fn set_string(&mut self, key: &'static CStr, value: Option<&CStr>) {
        let value = value.map_or(ptr::null(), CStr::as_ptr);
        // SAFETY: the caller's live data, a static key and NULL or a name
        // that outlives the call.
        unsafe { obs_data_set_string(self.0, key.as_ptr(), value) }
    }
    fn set_int(&mut self, key: &'static CStr, value: i64) {
        // SAFETY: the caller's live data and a static key.
        unsafe { obs_data_set_int(self.0, key.as_ptr(), value) }
    }
}

/// `Some` for a non-NULL source pointer.
fn source(transition: *const c_void) -> Option<Seam> {
    (!transition.is_null()).then(|| Seam(transition.cast_mut()))
}

/// `bool obs_transition_is_active(obs_source_t *transition)`.
///
/// # Safety
/// `transition` is a live `obs_source_t` (the C does not check it).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_transition_is_active(transition: *mut c_void) -> bool {
    core::is_active(&Seam(transition))
}

/// `void obs_transition_set_scale_type(obs_source_t *transition, enum
/// obs_transition_scale_type type)`.
///
/// # Safety
/// `transition` is NULL or a live `obs_source_t`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_transition_set_scale_type(transition: *mut c_void, scale_type: c_int) {
    core::set_scale_type(source(transition).as_mut(), scale_type);
}

/// `enum obs_transition_scale_type obs_transition_get_scale_type(const
/// obs_source_t *transition)`.
///
/// # Safety
/// `transition` is NULL or a live `obs_source_t`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_transition_get_scale_type(transition: *const c_void) -> c_int {
    core::get_scale_type(source(transition).as_ref())
}

/// `void obs_transition_set_alignment(obs_source_t *transition, uint32_t
/// alignment)`.
///
/// # Safety
/// `transition` is NULL or a live `obs_source_t`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_transition_set_alignment(transition: *mut c_void, alignment: u32) {
    core::set_alignment(source(transition).as_mut(), alignment);
}

/// `uint32_t obs_transition_get_alignment(const obs_source_t *transition)`.
///
/// # Safety
/// `transition` is NULL or a live `obs_source_t`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_transition_get_alignment(transition: *const c_void) -> u32 {
    core::get_alignment(source(transition).as_ref())
}

/// `void obs_transition_set_size(obs_source_t *transition, uint32_t cx,
/// uint32_t cy)`.
///
/// # Safety
/// `transition` is NULL or a live `obs_source_t`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_transition_set_size(transition: *mut c_void, cx: u32, cy: u32) {
    core::set_size(source(transition).as_mut(), cx, cy);
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
    let (width, height) = core::get_size(source(transition).as_ref());
    // SAFETY: the caller's outputs, written as the C writes them.
    unsafe {
        *cx = width;
        *cy = height;
    }
}

/// `void obs_transition_save(obs_source_t *tr, obs_data_t *data)`.
///
/// # Safety
/// `tr` is a live transition (the C does not check it) and `data` a live
/// `obs_data_t`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_transition_save(tr: *mut c_void, data: *mut c_void) {
    core::save_to(&Seam(tr), &mut Data(data));
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
    core::enable_fixed(source(transition).as_mut(), enable, duration);
}

/// `bool obs_transition_fixed(obs_source_t *transition)`.
///
/// # Safety
/// `transition` is NULL or a live `obs_source_t`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_transition_fixed(transition: *mut c_void) -> bool {
    core::fixed(source(transition).as_ref())
}
