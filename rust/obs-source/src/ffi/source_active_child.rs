//! C ABI of `obs_source_add_active_child()` and
//! `obs_source_remove_active_child()`. The full-tree walk (with its stop
//! source), the counter loads and `obs_source_activate()` /
//! `obs_source_deactivate()` stay C: the walk through obs-source.c.h's hidden
//! `obs_source_active_child_enum_full_tree()`, the loads through
//! `obs-source-active-child-fields.c.h` (both without `EXPORT`).
use crate::active_child;
use core::ffi::{CStr, c_char, c_int, c_long, c_void};
use core::ptr::NonNull;

/// `obs_source_enum_proc_t`.
type EnumProc = unsafe extern "C" fn(parent: *mut c_void, child: *mut c_void, param: *mut c_void);

unsafe extern "C" {
    fn obs_source_active_child_enum_full_tree(
        source: *mut c_void,
        enum_callback: EnumProc,
        param: *mut c_void,
        stop_source: *mut c_void,
    );
    fn obs_source_active_child_show_refs(source: *const c_void) -> c_long;
    fn obs_source_active_child_activate_refs(source: *const c_void) -> c_long;
    fn obs_source_activate(source: *mut c_void, view_type: u32);
    fn obs_source_deactivate(source: *mut c_void, view_type: u32);
    fn blog(level: c_int, format: *const c_char, ...);
}

/// `LOG_WARNING`, the level of the parent == child line.
const LOG_WARNING: c_int = 200;
/// `LOG_DEBUG`, the level of `obs_ptr_valid()`'s NULL line.
const LOG_DEBUG: c_int = 400;

/// The walk's visitor, as the walk's `param`.
type Visit<'a> = &'a mut dyn FnMut(Option<NonNull<c_void>>, Option<NonNull<c_void>>);

/// The `obs_source_enum_proc_t` of the walk: forwards each edge to the
/// visitor `param` points at.
unsafe extern "C" fn visit_edge(parent: *mut c_void, child: *mut c_void, param: *mut c_void) {
    // SAFETY: `param` is the `Visit` that `enum_full_tree` passed to the walk
    // and keeps alive (and otherwise unused) until the walk returns.
    let visit = unsafe { &mut *param.cast::<Visit<'_>>() };
    visit(NonNull::new(parent), NonNull::new(child));
}

/// What the shims reach: libobs's C. Each source is non-NULL and, under the
/// libobs contract, live for the call.
#[derive(Debug, Default, Clone, Copy)]
pub struct Libobs;

impl active_child::Libobs for Libobs {
    type Source = NonNull<c_void>;

    fn enum_full_tree(
        &mut self,
        root: Self::Source,
        stop: Self::Source,
        mut visit: &mut dyn FnMut(Option<Self::Source>, Option<Self::Source>),
    ) {
        let param: *mut Visit<'_> = &mut visit;
        // SAFETY: live sources; the walk calls `visit_edge` with `param`
        // only while it runs, and `visit` outlives it.
        unsafe {
            obs_source_active_child_enum_full_tree(
                root.as_ptr(),
                visit_edge,
                param.cast(),
                stop.as_ptr(),
            );
        }
    }

    fn show_refs(&mut self, source: Self::Source) -> c_long {
        // SAFETY: a live source; the adapter loads one field atomically.
        unsafe { obs_source_active_child_show_refs(source.as_ptr()) }
    }

    fn activate_refs(&mut self, source: Self::Source) -> c_long {
        // SAFETY: as `show_refs`.
        unsafe { obs_source_active_child_activate_refs(source.as_ptr()) }
    }

    fn activate(&mut self, source: Self::Source, view_type: u32) {
        // SAFETY: a live source and a `enum view_type` value.
        unsafe { obs_source_activate(source.as_ptr(), view_type) }
    }

    fn deactivate(&mut self, source: Self::Source, view_type: u32) {
        // SAFETY: as `activate`.
        unsafe { obs_source_deactivate(source.as_ptr(), view_type) }
    }

    fn log_null(&mut self, function: &'static CStr, parameter: &'static CStr) {
        // SAFETY: static strings match the two %s arguments of the format.
        unsafe {
            blog(
                LOG_DEBUG,
                c"%s: Null '%s' parameter".as_ptr(),
                function.as_ptr(),
                parameter.as_ptr(),
            );
        }
    }

    fn warn_self(&mut self) {
        // SAFETY: a static string for the one %s argument.
        unsafe {
            blog(
                LOG_WARNING,
                c"%s".as_ptr(),
                active_child::SELF_WARNING.as_ptr(),
            )
        }
    }
}

/// `obs_source_add_active_child()` over any [`active_child::Libobs`] whose
/// sources are pointers (the export uses [`Libobs`]; the parity test passes
/// mutants of it).
pub fn add_with<L: active_child::Libobs<Source = NonNull<c_void>>>(
    libobs: &mut L,
    parent: *mut c_void,
    child: *mut c_void,
) -> bool {
    active_child::add(libobs, NonNull::new(parent), NonNull::new(child))
}

/// `obs_source_remove_active_child()` over any [`active_child::Libobs`], as
/// [`add_with`].
pub fn remove_with<L: active_child::Libobs<Source = NonNull<c_void>>>(
    libobs: &mut L,
    parent: *mut c_void,
    child: *mut c_void,
) {
    active_child::remove(libobs, NonNull::new(parent), NonNull::new(child));
}

/// # Safety
/// Non-null sources must be live under the original libobs ownership
/// contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_add_active_child(
    parent: *mut c_void,
    child: *mut c_void,
) -> bool {
    add_with(&mut Libobs, parent, child)
}

/// # Safety
/// Non-null sources must be live under the original libobs ownership
/// contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_remove_active_child(parent: *mut c_void, child: *mut c_void) {
    remove_with(&mut Libobs, parent, child);
}
