//! Thin public ABI and private field access for the render-target setters.
use crate::render_target_setters::{self as core, Entry, Host};
use std::ffi::{c_char, c_int, c_void};
use std::ptr;

unsafe extern "C" {
    fn obs_graphics_render_target_setters_context() -> *mut c_void;
    fn obs_graphics_render_target_setters_debug(function: *const c_char, reason: *const c_char);
    fn obs_graphics_render_target_setters_target(
        context: *mut c_void,
        texture: *mut c_void,
        depth: *mut c_void,
    );
    fn obs_graphics_render_target_setters_cube(
        context: *mut c_void,
        texture: *mut c_void,
        side: c_int,
        depth: *mut c_void,
    );
}

struct Context(*mut c_void);

fn current() -> Context {
    // SAFETY: retrieves TLS without dereferencing the nullable context.
    Context(unsafe { obs_graphics_render_target_setters_context() })
}

fn optional(pointer: *mut c_void) -> Option<*mut c_void> {
    (!pointer.is_null()).then_some(pointer)
}

impl Host for Context {
    type Texture = *mut c_void;
    type Depth = *mut c_void;
    fn has_context(&self) -> bool {
        !self.0.is_null()
    }
    fn debug(&mut self, entry: Entry) {
        // SAFETY: static NUL-terminated strings; no graphics fields are read.
        unsafe {
            obs_graphics_render_target_setters_debug(
                entry.c_name().as_ptr(),
                core::NO_CONTEXT.as_ptr(),
            )
        }
    }
    fn target(&mut self, texture: Option<Self::Texture>, depth: Option<Self::Depth>) {
        // SAFETY: the Rust core checked context for NULL before field access.
        // Nullable handles are passed through without being dereferenced.
        unsafe {
            obs_graphics_render_target_setters_target(
                self.0,
                texture.unwrap_or(ptr::null_mut()),
                depth.unwrap_or(ptr::null_mut()),
            )
        }
    }
    fn cube(&mut self, texture: Option<Self::Texture>, side: i32, depth: Option<Self::Depth>) {
        // SAFETY: the Rust core checked context for NULL before field access.
        // The C contract forwards all side values and both nullable handles.
        unsafe {
            obs_graphics_render_target_setters_cube(
                self.0,
                texture.unwrap_or(ptr::null_mut()),
                side,
                depth.unwrap_or(ptr::null_mut()),
            )
        }
    }
}

/// Set a nullable render target and depth buffer.
/// # Safety
/// A current context must have a live backend; non-NULL handles must meet its contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_set_render_target(texture: *mut c_void, depth: *mut c_void) {
    core::set_render_target(&mut current(), optional(texture), optional(depth));
}

/// Set a nullable cube target, forwarding the face value unchanged.
/// # Safety
/// A current context must have a live backend; non-NULL handles must meet its contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_set_cube_render_target(
    texture: *mut c_void,
    side: c_int,
    depth: *mut c_void,
) {
    core::set_cube_render_target(&mut current(), optional(texture), side, optional(depth));
}
