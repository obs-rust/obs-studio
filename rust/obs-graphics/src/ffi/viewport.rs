//! Private C layout and callback boundary for viewport policy.
use crate::viewport::{self, Backend};
use std::ffi::{c_char, c_void};
unsafe extern "C" {
    fn obs_graphics_viewport_valid(function: *const c_char) -> bool;
    fn obs_graphics_viewport_maximum(graphics: *mut c_void, value: *mut u32) -> bool;
    fn obs_graphics_viewport_type(graphics: *mut c_void) -> i32;
    fn obs_graphics_viewport_size(width: *mut u32, height: *mut u32);
    fn obs_graphics_viewport_set(x: i32, y: i32, width: i32, height: i32);
    fn obs_graphics_viewport_ortho(
        left: f32,
        right: f32,
        top: f32,
        bottom: f32,
        near: f32,
        far: f32,
    );
}
struct Context(*mut c_void);
impl Backend for Context {
    fn valid(&mut self, function: &'static str) -> bool {
        let function = match function {
            "gs_reset_viewport" => c"gs_reset_viewport",
            _ => c"gs_set_2d_mode",
        };
        // SAFETY: the string is static; C checks the thread-local context.
        unsafe { obs_graphics_viewport_valid(function.as_ptr()) }
    }
    fn size(&mut self) -> (u32, u32) {
        let (mut width, mut height) = (0, 0);
        // SAFETY: the core validates the current context before this call.
        unsafe { obs_graphics_viewport_size(&mut width, &mut height) };
        (width, height)
    }
    fn viewport(&mut self, v: [i32; 4]) {
        // SAFETY: the core validated the context; these are scalar arguments.
        unsafe { obs_graphics_viewport_set(v[0], v[1], v[2], v[3]) }
    }
    fn ortho(&mut self, v: [f32; 6]) {
        // SAFETY: the core validated the context; these are scalar arguments.
        unsafe { obs_graphics_viewport_ortho(v[0], v[1], v[2], v[3], v[4], v[5]) }
    }
    fn maximum(&mut self) -> Option<u32> {
        let mut value = 0;
        // SAFETY: the private entry point receives the live gs_create context.
        unsafe { obs_graphics_viewport_maximum(self.0, &mut value) }.then_some(value)
    }
    fn device_type(&mut self) -> i32 {
        // SAFETY: the private entry point receives the live gs_create context.
        unsafe { obs_graphics_viewport_type(self.0) }
    }
}
/// Reset the current thread's viewport to its full target.
#[unsafe(no_mangle)]
pub extern "C" fn gs_reset_viewport() {
    viewport::reset_viewport(&mut Context(std::ptr::null_mut()));
}
/// Set the current thread's pixel-space projection.
#[unsafe(no_mangle)]
pub extern "C" fn gs_set_2d_mode() {
    viewport::set_2d_mode(&mut Context(std::ptr::null_mut()));
}
/// Read a live device's maximum texture capability.
/// # Safety
/// `graphics` must point to the initialized context owned by gs_create.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_graphics_device_max_texture_size(graphics: *mut c_void) -> u32 {
    viewport::max_texture_size(&mut Context(graphics))
}
