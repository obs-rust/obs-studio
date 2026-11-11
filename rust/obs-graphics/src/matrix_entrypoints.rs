//! Matrix entry-point orchestration: the `gs_valid` guard and `gs_matrix_pop`'s
//! error decision over a validated context's matrix stack. Storage and the
//! stack operations remain in the context implementation (state_stacks in the
//! shim).
//!
//! The entries apply to the current top of a validated context's matrix stack;
//! each is a no-op outside a graphics context (`gs_valid`), and `pop` reports
//! the `Tried to pop last matrix on stack` decision to its caller. Storage and
//! the stack operations remain in the context implementation (state_stacks in
//! the shim).
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
pub fn push<C: Context>(context: Option<&mut C>) {
    let Some(context) = context else { return };
    context.matrix_stack_push()
}
/// false when the stack held only the current matrix; the
/// caller logs `Tried to pop last matrix on stack`.
pub fn pop<C: Context>(context: Option<&mut C>) -> Option<bool> {
    let Some(context) = context else { return None };
    Some(context.matrix_stack_pop())
}
/// `gs_matrix_stack_identity`.
pub fn identity<C: Context>(context: Option<&mut C>) {
    let Some(context) = context else { return };
    context.matrix_apply(MatrixOp::Identity)
}
/// `gs_matrix_stack_transpose`.
pub fn transpose<C: Context>(context: Option<&mut C>) {
    let Some(context) = context else { return };
    context.matrix_apply(MatrixOp::Transpose)
}
/// `gs_matrix_stack_set`.
pub fn set<C: Context>(context: Option<&mut C>, matrix: Matrix4) {
    let Some(context) = context else { return };
    context.matrix_apply(MatrixOp::Set(matrix))
}
/// written to the caller's matrix when the context has a
/// top; the destination stays alone when it returns None.
pub fn get<C: Context>(context: Option<&C>) -> Option<Matrix4> {
    let Some(context) = context else { return None };
    context.matrix_get()
}
/// `gs_matrix_stack_mul`: the argument times the top.
pub fn mul<C: Context>(context: Option<&mut C>, matrix: Matrix4) {
    let Some(context) = context else { return };
    context.matrix_apply(MatrixOp::Mul(matrix))
}
/// `gs_matrix_stack_rotquat`.
pub fn rotquat<C: Context>(context: Option<&mut C>, rot: Quat) {
    let Some(context) = context else { return };
    context.matrix_apply(MatrixOp::RotQuat(rot))
}
/// `gs_matrix_stack_rotaa`.
pub fn rotaa<C: Context>(context: Option<&mut C>, rot: AxisAng) {
    let Some(context) = context else { return };
    context.matrix_apply(MatrixOp::RotAa(rot))
}
/// the scalar `axisang_set` form of `rotaa`.
pub fn rotaa4f<C: Context>(context: Option<&mut C>, x: f32, y: f32, z: f32, angle: f32) {
    let Some(context) = context else { return };
    context.matrix_apply(MatrixOp::RotAa(AxisAng::new(x, y, z, angle)))
}
/// `gs_matrix_stack_translate`.
pub fn translate<C: Context>(context: Option<&mut C>, pos: Vec3) {
    let Some(context) = context else { return };
    context.matrix_apply(MatrixOp::Translate(pos))
}
/// the scalar `vec3_set` form of `translate`.
pub fn translate3f<C: Context>(context: Option<&mut C>, x: f32, y: f32, z: f32) {
    let Some(context) = context else { return };
    context.matrix_apply(MatrixOp::Translate(Vec3::new(x, y, z)))
}
/// `gs_matrix_stack_scale`.
pub fn scale<C: Context>(context: Option<&mut C>, factor: Vec3) {
    let Some(context) = context else { return };
    context.matrix_apply(MatrixOp::Scale(factor))
}
/// the scalar `vec3_set` form of `scale`.
pub fn scale3f<C: Context>(context: Option<&mut C>, x: f32, y: f32, z: f32) {
    let Some(context) = context else { return };
    context.matrix_apply(MatrixOp::Scale(Vec3::new(x, y, z)))
}
