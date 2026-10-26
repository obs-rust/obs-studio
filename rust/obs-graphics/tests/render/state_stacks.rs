//! Tier 1: the safe core of `libobs/graphics/graphics-state-stacks.c.h`.
//!
//! The cases mirror `test/cmocka/test_graphics_state_stacks.c` one to one,
//! on `Vec` stacks. The proptests check the depth after any push and pop
//! sequence, and that the top after any operation sequence equals the same
//! `Matrix4` steps.

use obs_graphics::axisang::AxisAng;
use obs_graphics::matrix4::Matrix4;
use obs_graphics::quat::Quat;
use obs_graphics::state_stacks::{
    self, BlendState, GsRect, MatrixOp, blend, blend_reset_steps, identity, matrix_apply,
    matrix_get, matrix_pop, matrix_push, state_pop, state_push,
};
use obs_graphics::vec3::Vec3;
use obs_graphics::vec4::Vec4;
use proptest::prelude::*;

/// A matrix with sixteen different values (the cmocka test_matrix).
fn test_matrix(base: f32) -> Matrix4 {
    let f = |i: i32| base + i as f32 * 0.25 - (i % 3) as f32;
    let row = |r: i32| Vec4::new(f(r * 4), f(r * 4 + 1), f(r * 4 + 2), f(r * 4 + 3));
    Matrix4::new(row(0), row(1), row(2), row(3))
}

fn stack() -> (Vec<Matrix4>, usize) {
    (vec![identity()], 0)
}

#[test]
fn matrix_push_pop() {
    let (mut s, mut cur) = stack();
    let a = test_matrix(1.0);
    matrix_apply(&mut s, cur, MatrixOp::Set(a));
    for depth in 1..=5 {
        matrix_push(&mut s, &mut cur);
        assert_eq!((cur, s.len()), (depth, depth + 1));
        assert_eq!(s[cur], s[cur - 1]);
    }
    let b = test_matrix(-3.0);
    matrix_apply(&mut s, cur, MatrixOp::Set(b));
    assert_eq!(s[cur - 1], a);
    for depth in (1..=5).rev() {
        assert!(matrix_pop(&mut s, &mut cur));
        assert_eq!((cur, s.len()), (depth - 1, depth));
    }
    assert_eq!(s[0], a);
    assert!(!matrix_pop(&mut s, &mut cur));
    assert_eq!((cur, s.len()), (0, 1));
}

#[test]
fn matrix_operations() {
    let start = test_matrix(2.0);
    let arg = test_matrix(-1.5);
    let q = Quat::new(0.2, -0.4, 0.1, 0.88);
    let aa = AxisAng::new(0.3, 0.5, -0.8, 1.25);
    let v = Vec3::new(1.5, -2.0, 0.75);
    let ops: [(Option<MatrixOp>, Matrix4); 9] = [
        (Some(MatrixOp::Identity), identity()),
        (Some(MatrixOp::Transpose), start.transpose()),
        (Some(MatrixOp::Set(arg)), arg),
        (Some(MatrixOp::Mul(arg)), arg * &start),
        (Some(MatrixOp::RotQuat(q)), start.rotate_i(q)),
        (Some(MatrixOp::RotAa(aa)), start.rotate_aa_i(aa)),
        (Some(MatrixOp::Translate(v)), start.translate3v_i(v)),
        (Some(MatrixOp::Scale(v)), start.scale_i(v)),
        (None, start),
    ];
    for depth in 0..3 {
        for (op, expect) in ops {
            let (mut s, mut cur) = stack();
            for _ in 0..depth {
                matrix_push(&mut s, &mut cur);
            }
            matrix_apply(&mut s, cur, MatrixOp::Set(start));
            if let Some(op) = op {
                matrix_apply(&mut s, cur, op);
            }
            assert_eq!(matrix_get(&s, cur), Some(expect), "{op:?} at depth {depth}");
        }
    }
}

#[test]
fn viewport_stack() {
    let mut s: Vec<GsRect> = Vec::new();
    assert_eq!(state_pop(&mut s), None);
    for i in 0..4 {
        state_push(
            &mut s,
            GsRect {
                x: i,
                y: i * 2,
                cx: 100 + i,
                cy: 50 + i,
            },
        );
    }
    for i in (0..4).rev() {
        assert_eq!(
            state_pop(&mut s),
            Some(GsRect {
                x: i,
                y: i * 2,
                cx: 100 + i,
                cy: 50 + i
            })
        );
    }
    assert!(s.is_empty());
    assert_eq!(state_pop(&mut s), None);
}

#[test]
fn blend_state_stack() {
    let mut s: Vec<BlendState> = Vec::new();
    assert_eq!(state_pop(&mut s), None);
    let state = |i: u32| BlendState {
        enabled: i.is_multiple_of(2),
        src_c: i,
        dest_c: i + 1,
        src_a: i + 2,
        dest_a: i + 3,
        op: i % 5,
    };
    for i in 0..3 {
        state_push(&mut s, state(i));
    }
    for i in (0..3).rev() {
        assert_eq!(state_pop(&mut s), Some(state(i)));
    }
    assert_eq!(state_pop(&mut s), None);
}

const DEFAULT: BlendState = BlendState {
    enabled: true,
    src_c: blend::SRCALPHA,
    dest_c: blend::INVSRCALPHA,
    src_a: blend::ONE,
    dest_a: blend::INVSRCALPHA,
    op: blend::OP_ADD,
};

#[test]
fn blend_reset_steps_cases() {
    let steps = |s: BlendState| {
        let r = blend_reset_steps(&s);
        (r.enable, r.function, r.op)
    };
    assert_eq!(steps(DEFAULT), (false, false, false));
    assert_eq!(
        steps(BlendState {
            enabled: false,
            ..DEFAULT
        }),
        (true, false, false)
    );
    for field in 0..4 {
        let mut s = DEFAULT;
        *[&mut s.src_c, &mut s.dest_c, &mut s.src_a, &mut s.dest_a][field] = blend::ZERO;
        assert_eq!(steps(s), (false, true, true), "field {field}");
    }
    let subtract = BlendState {
        src_c: 1,
        dest_c: 1,
        src_a: 1,
        dest_a: 1,
        op: 2,
        ..DEFAULT
    };
    assert_eq!(steps(subtract), (false, true, true));
    // The default function with another op: the op is reset too (#2519).
    for op in 1..=4 {
        assert_eq!(
            steps(BlendState { op, ..DEFAULT }),
            (false, false, true),
            "op {op}"
        );
    }
}

// ---------------------------------------------------------------------------
// Edges of the safe API.

#[test]
fn edge_index_past_the_stack_changes_nothing() {
    // The C reads past its darray there; gs_matrix_* never get there (cur
    // is the last index).
    let (mut s, mut cur) = stack();
    let mut far = 7;
    matrix_apply(&mut s, far, MatrixOp::Identity);
    assert_eq!(matrix_get(&s, far), None);
    matrix_push(&mut s, &mut far);
    assert_eq!((s.len(), far), (1, 7));
    assert!(!matrix_pop(&mut s, &mut cur));
}

// ---------------------------------------------------------------------------
// Properties.

fn op() -> impl Strategy<Value = MatrixOp> {
    let f = -4.0f32..4.0;
    prop_oneof![
        Just(MatrixOp::Identity),
        Just(MatrixOp::Transpose),
        (f.clone()).prop_map(|b| MatrixOp::Set(test_matrix(b))),
        (f.clone()).prop_map(|b| MatrixOp::Mul(test_matrix(b))),
        (f.clone(), f.clone(), f.clone(), f.clone())
            .prop_map(|(x, y, z, w)| MatrixOp::RotQuat(Quat::new(x, y, z, w))),
        (f.clone(), f.clone(), f.clone(), f.clone())
            .prop_map(|(x, y, z, w)| MatrixOp::RotAa(AxisAng::new(x, y, z, w))),
        (f.clone(), f.clone(), f.clone())
            .prop_map(|(x, y, z)| MatrixOp::Translate(Vec3::new(x, y, z))),
        (f.clone(), f.clone(), f).prop_map(|(x, y, z)| MatrixOp::Scale(Vec3::new(x, y, z))),
    ]
}

fn reference(m: Matrix4, op: MatrixOp) -> Matrix4 {
    match op {
        MatrixOp::Identity => identity(),
        MatrixOp::Transpose => m.transpose(),
        MatrixOp::Set(a) => a,
        MatrixOp::Mul(a) => a * &m,
        MatrixOp::RotQuat(q) => m.rotate_i(q),
        MatrixOp::RotAa(aa) => m.rotate_aa_i(aa),
        MatrixOp::Translate(v) => m.translate3v_i(v),
        MatrixOp::Scale(v) => m.scale_i(v),
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    #[test]
    fn prop_depth_and_top(steps in proptest::collection::vec(prop_oneof![
        3 => op().prop_map(Some),
        1 => Just(None::<MatrixOp>),
    ], 0..40), pushes in proptest::collection::vec(any::<bool>(), 0..40)) {
        let (mut s, mut cur) = stack();
        // A model: a Vec of matrices, the top is the last.
        let mut model = vec![identity()];
        for (step, push) in steps.into_iter().zip(pushes) {
            match step {
                Some(op) => {
                    matrix_apply(&mut s, cur, op);
                    let top = model.pop().unwrap_or_else(identity);
                    model.push(reference(top, op));
                }
                None if push => {
                    matrix_push(&mut s, &mut cur);
                    let top = *model.last().unwrap_or(&identity());
                    model.push(top);
                }
                None => {
                    let popped = matrix_pop(&mut s, &mut cur);
                    prop_assert_eq!(popped, model.len() > 1);
                    if model.len() > 1 {
                        model.pop();
                    }
                }
            }
            prop_assert_eq!(cur + 1, s.len());
            prop_assert_eq!(&s, &model);
        }
    }
}

#[allow(dead_code)]
fn unused(_: state_stacks::BlendReset) {}
