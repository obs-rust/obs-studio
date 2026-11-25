//! C ABI of `obs_source_enum_active_sources()` and
//! `obs_source_enum_active_tree()`. The source fields and the plugin's
//! `enum_active_sources` call go through
//! `obs-source-enum-active-fields.c.h` (no `EXPORT`);
//! `obs_transition_enum_sources()`, `obs_source_get_ref()` and
//! `obs_source_release()` are called directly.
use crate::enum_active;
use core::ffi::{CStr, c_char, c_int, c_void};
use core::ptr::NonNull;

/// `obs_source_enum_proc_t`.
pub type EnumProc =
    unsafe extern "C" fn(parent: *mut c_void, child: *mut c_void, param: *mut c_void);

unsafe extern "C" {
    fn obs_source_enum_active_has_data(source: *const c_void) -> bool;
    fn obs_source_enum_active_is_transition(source: *const c_void) -> bool;
    fn obs_source_enum_active_has_callback(source: *const c_void) -> bool;
    fn obs_source_enum_active_call(
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

/// A callback with its parameter, as handed to the C.
#[derive(Debug, Clone, Copy)]
pub struct Sink {
    /// The callback.
    pub callback: EnumProc,
    /// Its parameter.
    pub param: *mut c_void,
}

/// The tree walk's parameter (`struct source_enum_data`): the caller's
/// callback and parameter.
#[repr(C)]
struct TreeData {
    enum_callback: EnumProc,
    param: *mut c_void,
}

/// The fields and the plugin call, through the production adapters; the
/// rest of [`enum_active::Libobs`] is provided by [`Calls`].
pub trait Calls: Default {
    /// `obs_transition_enum_sources()`.
    ///
    /// # Safety
    /// A live source and a callback that takes `param`.
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

/// [`enum_active::Libobs`] over the production field adapters and `C`'s
/// calls.
#[derive(Debug, Default, Clone, Copy)]
pub struct With<C>(pub C);

impl<C: Calls> enum_active::Libobs for With<C> {
    type Source = NonNull<c_void>;
    type Sink = Sink;

    fn has_data(&mut self, source: Self::Source) -> bool {
        // SAFETY: a live source; the adapter reads one field.
        unsafe { obs_source_enum_active_has_data(source.as_ptr()) }
    }

    fn is_transition(&mut self, source: Self::Source) -> bool {
        // SAFETY: as `has_data`.
        unsafe { obs_source_enum_active_is_transition(source.as_ptr()) }
    }

    fn has_enum_active(&mut self, source: Self::Source) -> bool {
        // SAFETY: as `has_data`.
        unsafe { obs_source_enum_active_has_callback(source.as_ptr()) }
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

    fn enum_active(&mut self, source: Self::Source, sink: Sink) {
        // SAFETY: a live source with the callback and data (the core
        // checked both); the sink's callback takes its param.
        unsafe { obs_source_enum_active_call(source.as_ptr(), sink.callback, sink.param) }
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

/// `enum_source_active_tree_callback()`.
///
/// # Safety
/// `param` is the [`TreeData`] of a running tree walk; `child` is live.
unsafe extern "C" fn tree_callback<C: Calls>(
    parent: *mut c_void,
    child: *mut c_void,
    param: *mut c_void,
) {
    let sink = Sink {
        callback: tree_callback::<C>,
        param,
    };
    if let Some(child_source) = NonNull::new(child) {
        enum_active::tree_edge(&mut With(C::default()), child_source, sink);
    }
    // SAFETY: `param` is the walk's `TreeData`, alive until it returns.
    let data = unsafe { &*param.cast::<TreeData>() };
    // SAFETY: the caller's callback with its own parameter.
    unsafe { (data.enum_callback)(parent, child, data.param) }
}

/// `obs_source_enum_active_sources()` over `C`'s calls (the export uses
/// [`Libobs`]; the parity test passes stand-ins).
///
/// # Safety
/// `source` is NULL or live; `enum_callback` takes `param`.
pub unsafe fn enum_active_sources_with<C: Calls>(
    source: *mut c_void,
    enum_callback: EnumProc,
    param: *mut c_void,
) {
    let sink = Sink {
        callback: enum_callback,
        param,
    };
    enum_active::enumerate(
        &mut With(C::default()),
        enum_active::SOURCES_FUNCTION,
        NonNull::new(source),
        sink,
    );
}

/// `obs_source_enum_active_tree()` over `C`'s calls, as
/// [`enum_active_sources_with`].
///
/// # Safety
/// As [`enum_active_sources_with`].
pub unsafe fn enum_active_tree_with<C: Calls>(
    source: *mut c_void,
    enum_callback: EnumProc,
    param: *mut c_void,
) {
    let mut data = TreeData {
        enum_callback,
        param,
    };
    let sink = Sink {
        callback: tree_callback::<C>,
        param: (&raw mut data).cast(),
    };
    enum_active::enumerate(
        &mut With(C::default()),
        enum_active::TREE_FUNCTION,
        NonNull::new(source),
        sink,
    );
}

/// # Safety
/// `source` is NULL or live under the original libobs ownership contract;
/// `enum_callback` takes `param`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_enum_active_sources(
    source: *mut c_void,
    enum_callback: EnumProc,
    param: *mut c_void,
) {
    // SAFETY: the caller's contract.
    unsafe { enum_active_sources_with::<Libobs>(source, enum_callback, param) }
}

/// # Safety
/// As [`obs_source_enum_active_sources`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_enum_active_tree(
    source: *mut c_void,
    enum_callback: EnumProc,
    param: *mut c_void,
) {
    // SAFETY: the caller's contract.
    unsafe { enum_active_tree_with::<Libobs>(source, enum_callback, param) }
}
