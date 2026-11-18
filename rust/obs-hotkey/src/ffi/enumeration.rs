//! Opaque handles and private C layout adapters for registry enumeration.
use super::{binding_press::obs_hotkey_binding, pair::obs_hotkey_t};
use crate::enumeration::{self, Context, Registry};
use core::ffi::c_void;
use obs_util::ffi::darray::darray;

type HotkeyCallback = Option<unsafe extern "C" fn(*mut c_void, usize, *mut c_void) -> bool>;
type ContextCallback = Option<unsafe extern "C" fn(*mut c_void, *mut c_void) -> bool>;
type BindingCallback = HotkeyCallback;
type BindingVisitor<'a> = &'a mut dyn FnMut(usize, *mut c_void) -> bool;

unsafe extern "C" {
    fn obs_hotkey_seam_lock() -> bool;
    fn obs_hotkey_seam_unlock();
    fn obs_hotkey_seam_find(id: usize) -> *mut obs_hotkey_t;
    fn obs_hotkey_seam_enum_bindings(
        callback: unsafe extern "C" fn(*mut c_void, usize, *mut obs_hotkey_binding) -> bool,
        data: *mut c_void,
    );
    fn obs_hotkey_enum_first() -> *mut c_void;
    fn obs_hotkey_enum_next(key: *mut c_void) -> *mut c_void;
    fn obs_hotkey_enum_id(key: *mut c_void) -> usize;
    fn obs_hotkey_enum_context_fields(context: *mut c_void) -> *const darray;
}

struct Live;
impl Registry for Live {
    type Hotkey = *mut c_void;
    type Binding = *mut c_void;
    fn lock(&mut self) -> bool {
        // SAFETY: existing seam checks core presence and locks its recursive mutex.
        unsafe { obs_hotkey_seam_lock() }
    }
    fn unlock(&mut self) {
        // SAFETY: core calls this once per successful acquisition.
        unsafe { obs_hotkey_seam_unlock() };
    }
    fn first(&self) -> Option<Self::Hotkey> {
        // SAFETY: the mutex is held, so the core is live.
        let key = unsafe { obs_hotkey_enum_first() };
        (!key.is_null()).then_some(key)
    }
    fn next(&self, key: Self::Hotkey) -> Option<Self::Hotkey> {
        if key.is_null() {
            return None;
        }
        // SAFETY: non-null current entry, read before its callback can remove it.
        let next = unsafe { obs_hotkey_enum_next(key) };
        (!next.is_null()).then_some(next)
    }
    fn id(&self, key: Self::Hotkey) -> usize {
        if key.is_null() {
            return usize::MAX;
        }
        // SAFETY: non-null current entry, read before its callback runs.
        unsafe { obs_hotkey_enum_id(key) }
    }
    fn bindings(&mut self, callback: BindingVisitor<'_>) {
        let mut visitor = callback;
        // SAFETY: visitor lives until this synchronous walk returns. The seam
        // retains the C array snapshot and passes the original indices/handles.
        unsafe {
            obs_hotkey_seam_enum_bindings(visit_binding, (&raw mut visitor).cast());
        }
    }
}

unsafe extern "C" fn visit_binding(
    data: *mut c_void,
    index: usize,
    binding: *mut obs_hotkey_binding,
) -> bool {
    // SAFETY: Live::bindings passes a live pointer to its stack visitor.
    let visitor = unsafe { &mut *data.cast::<BindingVisitor<'_>>() };
    visitor(index, binding.cast())
}

struct LiveContext(*mut c_void);
impl Context for LiveContext {
    type Hotkey = *mut c_void;
    type Snapshot = *const usize;
    fn snapshot(&self) -> (usize, Self::Snapshot) {
        if self.0.is_null() {
            return (0, core::ptr::null());
        }
        // SAFETY: non-null context belongs to the caller holding the hotkey
        // mutex. Copy just its array header, never borrow across callbacks.
        let fields = unsafe { &*obs_hotkey_enum_context_fields(self.0) };
        (fields.num, fields.array.cast())
    }
    fn id(&self, snapshot: &Self::Snapshot, index: usize) -> usize {
        // SAFETY: index is below the snapshotted count, as in the original C;
        // callers must not reallocate the context ID array during enumeration.
        unsafe { *snapshot.add(index) }
    }
    fn find(&self, id: usize) -> Option<Self::Hotkey> {
        // SAFETY: context callers hold the registry mutex; resolve every ID live.
        let key = unsafe { obs_hotkey_seam_find(id) }.cast::<c_void>();
        (!key.is_null()).then_some(key)
    }
}

/// # Safety
/// Callback and data must remain valid through the synchronous enumeration.
/// Callbacks may reenter under the existing recursive hotkey mutex and remove
/// their current entry, but must keep the cached next entry alive (C contract).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_enum_hotkeys(callback: HotkeyCallback, data: *mut c_void) {
    enumeration::hotkeys(&mut Live, |id, key| {
        // SAFETY: caller supplies a valid callback for each visited entry.
        unsafe { callback.is_none_or(|callback| callback(data, id, key)) }
    });
}

/// # Safety
/// Callback/data and the binding snapshot must remain valid during the walk.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_enum_hotkey_bindings(callback: BindingCallback, data: *mut c_void) {
    enumeration::bindings(&mut Live, |index, binding| {
        // SAFETY: caller supplies a valid callback for each borrowed binding.
        unsafe { callback.is_none_or(|callback| callback(data, index, binding)) }
    });
}

/// Private seam consumed by C and Rust save/load callers.
///
/// # Safety
/// Caller holds the hotkey mutex. Context and callback data remain valid, and
/// callbacks do not reallocate the snapshotted context ID array.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkey_seam_enum_context(
    context: *mut c_void,
    callback: ContextCallback,
    data: *mut c_void,
) {
    enumeration::context(&LiveContext(context), |key| {
        // SAFETY: caller's synchronous context enumeration contract above.
        unsafe { callback.is_none_or(|callback| callback(data, key)) }
    });
}
