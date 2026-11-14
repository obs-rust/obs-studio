//! The matrix calls from `matrix-entrypoints.c.h`: `gs_matrix_push`,
//! `gs_matrix_pop`, `gs_matrix_identity`, `gs_matrix_transpose`,
//! `gs_matrix_set`, `gs_matrix_get`, `gs_matrix_mul`, `gs_matrix_rotquat`,
//! `gs_matrix_rotaa`, `gs_matrix_translate`, `gs_matrix_scale`,
//! `gs_matrix_rotaa4f`, `gs_matrix_translate3f` and `gs_matrix_scale3f`.
//!
//! Each call checks for a graphics context first. Without one it writes the
//! context's debug line under its own name and does nothing else: it reads
//! no argument and touches no stack, and `gs_matrix_get` leaves its output
//! alone. With one it works on the context's own matrix stack through the
//! [`state_stacks`] functions, as the C calls `gs_matrix_stack_*`. A pop on
//! the last matrix writes "Tried to pop last matrix on stack" at error
//! level and keeps it. The `4f` and `3f` forms build their axis-angle or
//! vector from the floats as `axisang_set` and `vec3_set` do.

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
use crate::state_stacks::{self, MatrixOp, Stack};
use crate::vec3::Vec3;
use std::ffi::CStr;

/// The debug line's reason, after the call's name.
pub const NOT_IN_CONTEXT: &str = "called while not in a graphics context";

/// `gs_matrix_pop`'s error line when the top is the last matrix.
pub const POP_LAST: &str = "Tried to pop last matrix on stack";

/// The fourteen calls, for the name in their debug line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entry {
    Push,
    Pop,
    Identity,
    Transpose,
    Set,
    Get,
    Mul,
    RotQuat,
    RotAa,
    Translate,
    Scale,
    RotAa4f,
    Translate3f,
    Scale3f,
}

impl Entry {
    /// Every call, in the order of the C file.
    pub const ALL: [Self; 14] = [
        Self::Push,
        Self::Pop,
        Self::Identity,
        Self::Transpose,
        Self::Set,
        Self::Get,
        Self::Mul,
        Self::RotQuat,
        Self::RotAa,
        Self::Translate,
        Self::Scale,
        Self::RotAa4f,
        Self::Translate3f,
        Self::Scale3f,
    ];

    /// The exported C name.
    pub fn name(self) -> &'static str {
        self.c_name().to_str().unwrap_or_default()
    }

    /// The exported C name, terminated, for the C debug line.
    pub const fn c_name(self) -> &'static CStr {
        match self {
            Self::Push => c"gs_matrix_push",
            Self::Pop => c"gs_matrix_pop",
            Self::Identity => c"gs_matrix_identity",
            Self::Transpose => c"gs_matrix_transpose",
            Self::Set => c"gs_matrix_set",
            Self::Get => c"gs_matrix_get",
            Self::Mul => c"gs_matrix_mul",
            Self::RotQuat => c"gs_matrix_rotquat",
            Self::RotAa => c"gs_matrix_rotaa",
            Self::Translate => c"gs_matrix_translate",
            Self::Scale => c"gs_matrix_scale",
            Self::RotAa4f => c"gs_matrix_rotaa4f",
            Self::Translate3f => c"gs_matrix_translate3f",
            Self::Scale3f => c"gs_matrix_scale3f",
        }
    }
}

/// The current thread's graphics context.
pub trait Host {
    /// The context's matrix stack.
    type Stack<'a>: Stack<Matrix4>
    where
        Self: 'a;

    /// The context's matrix stack and the index of its top
    /// (`graphics->matrix_stack` and `graphics->cur_matrix`), or `None`
    /// without a context. Asked once per call, before anything else.
    fn stack(&mut self) -> Option<(Self::Stack<'_>, &mut usize)>;
    /// Write `"<entry>: called while not in a graphics context"` at debug level.
    fn debug(&mut self, entry: Entry);
    /// Write [`POP_LAST`] at error level.
    fn pop_error(&mut self);
}

/// One operation on the top, after the context check. `arg` is read only
/// with a context, as the C reads its argument only then; `None` (a NULL
/// pointer) leaves the stack alone, as `gs_matrix_stack_*` do.
fn apply<H: Host>(host: &mut H, entry: Entry, op: impl FnOnce() -> Option<MatrixOp>) {
    let Some((mut stack, cur)) = host.stack() else {
        host.debug(entry);
        return;
    };
    if let Some(op) = op() {
        state_stacks::matrix_apply(&mut stack, *cur, op);
    }
}

pub fn push(host: &mut impl Host) {
    let Some((mut stack, cur)) = host.stack() else {
        host.debug(Entry::Push);
        return;
    };
    state_stacks::matrix_push(&mut stack, cur);
}

pub fn pop(host: &mut impl Host) {
    let Some((mut stack, cur)) = host.stack() else {
        host.debug(Entry::Pop);
        return;
    };
    let popped = state_stacks::matrix_pop(&mut stack, cur);
    drop(stack);
    if !popped {
        host.pop_error();
    }
}

pub fn identity(host: &mut impl Host) {
    apply(host, Entry::Identity, || Some(MatrixOp::Identity));
}

pub fn transpose(host: &mut impl Host) {
    apply(host, Entry::Transpose, || Some(MatrixOp::Transpose));
}

pub fn set(host: &mut impl Host, matrix: impl FnOnce() -> Option<Matrix4>) {
    apply(host, Entry::Set, || matrix().map(MatrixOp::Set));
}

/// The top, or `None` without a context (or an empty stack): the caller
/// then leaves its output alone.
pub fn get(host: &mut impl Host) -> Option<Matrix4> {
    let Some((stack, cur)) = host.stack() else {
        host.debug(Entry::Get);
        return None;
    };
    state_stacks::matrix_get(&stack, *cur)
}

pub fn mul(host: &mut impl Host, matrix: impl FnOnce() -> Option<Matrix4>) {
    apply(host, Entry::Mul, || matrix().map(MatrixOp::Mul));
}

pub fn rotquat(host: &mut impl Host, rot: impl FnOnce() -> Option<Quat>) {
    apply(host, Entry::RotQuat, || rot().map(MatrixOp::RotQuat));
}

pub fn rotaa(host: &mut impl Host, rot: impl FnOnce() -> Option<AxisAng>) {
    apply(host, Entry::RotAa, || rot().map(MatrixOp::RotAa));
}

pub fn translate(host: &mut impl Host, pos: impl FnOnce() -> Option<Vec3>) {
    apply(host, Entry::Translate, || pos().map(MatrixOp::Translate));
}

pub fn scale(host: &mut impl Host, scale: impl FnOnce() -> Option<Vec3>) {
    apply(host, Entry::Scale, || scale().map(MatrixOp::Scale));
}

/// `axisang_set(&aa, x, y, z, angle)`, then `gs_matrix_rotaa(&aa)`.
pub fn rotaa4f(host: &mut impl Host, x: f32, y: f32, z: f32, angle: f32) {
    apply(host, Entry::RotAa4f, || {
        Some(MatrixOp::RotAa(AxisAng::new(x, y, z, angle)))
    });
}

/// `vec3_set(&p, x, y, z)`, then `gs_matrix_translate(&p)`.
pub fn translate3f(host: &mut impl Host, x: f32, y: f32, z: f32) {
    apply(host, Entry::Translate3f, || {
        Some(MatrixOp::Translate(Vec3::new(x, y, z)))
    });
}

/// `vec3_set(&p, x, y, z)`, then `gs_matrix_scale(&p)`.
pub fn scale3f(host: &mut impl Host, x: f32, y: f32, z: f32) {
    apply(host, Entry::Scale3f, || {
        Some(MatrixOp::Scale(Vec3::new(x, y, z)))
    });
}
