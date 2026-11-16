//! Opaque scenes use the existing private source accessor and source operations.
use crate::refs::{self, Host};
use core::{ffi::c_void, ptr::NonNull};

unsafe extern "C" {
    fn obs_scene_source_fields(scene: *const c_void) -> *const *mut c_void;
    fn obs_scene_ref_acquire_source(source: *mut c_void) -> bool;
    fn obs_scene_ref_release_source(source: *mut c_void);
}
struct Scene;
impl Host for Scene {
    type Scene = NonNull<c_void>;
    type Source = *mut c_void;
    fn source(&mut self, scene: NonNull<c_void>) -> *mut c_void {
        // SAFETY: NonNull rejects absent scenes before invoking the accessor;
        // callers retain readable scene storage through the source operation.
        unsafe { *obs_scene_source_fields(scene.as_ptr()) }
    }
    fn acquire(&mut self, source: *mut c_void) -> bool {
        // SAFETY: the scene's source is live or expired but still allocated,
        // exactly as required by the original obs_source_get_ref contract.
        unsafe { obs_scene_ref_acquire_source(source) }
    }
    fn release(&mut self, source: *mut c_void) {
        // SAFETY: the caller transfers one strong source reference. The call
        // can destroy both source and scene, which are not accessed afterwards.
        unsafe { obs_scene_ref_release_source(source) };
    }
}

/// # Safety
/// A non-NULL scene and its source must remain allocated during promotion.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_scene_get_ref(scene: *mut c_void) -> *mut c_void {
    refs::get_ref(&mut Scene, NonNull::new(scene)).map_or(core::ptr::null_mut(), NonNull::as_ptr)
}

/// # Safety
/// A non-NULL scene must own a source reference. Final release may destroy it;
/// callers synchronize core shutdown as required by obs_source_release.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_scene_release(scene: *mut c_void) {
    refs::release(&mut Scene, NonNull::new(scene));
}
