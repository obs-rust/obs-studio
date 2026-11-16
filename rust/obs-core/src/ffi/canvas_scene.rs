//! C ABI for canvas scene lifecycle; C retains all private object layouts.
use crate::canvas_scene::{self, Host};
use core::ffi::{c_char, c_void};
use core::ptr;

unsafe extern "C" {
    fn obs_source_create_canvas(
        canvas: *mut c_void,
        id: *const c_char,
        name: *const c_char,
        settings: *mut c_void,
        hotkey_data: *mut c_void,
    ) -> *mut c_void;
    fn obs_canvas_remove_source(source: *mut c_void);
    fn obs_canvas_scene_context_data(source: *mut c_void) -> *mut c_void;
    fn obs_canvas_scene_source(scene: *mut c_void) -> *mut c_void;
}

// Private: constructed only by entry points whose callers retain the objects.
struct LibobsHost;
impl Host for LibobsHost {
    type Handle = *mut c_void;
    type Name = *const c_char;
    fn create_scene_source(&mut self, canvas: Self::Handle, name: Self::Name) -> Self::Handle {
        // SAFETY: entry-point contract retains canvas/name for this synchronous call.
        unsafe {
            obs_source_create_canvas(
                canvas,
                c"scene".as_ptr(),
                name,
                ptr::null_mut(),
                ptr::null_mut(),
            )
        }
    }
    fn source_scene(&self, source: Self::Handle) -> Self::Handle {
        if source.is_null() {
            return ptr::null_mut();
        }
        // SAFETY: create_scene_source returned this live source; read only context.data.
        unsafe { obs_canvas_scene_context_data(source) }
    }
    fn scene_source(&self, scene: Self::Handle) -> Self::Handle {
        if scene.is_null() {
            return ptr::null_mut();
        }
        // SAFETY: the entry point requires a live scene.
        unsafe { obs_canvas_scene_source(scene) }
    }
    fn remove_source(&mut self, source: Self::Handle) {
        // SAFETY: scene_source returned the live scene's source, retained by the caller.
        unsafe { obs_canvas_remove_source(source) }
    }
}

/// # Safety
/// Canvas and name must satisfy obs_source_create_canvas's lifetime contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_canvas_scene_create(
    canvas: *mut c_void,
    name: *const c_char,
) -> *mut c_void {
    canvas_scene::create(&mut LibobsHost, canvas, name)
}

/// # Safety
/// Scene must be non-null, live and synchronized as required by libobs.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_canvas_scene_remove(scene: *mut c_void) {
    canvas_scene::remove(&mut LibobsHost, scene);
}
