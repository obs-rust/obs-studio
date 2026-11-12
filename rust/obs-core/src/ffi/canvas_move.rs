//! C ABI bridge; libobs owns the sources and retains items during enumeration.
use crate::canvas_move::{self, Host};
use core::ffi::c_void;

unsafe extern "C" {
    fn obs_canvas_move_scene_source(scene: *mut c_void) -> *mut c_void;
    fn obs_canvas_move_item_source(item: *mut c_void) -> *mut c_void;
    fn obs_source_is_group(source: *mut c_void) -> bool;
    fn obs_canvas_remove_source(source: *mut c_void);
    fn obs_canvas_insert_source(canvas: *mut c_void, source: *mut c_void);
    fn obs_scene_enum_items(
        scene: *mut c_void,
        callback: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> bool,
        param: *mut c_void,
    );
}

struct LibobsHost;
impl Host for LibobsHost {
    type Handle = *mut c_void;
    fn scene_source(&self, scene: Self::Handle) -> Self::Handle {
        if scene.is_null() {
            return core::ptr::null_mut();
        }
        // SAFETY: the entry point retains the non-null scene for this call.
        unsafe { obs_canvas_move_scene_source(scene) }
    }
    fn item_source(&self, item: Self::Handle) -> Self::Handle {
        if item.is_null() {
            return core::ptr::null_mut();
        }
        // SAFETY: the enumerator retains the non-null item through the callback.
        unsafe { obs_canvas_move_item_source(item) }
    }
    fn is_group(&self, source: Self::Handle) -> bool {
        // SAFETY: the scene/item retains its source; the predicate accepts null.
        unsafe { obs_source_is_group(source) }
    }
    fn remove_source(&mut self, source: Self::Handle) {
        // SAFETY: the caller or current enumeration item retains this source.
        unsafe { obs_canvas_remove_source(source) }
    }
    fn insert_source(&mut self, canvas: Self::Handle, source: Self::Handle) {
        // SAFETY: both objects are retained by the caller/current item.
        unsafe { obs_canvas_insert_source(canvas, source) }
    }
    fn enumerate_items<F: FnMut(&mut Self, Self::Handle)>(
        &mut self,
        scene: Self::Handle,
        visit: F,
    ) {
        struct Context<'a, F> {
            host: &'a mut LibobsHost,
            visit: F,
        }
        unsafe extern "C" fn callback<F: FnMut(&mut LibobsHost, *mut c_void)>(
            _scene: *mut c_void,
            item: *mut c_void,
            param: *mut c_void,
        ) -> bool {
            // SAFETY: enumeration synchronously passes the unique stack context
            // and a retained item; callbacks do not retain either pointer.
            let context = unsafe { &mut *param.cast::<Context<'_, F>>() };
            (context.visit)(context.host, item);
            true
        }
        let mut context = Context { host: self, visit };
        // SAFETY: libobs invokes callbacks synchronously and retains all items.
        unsafe { obs_scene_enum_items(scene, callback::<F>, (&raw mut context).cast()) }
    }
}

/// # Safety
/// Scene and destination must be live, non-null and synchronized under the
/// original libobs contract. The caller retains the scene during migration.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_canvas_move_scene(scene: *mut c_void, dst: *mut c_void) {
    if scene.is_null() || dst.is_null() {
        return;
    }
    canvas_move::move_scene(&mut LibobsHost, scene, dst);
}
