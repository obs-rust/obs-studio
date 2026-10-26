//! Safe core of `libobs/graphics/graphics-state-stacks.c.h`: the state
//! stacks of the graphics context.
//!
//! The matrix stack (push, pop, the top and the operations on it, on
//! [`Matrix4`]), the viewport stack and the blend state stack, with the
//! steps of `gs_reset_blend_state` ([`blend_reset_steps`]). The stacks are
//! [`Stack`]s: a `Vec` here, the C darrays of `graphics_subsystem` in the
//! shim. `graphics.c` keeps `gs_valid`, the device calls and the logging.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::unreachable
)]

use crate::axisang::AxisAng;
use crate::matrix4::Matrix4;
use crate::quat::Quat;
use crate::vec3::Vec3;
use crate::vec4::Vec4;

/// A stack of values: the operations the C does on its darrays.
pub trait Stack<T: Copy> {
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    fn get(&self, index: usize) -> Option<T>;
    fn set(&mut self, index: usize, value: T);
    fn push(&mut self, value: T);
    /// `darray_erase`: remove `index`, the ones after it move down.
    fn erase(&mut self, index: usize);
    /// `darray_pop_back`.
    fn pop(&mut self);
}

impl<T: Copy> Stack<T> for Vec<T> {
    fn len(&self) -> usize {
        Vec::len(self)
    }

    fn get(&self, index: usize) -> Option<T> {
        <[T]>::get(self, index).copied()
    }

    fn set(&mut self, index: usize, value: T) {
        if let Some(slot) = self.get_mut(index) {
            *slot = value;
        }
    }

    fn push(&mut self, value: T) {
        Vec::push(self, value);
    }

    fn erase(&mut self, index: usize) {
        if index < Vec::len(self) {
            self.remove(index);
        }
    }

    fn pop(&mut self) {
        Vec::pop(self);
    }
}

/// `struct gs_rect`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GsRect {
    pub x: i32,
    pub y: i32,
    pub cx: i32,
    pub cy: i32,
}

/// `struct blend_state`, with the enums as their values.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BlendState {
    pub enabled: bool,
    pub src_c: u32,
    pub dest_c: u32,
    pub src_a: u32,
    pub dest_a: u32,
    pub op: u32,
}

/// `struct gs_blend_reset`: the device steps of `gs_reset_blend_state`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BlendReset {
    pub enable: bool,
    pub function: bool,
    pub op: bool,
}

/// `enum gs_blend_type` values of the default state.
pub mod blend {
    pub const ZERO: u32 = 0;
    pub const ONE: u32 = 1;
    pub const SRCALPHA: u32 = 4;
    pub const INVSRCALPHA: u32 = 5;
    /// `GS_BLEND_OP_ADD`.
    pub const OP_ADD: u32 = 0;
}

/// An operation on the top of the matrix stack.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MatrixOp {
    Identity,
    Transpose,
    Set(Matrix4),
    Mul(Matrix4),
    RotQuat(Quat),
    RotAa(AxisAng),
    Translate(Vec3),
    Scale(Vec3),
}

/// `matrix4_identity`.
#[must_use]
pub fn identity() -> Matrix4 {
    Matrix4::new(
        Vec4::new(1.0, 0.0, 0.0, 0.0),
        Vec4::new(0.0, 1.0, 0.0, 0.0),
        Vec4::new(0.0, 0.0, 1.0, 0.0),
        Vec4::new(0.0, 0.0, 0.0, 1.0),
    )
}

/// `gs_matrix_stack_push`: a copy of the top on top.
pub fn matrix_push<S: Stack<Matrix4>>(stack: &mut S, cur: &mut usize) {
    if let Some(top) = stack.get(*cur) {
        stack.push(top);
        *cur = cur.wrapping_add(1);
    }
}

/// `gs_matrix_stack_pop`: false when the top is the last matrix.
pub fn matrix_pop<S: Stack<Matrix4>>(stack: &mut S, cur: &mut usize) -> bool {
    if *cur == 0 {
        return false;
    }
    stack.erase(*cur);
    *cur = cur.wrapping_sub(1);
    true
}

/// The operations on the top (`gs_matrix_stack_identity` to
/// `gs_matrix_stack_scale`).
pub fn matrix_apply<S: Stack<Matrix4>>(stack: &mut S, cur: usize, op: MatrixOp) {
    let Some(top) = stack.get(cur) else {
        return;
    };
    let result = match op {
        MatrixOp::Identity => identity(),
        MatrixOp::Transpose => top.transpose(),
        MatrixOp::Set(matrix) => matrix,
        // matrix4_mul(top, matrix, top): the argument times the top.
        MatrixOp::Mul(matrix) => core::ops::Mul::mul(matrix, &top),
        MatrixOp::RotQuat(rot) => top.rotate_i(rot),
        MatrixOp::RotAa(rot) => top.rotate_aa_i(rot),
        MatrixOp::Translate(pos) => top.translate3v_i(pos),
        MatrixOp::Scale(scale) => top.scale_i(scale),
    };
    stack.set(cur, result);
}

/// `gs_matrix_stack_get`: the top.
#[must_use]
pub fn matrix_get<S: Stack<Matrix4>>(stack: &S, cur: usize) -> Option<Matrix4> {
    stack.get(cur)
}

/// `gs_viewport_stack_push` and `gs_blend_state_stack_push`.
pub fn state_push<T: Copy, S: Stack<T>>(stack: &mut S, value: T) {
    stack.push(value);
}

/// `gs_viewport_stack_pop` and `gs_blend_state_stack_pop`: the value to
/// restore, `None` when the stack is empty.
pub fn state_pop<T: Copy, S: Stack<T>>(stack: &mut S) -> Option<T> {
    let last = stack.len().checked_sub(1)?;
    let value = stack.get(last);
    stack.pop();
    value
}

/// `gs_blend_state_reset_steps`.
#[must_use]
pub fn blend_reset_steps(cur: &BlendState) -> BlendReset {
    let function = cur.src_c != blend::SRCALPHA
        || cur.dest_c != blend::INVSRCALPHA
        || cur.src_a != blend::ONE
        || cur.dest_a != blend::INVSRCALPHA;
    BlendReset {
        enable: !cur.enabled,
        function,
        // The op is part of the default state too (#2519).
        op: function || cur.op != blend::OP_ADD,
    }
}
