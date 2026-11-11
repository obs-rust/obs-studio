//! Opaque handles and borrowed name bytes over real C field accessors.
use crate::lookup::{Access, Query};
use core::ffi::{CStr, c_char, c_void};
use core::ptr::{self, NonNull};

type Handle = NonNull<c_void>;
unsafe extern "C" {
    fn obs_scene_lookup_lock(scene: *mut c_void) -> *mut c_void;
    fn obs_scene_lookup_unlock(scene: *mut c_void);
    fn obs_scene_lookup_next(item: *mut c_void) -> *mut c_void;
    fn obs_scene_lookup_name(item: *mut c_void) -> *const c_char;
    fn obs_scene_lookup_id(item: *mut c_void) -> i64;
    fn obs_scene_lookup_is_group(item: *mut c_void) -> bool;
    fn obs_scene_lookup_group(item: *mut c_void) -> *mut c_void;
}
struct Live;
struct Locked(Handle);
impl Drop for Locked {
    fn drop(&mut self) {
        // SAFETY: created only after the corresponding scene's two locks were acquired.
        unsafe { obs_scene_lookup_unlock(self.0.as_ptr()) };
    }
}
impl Access for Live {
    type Scene = Handle;
    type Item = Handle;
    fn with_scene<R>(&self, scene: Handle, visit: impl FnOnce(Option<Handle>) -> R) -> R {
        // SAFETY: public caller keeps the non-null scene alive for the lookup.
        let first = unsafe { obs_scene_lookup_lock(scene.as_ptr()) };
        let _locked = Locked(scene);
        visit(NonNull::new(first))
    }
    fn next(&self, item: Handle) -> Option<Handle> {
        // SAFETY: the core visits only items protected by their containing scene's locks.
        NonNull::new(unsafe { obs_scene_lookup_next(item.as_ptr()) })
    }
    fn name(&self, item: Handle) -> Option<&[u8]> {
        // SAFETY: a live locked item owns a null or terminated source name for this call.
        unsafe { bytes(obs_scene_lookup_name(item.as_ptr())) }
    }
    fn id(&self, item: Handle) -> i64 {
        // SAFETY: the item is live and its containing scene remains locked.
        unsafe { obs_scene_lookup_id(item.as_ptr()) }
    }
    fn is_group(&self, item: Handle) -> bool {
        // SAFETY: the item is live and its containing scene remains locked.
        unsafe { obs_scene_lookup_is_group(item.as_ptr()) }
    }
    fn group(&self, item: Handle) -> Option<Handle> {
        // SAFETY: the core asks only for a group item's live context data under lock.
        NonNull::new(unsafe { obs_scene_lookup_group(item.as_ptr()) })
    }
}
unsafe fn bytes<'a>(name: *const c_char) -> Option<&'a [u8]> {
    NonNull::new(name.cast_mut()).map(|name| {
        // SAFETY: caller guarantees a terminated name, valid for the lookup's duration.
        unsafe { CStr::from_ptr(name.as_ptr()).to_bytes() }
    })
}
fn find(scene: *mut c_void, query: Query<'_>) -> *mut c_void {
    crate::lookup::find(&Live, NonNull::new(scene), query).map_or(ptr::null_mut(), NonNull::as_ptr)
}

/// # Safety
/// Non-null scene must be live, and name must be a readable C string when scene is non-null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_scene_find_source(
    scene: *mut c_void,
    name: *const c_char,
) -> *mut c_void {
    // SAFETY: C permits ignoring name entirely for a null scene.
    let name = if scene.is_null() {
        None
    } else {
        unsafe { bytes(name) }
    };
    find(scene, Query::Name(name.unwrap_or_default()))
}

/// # Safety
/// Same lifetime and name contract as `obs_scene_find_source`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_scene_find_source_recursive(
    scene: *mut c_void,
    name: *const c_char,
) -> *mut c_void {
    // SAFETY: C permits ignoring name entirely for a null scene.
    let name = if scene.is_null() {
        None
    } else {
        unsafe { bytes(name) }
    };
    find(scene, Query::RecursiveName(name.unwrap_or_default()))
}

/// # Safety
/// Non-null scene must be live for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_scene_find_sceneitem_by_id(
    scene: *mut c_void,
    id: i64,
) -> *mut c_void {
    find(scene, Query::Id(id))
}

/// # Safety
/// Non-null scene must be live; non-null name must be a readable C string when scene is non-null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_scene_get_group(
    scene: *mut c_void,
    name: *const c_char,
) -> *mut c_void {
    // SAFETY: preserve C's short circuit for null scenes and null/empty names.
    let name = if scene.is_null() {
        None
    } else {
        unsafe { bytes(name) }
    };
    find(scene, Query::Group(name))
}
