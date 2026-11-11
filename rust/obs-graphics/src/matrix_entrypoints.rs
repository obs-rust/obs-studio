//! Matrix entry-point orchestration: the `gs_valid` guard and `gs_matrix_pop`'s
//! error decision over a validated context's matrix stack. Storage and the
//! stack operations remain in the context implementation (state_stacks in the
//! shim).
//!
//! RED stub for #4575: the entry bodies delegate to the context verbs once the
//! port turns GREEN.
use crate::axisang::AxisAng;
use crate::matrix4::Matrix4;
use crate::quat::Quat;
use crate::state_stacks::MatrixOp;
use crate::vec3::Vec3;

/// A validated graphics context's matrix stack and the ordered operations on it.
pub trait Context {
    /// `gs_matrix_stack_push`: copy the top matrix.
    fn matrix_stack_push(&mut self);
    /// false when the top is the last matrix.
    fn matrix_stack_pop(&mut self) -> bool;
    /// The operations on the top, `gs_matrix_stack_identity` to
    /// `gs_matrix_stack_scale`.
    fn matrix_apply(&mut self, op: MatrixOp);
    /// `gs_matrix_stack_get`: the top, when the context has one.
    fn matrix_get(&self) -> Option<Matrix4>;
}

/// `gs_matrix_push`.
pub fn push<C: Context>(context: Option<&mut C>) {}
/// false when the stack held only the current matrix; the
/// caller logs `Tried to pop last matrix on stack`.
pub fn pop<C: Context>(context: Option<&mut C>) -> Option<bool> {
    None
}
/// `gs_matrix_stack_identity`.
pub fn identity<C: Context>(context: Option<&mut C>) {}
/// `gs_matrix_stack_transpose`.
pub fn transpose<C: Context>(context: Option<&mut C>) {}
/// `gs_matrix_stack_set`.
pub fn set<C: Context>(context: Option<&mut C>, matrix: Matrix4) {}
/// written to the caller's matrix when the context has a
/// top; the destination stays alone when it returns None.
pub fn get<C: Context>(context: Option<&C>) -> Option<Matrix4> {
    None
}
/// `gs_matrix_stack_mul`: the argument times the top.
pub fn mul<C: Context>(context: Option<&mut C>, matrix: Matrix4) {}
/// `gs_matrix_stack_rotquat`.
pub fn rotquat<C: Context>(context: Option<&mut C>, rot: Quat) {}
/// `gs_matrix_stack_rotaa`.
pub fn rotaa<C: Context>(context: Option<&mut C>, rot: AxisAng) {}
/// the scalar `axisang_set` form of `rotaa`.
pub fn rotaa4f<C: Context>(context: Option<&mut C>, x: f32, y: f32, z: f32, angle: f32) {}
/// `gs_matrix_stack_translate`.
pub fn translate<C: Context>(context: Option<&mut C>, pos: Vec3) {}
/// the scalar `vec3_set` form of `translate`.
pub fn translate3f<C: Context>(context: Option<&mut C>, x: f32, y: f32, z: f32) {}
/// `gs_matrix_stack_scale`.
pub fn scale<C: Context>(context: Option<&mut C>, factor: Vec3) {}
/// the scalar `vec3_set` form of `scale`.
pub fn scale3f<C: Context>(context: Option<&mut C>, x: f32, y: f32, z: f32) {}
