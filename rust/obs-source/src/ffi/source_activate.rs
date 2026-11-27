//! C ABI of `obs_source_activate()` and `obs_source_deactivate()`, declared
//! in `obs-internal.h` without `EXPORT`, so both stay hidden
//! (`libobs/cmake/rust-hidden.d`). The counters stay behind the atomic
//! adapters of `obs-source-activate-fields.c.h`; the walk is the C
//! `obs_source_enum_active_tree()`, given the four tree callbacks here.
use crate::activate::{Counter, Counters, Tree, activate, deactivate, visit};
use core::ffi::{CStr, c_char, c_int, c_long, c_void};
use core::ptr::{NonNull, null_mut};

/// `obs_source_enum_proc_t`.
pub type EnumProc = Option<unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void)>;

unsafe extern "C" {
    fn obs_source_activate_inc_show_refs(source: *mut c_void);
    fn obs_source_activate_dec_show_refs(source: *mut c_void);
    fn obs_source_activate_load_show_refs(source: *const c_void) -> c_long;
    fn obs_source_activate_inc_activate_refs(source: *mut c_void);
    fn obs_source_activate_dec_activate_refs(source: *mut c_void);
    fn obs_source_activate_load_activate_refs(source: *const c_void) -> c_long;
    fn obs_source_enum_active_tree(source: *mut c_void, callback: EnumProc, param: *mut c_void);
    fn blog(level: c_int, format: *const c_char, ...);
}

/// `LOG_DEBUG`, the level of `obs_source_valid()`'s NULL line.
const LOG_DEBUG: c_int = 400;

/// libobs: each method is the one C step the original made.
#[derive(Clone, Copy, Debug, Default)]
pub struct Libobs;

impl Counters for Libobs {
    type Source = NonNull<c_void>;
    fn inc(&mut self, source: NonNull<c_void>, counter: Counter) {
        // SAFETY: a live source (the caller's, checked not NULL, or a child
        // the walk reports), whose counter the adapter changes atomically.
        unsafe {
            match counter {
                Counter::Show => obs_source_activate_inc_show_refs(source.as_ptr()),
                Counter::Activate => obs_source_activate_inc_activate_refs(source.as_ptr()),
            }
        }
    }
    fn dec(&mut self, source: NonNull<c_void>, counter: Counter) {
        // SAFETY: as in `inc`.
        unsafe {
            match counter {
                Counter::Show => obs_source_activate_dec_show_refs(source.as_ptr()),
                Counter::Activate => obs_source_activate_dec_activate_refs(source.as_ptr()),
            }
        }
    }
    fn load(&mut self, source: NonNull<c_void>, counter: Counter) -> c_long {
        // SAFETY: as in `inc`; the adapter only loads.
        unsafe {
            match counter {
                Counter::Show => obs_source_activate_load_show_refs(source.as_ptr()),
                Counter::Activate => obs_source_activate_load_activate_refs(source.as_ptr()),
            }
        }
    }
    fn walk(&mut self, source: NonNull<c_void>, tree: Tree) {
        let callback: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) = match tree {
            Tree::Show => show_tree,
            Tree::Activate => activate_tree,
            Tree::Hide => hide_tree,
            Tree::Deactivate => deactivate_tree,
        };
        // SAFETY: a live source; the callback reads no parameter.
        unsafe { obs_source_enum_active_tree(source.as_ptr(), Some(callback), null_mut()) }
    }
    fn log_null(&mut self, function: &'static CStr) {
        // SAFETY: static strings match the two %s arguments of the format.
        unsafe {
            blog(
                LOG_DEBUG,
                c"%s: Null '%s' parameter".as_ptr(),
                function.as_ptr(),
                c"source".as_ptr(),
            );
        }
    }
}

/// One child of the walk. The walk never reports a NULL child
/// (`obs_source_enum_active_tree()` reads each child's type before it calls
/// the callback), so a NULL one, which the original would dereference, does
/// nothing here.
fn callback(child: *mut c_void, tree: Tree) {
    if let Some(child) = NonNull::new(child) {
        visit(&mut Libobs, child, tree);
    }
}

/// `show_tree`.
///
/// # Safety
/// `child` is a live source, as `obs_source_enum_active_tree()` passes it.
pub unsafe extern "C" fn show_tree(_parent: *mut c_void, child: *mut c_void, _param: *mut c_void) {
    callback(child, Tree::Show);
}

/// `activate_tree`.
///
/// # Safety
/// As [`show_tree`].
pub unsafe extern "C" fn activate_tree(
    _parent: *mut c_void,
    child: *mut c_void,
    _param: *mut c_void,
) {
    callback(child, Tree::Activate);
}

/// `hide_tree`.
///
/// # Safety
/// As [`show_tree`].
pub unsafe extern "C" fn hide_tree(_parent: *mut c_void, child: *mut c_void, _param: *mut c_void) {
    callback(child, Tree::Hide);
}

/// `deactivate_tree`.
///
/// # Safety
/// As [`show_tree`].
pub unsafe extern "C" fn deactivate_tree(
    _parent: *mut c_void,
    child: *mut c_void,
    _param: *mut c_void,
) {
    callback(child, Tree::Deactivate);
}

/// `obs_source_activate()` (hidden).
///
/// # Safety
/// A non-null source must be live under the original libobs ownership contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_activate(source: *mut c_void, view_type: u32) {
    activate(&mut Libobs, NonNull::new(source), view_type);
}

/// `obs_source_deactivate()` (hidden).
///
/// # Safety
/// A non-null source must be live under the original libobs ownership contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_deactivate(source: *mut c_void, view_type: u32) {
    deactivate(&mut Libobs, NonNull::new(source), view_type);
}
