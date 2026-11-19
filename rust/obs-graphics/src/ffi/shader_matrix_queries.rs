//! Thin shader matrix-parameter ABI; C owns graphics layouts, TLS and backends.
use crate::shader_matrix_queries::{self as core, Entry, Host, Rejection};
use std::ffi::{c_char, c_void};
use std::ptr;

unsafe extern "C" {
    fn obs_graphics_shader_matrix_context() -> *mut c_void;
    fn obs_graphics_shader_matrix_debug(function: *const c_char, reason: *const c_char);
    fn obs_graphics_shader_matrix_viewproj(
        context: *mut c_void,
        shader: *const c_void,
    ) -> *mut c_void;
    fn obs_graphics_shader_matrix_world(context: *mut c_void, shader: *const c_void)
    -> *mut c_void;
}
struct Context(*mut c_void);
fn current() -> Context {
    // SAFETY: reads the calling thread's opaque context without dereferencing it.
    Context(unsafe { obs_graphics_shader_matrix_context() })
}
fn optional(pointer: *mut c_void) -> Option<*mut c_void> {
    (!pointer.is_null()).then_some(pointer)
}
impl Host for Context {
    type Shader = *const c_void;
    type Param = *mut c_void;
    fn has_context(&self) -> bool {
        !self.0.is_null()
    }
    fn debug(&mut self, entry: Entry, reason: Rejection) {
        // SAFETY: both strings are static and NUL-terminated.
        unsafe {
            obs_graphics_shader_matrix_debug(entry.c_name().as_ptr(), reason.message().as_ptr())
        }
    }
    fn viewproj(&mut self, shader: *const c_void) -> Option<*mut c_void> {
        // SAFETY: the safe core checked context and shader before C field access.
        // The shader is forwarded unchanged under the backend's contract.
        optional(unsafe { obs_graphics_shader_matrix_viewproj(self.0, shader) })
    }
    fn world(&mut self, shader: *const c_void) -> Option<*mut c_void> {
        // SAFETY: the safe core checked context and shader before C field access.
        // The shader is forwarded unchanged under the backend's contract.
        optional(unsafe { obs_graphics_shader_matrix_world(self.0, shader) })
    }
}

/// Borrow the shader's view-projection matrix parameter, or NULL.
/// # Safety
/// A current context must have a live backend; a non-NULL shader must meet its contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_shader_get_viewproj_matrix(shader: *const c_void) -> *mut c_void {
    let shader = (!shader.is_null()).then_some(shader);
    core::get_viewproj_matrix(&mut current(), shader).unwrap_or(ptr::null_mut())
}

/// Borrow the shader's world matrix parameter, or NULL.
/// # Safety
/// A current context must have a live backend; a non-NULL shader must meet its contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_shader_get_world_matrix(shader: *const c_void) -> *mut c_void {
    let shader = (!shader.is_null()).then_some(shader);
    core::get_world_matrix(&mut current(), shader).unwrap_or(ptr::null_mut())
}
