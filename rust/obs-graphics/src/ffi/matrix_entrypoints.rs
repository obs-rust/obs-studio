//! The C ABI of the matrix calls.
//!
//! The context read and the two log lines are one-line C adapters
//! (`libobs/graphics/matrix-entrypoints-fields.c.h`), which keep the private
//! graphics layout in C. Each call makes one adapter call, which reads this
//! thread's context and gives the addresses of its matrix stack
//! (`graphics->matrix_stack.da`) and of the index of its top
//! (`graphics->cur_matrix`); the Rust stack functions of
//! [`crate::state_stacks`] then work on them in place. Nothing is cached,
//! copied or allocated beyond what the stack itself does. A pointer
//! argument is read only after the context check, and a NULL one does
//! nothing, as `gs_matrix_stack_*` do.
use crate::axisang::AxisAng;
use crate::ffi::axisang::axisang;
use crate::ffi::matrix4::matrix4;
use crate::ffi::quat::quat;
use crate::ffi::state_stacks::{MatrixStack, matrices};
use crate::ffi::vec3::vec3;
use crate::matrix_entrypoints::{self as m, Entry, Host};
use crate::matrix4::Matrix4;
use crate::quat::Quat;
use crate::vec3::Vec3;
use obs_util::ffi::darray::darray;
use std::ffi::c_char;
use std::ptr;

unsafe extern "C" {
    fn obs_graphics_matrix_stack(stack: *mut *mut darray, cur: *mut *mut usize) -> bool;
    fn obs_graphics_matrix_debug(function: *const c_char);
    fn obs_graphics_matrix_pop_error();
}

/// This thread's graphics context, read when a call asks for its stack.
struct Current;

impl Host for Current {
    type Stack<'a> = MatrixStack<'a>;

    fn stack(&mut self) -> Option<(MatrixStack<'_>, &mut usize)> {
        let mut stack = ptr::null_mut();
        let mut cur = ptr::null_mut();
        // SAFETY: the adapter writes both outputs, or neither and false.
        if !unsafe { obs_graphics_matrix_stack(&mut stack, &mut cur) } {
            return None;
        }
        // SAFETY: the context is this thread's and stays current for the
        // call; its matrix stack is a darray of `struct matrix4` and
        // `cur_matrix` a separate field, so the two borrows don't overlap.
        let stack = unsafe { matrices(stack) }?;
        // SAFETY: as above.
        let cur = unsafe { cur.as_mut() }?;
        Some((stack, cur))
    }
    fn debug(&mut self, entry: Entry) {
        // SAFETY: a static terminated name, read during the call.
        unsafe { obs_graphics_matrix_debug(entry.c_name().as_ptr()) }
    }
    fn pop_error(&mut self) {
        // SAFETY: writes a constant log line.
        unsafe { obs_graphics_matrix_pop_error() }
    }
}

/// Push a copy of the top matrix.
/// # Safety
/// A current context must hold a valid matrix stack.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_matrix_push() {
    m::push(&mut Current);
}

/// Pop the top matrix; on the last one, log an error and keep it.
/// # Safety
/// As [`gs_matrix_push`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_matrix_pop() {
    m::pop(&mut Current);
}

/// Set the top matrix to the identity.
/// # Safety
/// As [`gs_matrix_push`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_matrix_identity() {
    m::identity(&mut Current);
}

/// Transpose the top matrix.
/// # Safety
/// As [`gs_matrix_push`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_matrix_transpose() {
    m::transpose(&mut Current);
}

/// Set the top matrix.
/// # Safety
/// As [`gs_matrix_push`]; `matrix` is NULL or readable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_matrix_set(matrix: *const matrix4) {
    // SAFETY: the caller's contract; read only with a context.
    m::set(&mut Current, || {
        unsafe { matrix.as_ref() }.map(|v| Matrix4::from(*v))
    });
}

/// Copy the top matrix to `dst`; without a context `dst` is left alone.
/// # Safety
/// As [`gs_matrix_push`]; `dst` is NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_matrix_get(dst: *mut matrix4) {
    if let Some(top) = m::get(&mut Current)
        && !dst.is_null()
    {
        // SAFETY: writable (the contract).
        unsafe { ptr::write(dst, top.into()) };
    }
}

/// Multiply the top matrix by `matrix` (`matrix` times the top).
/// # Safety
/// As [`gs_matrix_set`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_matrix_mul(matrix: *const matrix4) {
    // SAFETY: the caller's contract; read only with a context.
    m::mul(&mut Current, || {
        unsafe { matrix.as_ref() }.map(|v| Matrix4::from(*v))
    });
}

/// Rotate the top matrix by a quaternion.
/// # Safety
/// As [`gs_matrix_push`]; `rot` is NULL or readable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_matrix_rotquat(rot: *const quat) {
    // SAFETY: the caller's contract; read only with a context.
    m::rotquat(&mut Current, || {
        unsafe { rot.as_ref() }.map(|v| Quat::from(*v))
    });
}

/// Rotate the top matrix by an axis and angle.
/// # Safety
/// As [`gs_matrix_push`]; `rot` is NULL or readable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_matrix_rotaa(rot: *const axisang) {
    // SAFETY: the caller's contract; read only with a context.
    m::rotaa(&mut Current, || {
        unsafe { rot.as_ref() }.map(|v| AxisAng::from(*v))
    });
}

/// Translate the top matrix.
/// # Safety
/// As [`gs_matrix_push`]; `pos` is NULL or readable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_matrix_translate(pos: *const vec3) {
    // SAFETY: the caller's contract; read only with a context.
    m::translate(&mut Current, || {
        unsafe { pos.as_ref() }.map(|v| Vec3::from(*v))
    });
}

/// Scale the top matrix.
/// # Safety
/// As [`gs_matrix_push`]; `scale` is NULL or readable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_matrix_scale(scale: *const vec3) {
    // SAFETY: the caller's contract; read only with a context.
    m::scale(&mut Current, || {
        unsafe { scale.as_ref() }.map(|v| Vec3::from(*v))
    });
}

/// Rotate the top matrix by the axis `(x, y, z)` and `angle`.
/// # Safety
/// As [`gs_matrix_push`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_matrix_rotaa4f(x: f32, y: f32, z: f32, angle: f32) {
    m::rotaa4f(&mut Current, x, y, z, angle);
}

/// Translate the top matrix by `(x, y, z)`.
/// # Safety
/// As [`gs_matrix_push`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_matrix_translate3f(x: f32, y: f32, z: f32) {
    m::translate3f(&mut Current, x, y, z);
}

/// Scale the top matrix by `(x, y, z)`.
/// # Safety
/// As [`gs_matrix_push`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_matrix_scale3f(x: f32, y: f32, z: f32) {
    m::scale3f(&mut Current, x, y, z);
}
