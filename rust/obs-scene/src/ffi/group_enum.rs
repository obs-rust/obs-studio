//! Reuse the group getter and the existing scene enumerator without new layouts.
use core::{ffi::c_void, ptr::NonNull};

/// The public scene callback; false stops the existing scene enumerator.
pub type SceneCallback = unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> bool;

unsafe extern "C" {
    fn obs_scene_enum_items(
        scene: *mut c_void,
        callback: Option<SceneCallback>,
        param: *mut c_void,
    );
}

/// # Safety
/// A non-null group must be a live item, with a live source when marked as a
/// group. Its scene and callback must satisfy `obs_scene_enum_items`' contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_group_enum_items(
    group: *mut c_void,
    callback: Option<SceneCallback>,
    param: *mut c_void,
) {
    // SAFETY: the caller supplies the same live item/source as the C wrapper.
    // The existing getter checks NULL and the group flag before reading source.
    let scene =
        NonNull::new(unsafe { super::item_group_scene::obs_sceneitem_group_get_scene(group) });
    crate::group_enum::enum_group_items(scene, |scene| {
        // SAFETY: the borrowed scene is live, and callback/param are passed
        // through unchanged, including NULL. Enumeration stays in existing C.
        unsafe { obs_scene_enum_items(scene.as_ptr(), callback, param) };
    });
}
