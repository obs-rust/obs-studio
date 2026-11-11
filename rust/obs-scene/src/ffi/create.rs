//! Scene constructor ABI; source allocation and private layouts remain in C.
use crate::create::{self, Host};
use core::ffi::{c_char, c_void};

unsafe extern "C" {
    fn obs_scene_create_main_canvas() -> *mut c_void;
    fn obs_scene_create_public_source(
        canvas: *mut c_void,
        id: *const c_char,
        name: *const c_char,
    ) -> *mut c_void;
    fn obs_scene_create_private_source(
        canvas: *mut c_void,
        id: *const c_char,
        name: *const c_char,
    ) -> *mut c_void;
}
struct Libobs;
impl Host for Libobs {
    type Handle = *mut c_void;
    type Name = *const c_char;
    fn main_canvas(&self) -> Self::Handle {
        // SAFETY: constructors require initialized libobs; this borrows its canvas.
        unsafe { obs_scene_create_main_canvas() }
    }
    fn public(
        &mut self,
        canvas: Self::Handle,
        id: &'static [u8; 6],
        name: Self::Name,
    ) -> Self::Handle {
        // SAFETY: the core supplies the static NUL-terminated scene ID. The ABI
        // caller keeps name valid through synchronous creation and callbacks.
        unsafe { obs_scene_create_public_source(canvas, id.as_ptr().cast(), name) }
    }
    fn private(
        &mut self,
        canvas: Self::Handle,
        id: &'static [u8; 6],
        name: Self::Name,
    ) -> Self::Handle {
        // SAFETY: same lifetime contract as public; NULL names pass unchanged.
        unsafe { obs_scene_create_private_source(canvas, id.as_ptr().cast(), name) }
    }
}
/// Create a public scene with one caller-owned reference and canvas ownership.
///
/// # Safety
/// Libobs must be initialized and name must be NULL or a valid C string for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_scene_create(name: *const c_char) -> *mut c_void {
    create::create(&mut Libobs, name, false)
}
/// Create a private scene with one caller-owned reference.
///
/// # Safety
/// Libobs must be initialized and name must be NULL or a valid C string for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_scene_create_private(name: *const c_char) -> *mut c_void {
    create::create(&mut Libobs, name, true)
}
