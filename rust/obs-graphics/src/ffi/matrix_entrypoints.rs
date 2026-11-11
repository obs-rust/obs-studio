//! Original libobs matrix entry points over a validated thread-local context.
//!
//! The C entries (`libobs/graphics/state-matrix-entrypoints.c.h`) guard with
//! `gs_valid`, delegate to the `gs_matrix_stack_*` primitives
//! (`ffi/state_stacks.rs`), and `gs_matrix_pop` logs the
//! `Tried to pop last matrix on stack` decision. The entry orchestration is
//! the core module; this shim validates the current context, adapts its
//! matrix stack through [`MatrixStack`], and reports the pop decision through
//! the private fields adapter's logger.
use crate::matrix_entrypoints::{self, Context};
use crate::matrix4::Matrix4;
use crate::state_stacks::{self, MatrixOp};
use core::ffi::{c_char, c_void};
use core::ptr;
use core::ptr::NonNull;

use obs_util::ffi::darray::darray;

use crate::ffi::axisang::axisang;
use crate::ffi::matrix4::matrix4;
use crate::ffi::quat::quat;
use crate::ffi::vec3::vec3;

unsafe extern "C" {
    fn obs_graphics_matrix_context(function: *const c_char) -> *mut c_void;
    fn obs_graphics_matrix_stack(context: *mut c_void) -> *mut darray;
    fn obs_graphics_matrix_current(context: *mut c_void) -> *mut usize;
    fn obs_graphics_matrix_log_pop_last();
}
struct Native(NonNull<c_void>);
impl Native {
    fn context(function: &core::ffi::CStr) -> Option<Self> {
        // SAFETY: static function name; the adapter validates the current context.
        NonNull::new(unsafe { obs_graphics_matrix_context(function.as_ptr()) }).map(Self)
    }
}
impl Context for Native {
    fn matrix_stack_push(&mut self) {
        // SAFETY: Native is constructed only for a live, nonnull current
        // context; the private adapters return its exclusively accessed
        // matrix stack and current index.
        if let (Some(mut s), Some(cur)) = (
            unsafe {
                crate::ffi::state_stacks::matrices(obs_graphics_matrix_stack(self.0.as_ptr()))
            },
            unsafe { obs_graphics_matrix_current(self.0.as_ptr()).as_mut() },
        ) {
            state_stacks::matrix_push(&mut s, cur);
        }
    }
    fn matrix_stack_pop(&mut self) -> bool {
        // SAFETY: as in `matrix_stack_push`.
        match (
            unsafe {
                crate::ffi::state_stacks::matrices(obs_graphics_matrix_stack(self.0.as_ptr()))
            },
            unsafe { obs_graphics_matrix_current(self.0.as_ptr()).as_mut() },
        ) {
            (Some(mut s), Some(cur)) => state_stacks::matrix_pop(&mut s, cur),
            _ => false,
        }
    }
    fn matrix_apply(&mut self, op: MatrixOp) {
        // SAFETY: as in `matrix_stack_push`; the index is read, not mutated.
        if let (Some(mut s), Some(cur)) = (
            unsafe {
                crate::ffi::state_stacks::matrices(obs_graphics_matrix_stack(self.0.as_ptr()))
            },
            unsafe { obs_graphics_matrix_current(self.0.as_ptr()).as_mut() },
        ) {
            state_stacks::matrix_apply(&mut s, *cur, op);
        }
    }
    fn matrix_get(&self) -> Option<Matrix4> {
        // SAFETY: as in `matrix_stack_push`; the stack is only read.
        match (
            unsafe {
                crate::ffi::state_stacks::matrices(obs_graphics_matrix_stack(self.0.as_ptr()))
            },
            unsafe { obs_graphics_matrix_current(self.0.as_ptr()).as_mut() },
        ) {
            (Some(s), Some(cur)) => state_stacks::matrix_get(&s, *cur),
            _ => None,
        }
    }
}

/// Save a copy of the current top matrix.
#[unsafe(no_mangle)]
pub extern "C" fn gs_matrix_push() {
    matrix_entrypoints::push(Native::context(c"gs_matrix_push").as_mut());
}
/// Restore the previous matrix, logging when the stack held only the current one.
#[unsafe(no_mangle)]
pub extern "C" fn gs_matrix_pop() {
    // SAFETY: the logger only calls blog, which writes to the process log.
    if matrix_entrypoints::pop(Native::context(c"gs_matrix_pop").as_mut()) == Some(false) {
        unsafe { obs_graphics_matrix_log_pop_last() };
    }
}
/// Reset the top to the identity matrix.
#[unsafe(no_mangle)]
pub extern "C" fn gs_matrix_identity() {
    matrix_entrypoints::identity(Native::context(c"gs_matrix_identity").as_mut());
}
/// Transpose the top.
#[unsafe(no_mangle)]
pub extern "C" fn gs_matrix_transpose() {
    matrix_entrypoints::transpose(Native::context(c"gs_matrix_transpose").as_mut());
}
/// Replace the top with a copy of the argument.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_matrix_set(matrix: *const matrix4) {
    let mut context = Native::context(c"gs_matrix_set");
    // SAFETY: `matrix` is null or readable, as the C caller passes a struct.
    if let Some(matrix) = unsafe { matrix.as_ref() } {
        // SAFETY: readable.
        matrix_entrypoints::set(context.as_mut(), Matrix4::from(*matrix));
    }
}
/// Copy the top to the caller's matrix when the context has one.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_matrix_get(dst: *mut matrix4) {
    if let Some(top) = matrix_entrypoints::get(Native::context(c"gs_matrix_get").as_ref())
        && let false = dst.is_null()
    {
        // SAFETY: writable (the contract).
        unsafe { ptr::write(dst, top.into()) };
    }
}
/// Multiply the top by the argument times the top.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_matrix_mul(matrix: *const matrix4) {
    let mut context = Native::context(c"gs_matrix_mul");
    // SAFETY: `matrix` is null or readable, as the C caller passes a struct.
    if let Some(matrix) = unsafe { matrix.as_ref() } {
        // SAFETY: readable.
        matrix_entrypoints::mul(context.as_mut(), Matrix4::from(*matrix));
    }
}
/// Rotate the top by the quaternion argument.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_matrix_rotquat(rot: *const quat) {
    let mut context = Native::context(c"gs_matrix_rotquat");
    // SAFETY: `rot` is null or readable, as the C caller passes a struct.
    if let Some(rot) = unsafe { rot.as_ref() } {
        // SAFETY: readable.
        matrix_entrypoints::rotquat(context.as_mut(), (*rot).into());
    }
}
/// Rotate the top by the axis-angle argument.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_matrix_rotaa(rot: *const axisang) {
    let mut context = Native::context(c"gs_matrix_rotaa");
    // SAFETY: `rot` is null or readable, as the C caller passes a struct.
    if let Some(rot) = unsafe { rot.as_ref() } {
        // SAFETY: readable.
        matrix_entrypoints::rotaa(context.as_mut(), (*rot).into());
    }
}
/// Rotate the top by the scalar axis-angle form.
#[unsafe(no_mangle)]
pub extern "C" fn gs_matrix_rotaa4f(x: f32, y: f32, z: f32, angle: f32) {
    matrix_entrypoints::rotaa4f(
        Native::context(c"gs_matrix_rotaa4f").as_mut(),
        x,
        y,
        z,
        angle,
    );
}
/// Translate the top by the vector argument.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_matrix_translate(pos: *const vec3) {
    let mut context = Native::context(c"gs_matrix_translate");
    // SAFETY: `pos` is null or readable, as the C caller passes a struct.
    if let Some(pos) = unsafe { pos.as_ref() } {
        // SAFETY: readable.
        matrix_entrypoints::translate(context.as_mut(), (*pos).into());
    }
}
/// Translate the top by the scalar vector form.
#[unsafe(no_mangle)]
pub extern "C" fn gs_matrix_translate3f(x: f32, y: f32, z: f32) {
    matrix_entrypoints::translate3f(Native::context(c"gs_matrix_translate3f").as_mut(), x, y, z);
}
/// Scale the top by the vector argument.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_matrix_scale(factor: *const vec3) {
    let mut context = Native::context(c"gs_matrix_scale");
    // SAFETY: `factor` is null or readable, as the C caller passes a struct.
    if let Some(factor) = unsafe { factor.as_ref() } {
        // SAFETY: readable.
        matrix_entrypoints::scale(context.as_mut(), (*factor).into());
    }
}
/// Scale the top by the scalar vector form.
#[unsafe(no_mangle)]
pub extern "C" fn gs_matrix_scale3f(x: f32, y: f32, z: f32) {
    matrix_entrypoints::scale3f(Native::context(c"gs_matrix_scale3f").as_mut(), x, y, z);
}
