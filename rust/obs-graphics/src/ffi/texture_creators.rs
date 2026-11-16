//! Opaque context and device-pointer conversion for texture creation.
use crate::sprite::MipWarning;
use crate::texture_creators::{self, Device, Request};
use std::ffi::{c_char, c_void};
use std::ptr::{self, NonNull};

unsafe extern "C" {
    fn gs_get_context() -> *mut c_void;
    fn obs_texture_creator_dispatch(
        context: *mut c_void,
        width: u32,
        height: u32,
        format: i32,
        levels: u32,
        data: *const *const u8,
        flags: u32,
        cube: bool,
    ) -> *mut c_void;
    fn obs_texture_creator_log(level: i32, text: *const c_char);
}
struct Host(*mut c_void);
impl Device<NonNull<*const u8>> for Host {
    type Texture = NonNull<c_void>;
    fn has_context(&self) -> bool {
        !self.0.is_null()
    }
    fn no_context(&mut self, cube: bool) {
        let text = if cube {
            c"gs_cubetexture_create: called while not in a graphics context"
        } else {
            c"gs_texture_create: called while not in a graphics context"
        };
        // SAFETY: static terminated message; the adapter copies it synchronously.
        unsafe { obs_texture_creator_log(400, text.as_ptr()) };
    }
    fn warning(&mut self, warning: MipWarning) {
        let text = match warning {
            MipWarning::NotPow2 => c"Cannot use mipmaps with a non-power-of-two texture.  Disabling mipmaps for this texture.",
            MipWarning::RenderTarget => c"Cannot use mipmaps with render targets.  Disabling mipmaps for this texture.",
            MipWarning::Ok => return,
        };
        // SAFETY: static terminated message; logging does not retain the pointer.
        unsafe { obs_texture_creator_log(200, text.as_ptr()) };
    }
    fn create(&mut self, r: Request<NonNull<*const u8>>) -> Option<Self::Texture> {
        // SAFETY: core checked context presence; the caller supplies backend-valid data.
        NonNull::new(unsafe {
            obs_texture_creator_dispatch(
                self.0,
                r.width,
                r.height,
                r.format,
                r.levels,
                r.data.map_or(ptr::null(), |p| p.as_ptr()),
                r.flags,
                r.cube,
            )
        })
    }
}
unsafe fn run(
    width: u32,
    height: u32,
    format: i32,
    levels: u32,
    data: *const *const u8,
    flags: u32,
    cube: bool,
) -> *mut c_void {
    // SAFETY: public libobs accessor returns this thread's live graphics context.
    let mut host = Host(unsafe { gs_get_context() });
    texture_creators::create(
        &mut host,
        Request {
            width,
            height,
            format,
            levels,
            flags,
            cube,
            data: NonNull::new(data.cast_mut()),
        },
    )
    .map_or(ptr::null_mut(), NonNull::as_ptr)
}
/// Create a 2D texture with the unchanged libobs ABI.
///
/// # Safety
/// Initial data must satisfy the active backend's texture upload contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_texture_create(
    width: u32,
    height: u32,
    format: i32,
    levels: u32,
    data: *const *const u8,
    flags: u32,
) -> *mut c_void {
    // SAFETY: forwards the caller's backend data contract.
    unsafe { run(width, height, format, levels, data, flags, false) }
}
/// Create a cube texture with the unchanged libobs ABI.
///
/// # Safety
/// Initial data must satisfy the active backend's cube upload contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_cubetexture_create(
    size: u32,
    format: i32,
    levels: u32,
    data: *const *const u8,
    flags: u32,
) -> *mut c_void {
    // SAFETY: forwards the caller's backend data contract.
    unsafe { run(size, size, format, levels, data, flags, true) }
}
