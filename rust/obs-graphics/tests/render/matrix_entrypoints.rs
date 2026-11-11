//! Tier 1 mirrors test/cmocka/test_matrix_entrypoints.c, with no FFI or unsafe
//! code: the Mock supplies the context verbs straight from state_stacks.
use obs_graphics::matrix_entrypoints::{self as entry, Context};
use obs_graphics::state_stacks::{self, MatrixOp};
use obs_graphics::matrix4::Matrix4;
use obs_graphics::vec4::Vec4;
use obs_graphics::vec3::Vec3;
use obs_graphics::quat::Quat;
use obs_graphics::axisang::AxisAng;

fn component(base: f32, i: usize) -> f32 {
    base + i as f32 * 0.25 - (i % 3) as f32
}
fn row(base: f32, n: usize) -> Vec4 {
    let i = n * 4;
    Vec4::new(component(base, i), component(base, i + 1), component(base, i + 2), component(base, i + 3))
}
fn matrix(base: f32) -> Matrix4 {
    Matrix4::new(row(base, 0), row(base, 1), row(base, 2), row(base, 3))
}
/// The expected top after applying an operation, mirroring matrix_apply.
fn expected_apply(top: Matrix4, op: MatrixOp) -> Matrix4 {
    match op {
        MatrixOp::Identity => state_stacks::identity(),
        MatrixOp::Transpose => top.transpose(),
        MatrixOp::Set(matrix) => matrix,
        MatrixOp::Mul(matrix) => core::ops::Mul::mul(matrix, &top),
        MatrixOp::RotQuat(rot) => top.rotate_i(rot),
        MatrixOp::RotAa(rot) => top.rotate_aa_i(rot),
        MatrixOp::Translate(pos) => top.translate3v_i(pos),
        MatrixOp::Scale(factor) => top.scale_i(factor),
    }
}
#[derive(Default)]
pub struct Mock {
    pub cur: usize,
    pub stack: Vec<Matrix4>,
}
impl Context for Mock {
    fn matrix_stack_push(&mut self) {
        state_stacks::matrix_push(&mut self.stack, &mut self.cur);
    }
    fn matrix_stack_pop(&mut self) -> bool {
        state_stacks::matrix_pop(&mut self.stack, &mut self.cur)
    }
    fn matrix_apply(&mut self, op: MatrixOp) {
        state_stacks::matrix_apply(&mut self.stack, self.cur, op);
    }
    fn matrix_get(&self) -> Option<Matrix4> {
        state_stacks::matrix_get(&self.stack, self.cur)
    }
}
/// A graphics context starts with the identity matrix as its only stack entry.
fn base() -> Mock {
    let mut m = Mock { cur: 0, stack: Vec::<Matrix4>::new() };
    m.stack.push(state_stacks::identity());
    m
}
#[test]
fn test_matrix_entrypoints_no_context() {
    entry::push::<Mock>(None);
    assert_eq!(entry::pop::<Mock>(None), None);
    entry::identity::<Mock>(None);
    entry::transpose::<Mock>(None);
    entry::set::<Mock>(None, matrix(1.0));
    assert_eq!(entry::get::<Mock>(None), None);
    entry::mul::<Mock>(None, matrix(2.0));
    entry::rotquat::<Mock>(None, Quat::new(0.0, 1.0, 0.0, 0.0));
    entry::rotaa::<Mock>(None, AxisAng::new(0.0, 1.0, 0.0, 0.5));
    entry::rotaa4f::<Mock>(None, 0.0, 1.0, 0.0, 0.5);
    entry::translate::<Mock>(None, Vec3::new(1.0, 2.0, 3.0));
    entry::translate3f::<Mock>(None, 1.0, 2.0, 3.0);
    entry::scale::<Mock>(None, Vec3::new(1.0, 1.0, 1.0));
    entry::scale3f::<Mock>(None, 1.0, 1.0, 1.0);
    let mut m = base();
    assert_eq!(m.stack.len(), 1);
    assert_eq!(m.cur, 0);
}
#[test]
fn test_matrix_entrypoints_stack_depth() {
    let mut m = base();
    for depth in 1..4 {
        entry::push(Some(&mut m));
        let a = matrix(depth as f32);
        entry::set(Some(&mut m), a);
        assert_eq!(entry::get(Some(&m)), Some(a));
        assert_eq!(m.stack.len(), depth + 1);
        assert_eq!(m.cur, depth);
    }
    while m.cur > 0 {
        assert_eq!(entry::pop(Some(&mut m)), Some(true));
    }
    assert_eq!(entry::pop(Some(&mut m)), Some(false));
    assert_eq!(m.stack.len(), 1);
    assert_eq!(entry::get(Some(&m)), Some(state_stacks::identity()));
}
#[test]
fn test_matrix_entrypoints_pop_last_matrix() {
    let mut m = base();
    assert_eq!(entry::pop(Some(&mut m)), Some(false));
    assert_eq!(entry::pop(Some(&mut m)), Some(false));
    assert_eq!(m.cur, 0);
    entry::push(Some(&mut m));
    assert_eq!(entry::pop(Some(&mut m)), Some(true));
    assert_eq!(m.cur, 0);
    assert_eq!(m.stack.len(), 1);
}
#[test]
fn test_matrix_entrypoints_operations_on_top() {
    let arg = matrix(10.0);
    let q = Quat::new(0.1, 0.2, 0.3, 0.9);
    let aa = AxisAng::new(0.0, 1.0, 0.0, 0.5);
    let pos = Vec3::new(1.0, 2.0, 3.0);
    let factor = Vec3::new(2.0, 0.5, 1.0);
    for depth in 0..3 {
        let mut m = base();
        for _ in 0..depth {
            entry::push(Some(&mut m));
        }
        let start = matrix(depth as f32 + 1.0);
        entry::set(Some(&mut m), start);
        let mut top = start;
        entry::identity(Some(&mut m));
        top = expected_apply(top, MatrixOp::Identity);
        assert_eq!(m.stack[m.cur], top);
        entry::transpose(Some(&mut m));
        top = expected_apply(top, MatrixOp::Transpose);
        assert_eq!(m.stack[m.cur], top);
        entry::mul(Some(&mut m), arg);
        top = expected_apply(top, MatrixOp::Mul(arg));
        assert_eq!(m.stack[m.cur], top);
        entry::set(Some(&mut m), top);
        assert_eq!(m.stack[m.cur], top);
        entry::rotquat(Some(&mut m), q);
        top = expected_apply(top, MatrixOp::RotQuat(q));
        assert_eq!(m.stack[m.cur], top);
        entry::rotaa(Some(&mut m), aa);
        top = expected_apply(top, MatrixOp::RotAa(aa));
        assert_eq!(m.stack[m.cur], top);
        entry::translate(Some(&mut m), pos);
        top = expected_apply(top, MatrixOp::Translate(pos));
        assert_eq!(m.stack[m.cur], top);
        entry::scale(Some(&mut m), factor);
        top = expected_apply(top, MatrixOp::Scale(factor));
        assert_eq!(m.stack[m.cur], top);
        assert_eq!(m.cur, depth);
    }
}
#[test]
fn test_matrix_entrypoints_scalar_parity() {
    let mut m = base();
    let mut n = base();
    entry::rotaa4f(Some(&mut m), 0.0, 1.0, 0.0, 0.75);
    entry::rotaa(Some(&mut n), AxisAng::new(0.0, 1.0, 0.0, 0.75));
    assert_eq!(m.stack, n.stack);
    entry::translate3f(Some(&mut m), 1.0, 2.0, 3.0);
    entry::translate(Some(&mut n), Vec3::new(1.0, 2.0, 3.0));
    assert_eq!(m.stack, n.stack);
    entry::scale3f(Some(&mut m), 2.0, 0.5, 1.0);
    entry::scale(Some(&mut n), Vec3::new(2.0, 0.5, 1.0));
    assert_eq!(m.stack, n.stack);
}
