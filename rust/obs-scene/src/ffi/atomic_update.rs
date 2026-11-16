//! The scene remains opaque; lifetime and full locks stay in the existing C API.
use crate::atomic_update::{self, Scene};
use core::ffi::c_void;
use core::ptr::NonNull;

unsafe extern "C" {
    fn obs_scene_get_ref(scene: *mut c_void) -> *mut c_void;
    fn obs_scene_release(scene: *mut c_void);
    fn obs_scene_atomic_full_lock(scene: *mut c_void);
    fn obs_scene_atomic_full_unlock(scene: *mut c_void);
}

struct CScene {
    ptr: NonNull<c_void>,
    owned: bool,
}
impl Scene for CScene {
    fn acquire(&self) -> Option<Self> {
        // SAFETY: construction is confined to the shim's readable-scene contract.
        NonNull::new(unsafe { obs_scene_get_ref(self.ptr.as_ptr()) })
            .map(|ptr| Self { ptr, owned: true })
    }
    fn lock(&self) {
        // SAFETY: the core holds the acquired reference through unlock.
        unsafe { obs_scene_atomic_full_lock(self.ptr.as_ptr()) };
    }
    fn unlock(&self) {
        // SAFETY: paired with lock on the same scene, before releasing its ref.
        unsafe { obs_scene_atomic_full_unlock(self.ptr.as_ptr()) };
    }
}
impl Drop for CScene {
    fn drop(&mut self) {
        if self.owned {
            // SAFETY: only a successful get_ref constructs an owned scene.
            unsafe { obs_scene_release(self.ptr.as_ptr()) };
        }
    }
}

/// # Safety
/// A non-null scene must remain readable until reference acquisition, including
/// during a destroy signal. For a live scene, `func` must be callable with `data`
/// and must not unwind. NULL callbacks on live scenes are invalid as in C.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_scene_atomic_update(
    scene: *mut c_void,
    func: Option<unsafe extern "C" fn(*mut c_void, *mut c_void)>,
    data: *mut c_void,
) {
    let scene = NonNull::new(scene).map(|ptr| CScene { ptr, owned: false });
    atomic_update::update(scene.as_ref(), |scene| {
        // SAFETY: the core owns a strong ref and both locks; caller supplies func/data.
        unsafe { func.expect("live scene requires a callback")(data, scene.ptr.as_ptr()) };
    });
}
