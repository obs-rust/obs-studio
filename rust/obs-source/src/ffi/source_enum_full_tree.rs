//! C ABI of `obs_source_enum_full_tree()` and the hidden
//! `obs_source_active_child_enum_full_tree()` (its walk with a stop source,
//! for `obs_source_add_active_child()`). The source fields and the plugin's
//! `enum_all_sources` / `enum_active_sources` calls go through
//! `obs-source-enum-full-tree-fields.c.h` (no `EXPORT`);
//! `obs_transition_enum_sources()`, `obs_source_get_ref()` and
//! `obs_source_release()` are called directly.
use crate::enum_full_tree;
use core::ffi::{CStr, c_char, c_int, c_void};
use core::ptr::NonNull;

/// `obs_source_enum_proc_t`.
pub type EnumProc =
    unsafe extern "C" fn(parent: *mut c_void, child: *mut c_void, param: *mut c_void);

unsafe extern "C" {
    fn obs_source_enum_full_tree_has_data(source: *const c_void) -> bool;
    fn obs_source_enum_full_tree_is_transition(source: *const c_void) -> bool;
    fn obs_source_enum_full_tree_has_enum_all(source: *const c_void) -> bool;
    fn obs_source_enum_full_tree_has_enum_active(source: *const c_void) -> bool;
    fn obs_source_enum_full_tree_call_enum_all(
        source: *mut c_void,
        enum_callback: EnumProc,
        param: *mut c_void,
    );
    fn obs_source_enum_full_tree_call_enum_active(
        source: *mut c_void,
        enum_callback: EnumProc,
        param: *mut c_void,
    );
    fn obs_transition_enum_sources(
        source: *mut c_void,
        enum_callback: EnumProc,
        param: *mut c_void,
    );
    fn obs_source_get_ref(source: *mut c_void) -> *mut c_void;
    fn obs_source_release(source: *mut c_void);
    fn blog(level: c_int, format: *const c_char, ...);
}

/// `LOG_DEBUG`, the level of `obs_ptr_valid()`'s NULL line.
const LOG_DEBUG: c_int = 400;

/// The walk's callback with its parameter, as handed to the C.
#[derive(Debug, Clone, Copy)]
pub struct Sink {
    /// The callback.
    pub callback: EnumProc,
    /// Its parameter.
    pub param: *mut c_void,
}

/// The walk's parameter (`struct source_enum_data`): the caller's callback
/// and parameter and the stop source (NULL for the public walk).
#[repr(C)]
struct TreeData {
    enum_callback: EnumProc,
    param: *mut c_void,
    stop_source: *mut c_void,
}

/// The calls the walk makes by symbol; the fields and the plugin calls go
/// through the production adapters in [`With`].
pub trait Calls: Default {
    /// `obs_transition_enum_sources()`.
    ///
    /// # Safety
    /// A live transition and a callback that takes `param`.
    unsafe fn transition_enum(source: *mut c_void, callback: EnumProc, param: *mut c_void);
    /// `obs_source_get_ref()`.
    ///
    /// # Safety
    /// A live source.
    unsafe fn get_ref(source: *mut c_void) -> *mut c_void;
    /// `obs_source_release()`.
    ///
    /// # Safety
    /// A reference taken by [`Calls::get_ref`].
    unsafe fn release(source: *mut c_void);
}

/// What the shims reach: libobs's C. Each source is non-NULL and, under the
/// libobs contract, live for the call.
#[derive(Debug, Default, Clone, Copy)]
pub struct Libobs;

impl Calls for Libobs {
    unsafe fn transition_enum(source: *mut c_void, callback: EnumProc, param: *mut c_void) {
        // SAFETY: the caller's contract.
        unsafe { obs_transition_enum_sources(source, callback, param) }
    }

    unsafe fn get_ref(source: *mut c_void) -> *mut c_void {
        // SAFETY: the caller's contract.
        unsafe { obs_source_get_ref(source) }
    }

    unsafe fn release(source: *mut c_void) {
        // SAFETY: the caller's contract.
        unsafe { obs_source_release(source) }
    }
}

/// [`enum_full_tree::Libobs`] over the production field adapters and `C`'s
/// calls.
#[derive(Debug, Default, Clone, Copy)]
pub struct With<C>(pub C);

impl<C: Calls> enum_full_tree::Libobs for With<C> {
    type Source = NonNull<c_void>;
    type Sink = Sink;

    fn has_data(&mut self, source: Self::Source) -> bool {
        // SAFETY: a live source; the adapter reads one field.
        unsafe { obs_source_enum_full_tree_has_data(source.as_ptr()) }
    }

    fn is_transition(&mut self, source: Self::Source) -> bool {
        // SAFETY: as `has_data`.
        unsafe { obs_source_enum_full_tree_is_transition(source.as_ptr()) }
    }

    fn has_enum_all(&mut self, source: Self::Source) -> bool {
        // SAFETY: as `has_data`.
        unsafe { obs_source_enum_full_tree_has_enum_all(source.as_ptr()) }
    }

    fn has_enum_active(&mut self, source: Self::Source) -> bool {
        // SAFETY: as `has_data`.
        unsafe { obs_source_enum_full_tree_has_enum_active(source.as_ptr()) }
    }

    fn get_ref(&mut self, source: Self::Source) -> Option<Self::Source> {
        // SAFETY: a live source.
        NonNull::new(unsafe { C::get_ref(source.as_ptr()) })
    }

    fn release(&mut self, source: Self::Source) {
        // SAFETY: the reference `get_ref` took.
        unsafe { C::release(source.as_ptr()) }
    }

    fn transition_enum(&mut self, source: Self::Source, sink: Sink) {
        // SAFETY: a live transition; the sink's callback takes its param.
        unsafe { C::transition_enum(source.as_ptr(), sink.callback, sink.param) }
    }

    fn enum_all(&mut self, source: Self::Source, sink: Sink) {
        // SAFETY: a live source with the callback and data (the core
        // checked both); the sink's callback takes its param.
        unsafe {
            obs_source_enum_full_tree_call_enum_all(source.as_ptr(), sink.callback, sink.param)
        }
    }

    fn enum_active(&mut self, source: Self::Source, sink: Sink) {
        // SAFETY: as `enum_all`.
        unsafe {
            obs_source_enum_full_tree_call_enum_active(source.as_ptr(), sink.callback, sink.param);
        }
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

/// `enum_source_full_tree_callback()`.
///
/// # Safety
/// `param` is the [`TreeData`] of a running walk; `child` is NULL, the stop
/// source or live.
unsafe extern "C" fn tree_callback<C: Calls>(
    parent: *mut c_void,
    child: *mut c_void,
    param: *mut c_void,
) {
    // SAFETY: `param` is the walk's `TreeData`, alive until it returns and
    // only read here.
    let (enum_callback, user_param, stop_source) = {
        let data = unsafe { &*param.cast::<TreeData>() };
        (data.enum_callback, data.param, data.stop_source)
    };
    if enum_full_tree::descends(NonNull::new(child), NonNull::new(stop_source))
        && let Some(child_source) = NonNull::new(child)
    {
        let sink = Sink {
            callback: tree_callback::<C>,
            param,
        };
        enum_full_tree::tree_edge(&mut With(C::default()), child_source, sink);
    }
    // SAFETY: the caller's callback with its own parameter.
    unsafe { enum_callback(parent, child, user_param) }
}

/// `enum_source_full_tree()` over `C`'s calls (the exports use [`Libobs`];
/// the parity test passes stand-ins).
///
/// # Safety
/// `source` is NULL or live; `enum_callback` takes `param`; `stop_source` is
/// NULL or a source compared by address only.
pub unsafe fn enum_full_tree_with<C: Calls>(
    source: *mut c_void,
    enum_callback: EnumProc,
    param: *mut c_void,
    stop_source: *mut c_void,
) {
    let mut data = TreeData {
        enum_callback,
        param,
        stop_source,
    };
    let sink = Sink {
        callback: tree_callback::<C>,
        param: (&raw mut data).cast(),
    };
    enum_full_tree::enumerate(&mut With(C::default()), NonNull::new(source), sink);
}

/// # Safety
/// `source` is NULL or live under the original libobs ownership contract;
/// `enum_callback` takes `param`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_enum_full_tree(
    source: *mut c_void,
    enum_callback: EnumProc,
    param: *mut c_void,
) {
    // SAFETY: the caller's contract; no stop source.
    unsafe { enum_full_tree_with::<Libobs>(source, enum_callback, param, core::ptr::null_mut()) }
}

/// The walk with a stop source, for `obs_source_add_active_child()`. Hidden:
/// no `EXPORT`.
///
/// # Safety
/// As [`obs_source_enum_full_tree`]; `stop_source` is compared by address
/// only.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_active_child_enum_full_tree(
    source: *mut c_void,
    enum_callback: EnumProc,
    param: *mut c_void,
    stop_source: *mut c_void,
) {
    // SAFETY: the caller's contract.
    unsafe { enum_full_tree_with::<Libobs>(source, enum_callback, param, stop_source) }
}
