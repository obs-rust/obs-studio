//! Tier 3: the shims against the original
//! `libobs/graphics/graphics-state-stacks.c.h`.
//!
//! The oracle (`rust/obs-c-oracle/oracle/graphics_state_stacks.c`) is the C
//! file, unmodified, with its functions renamed and its matrix4 calls on
//! the C graphics math oracle, so the C side is all C. Both get the same
//! calls through their C signatures on their own darrays, and are compared
//! after every call: the results, the matrix index, every element and the
//! capacity of the three darrays (the growth of `util/darray.h`), and the
//! reset steps.
//!
//! Floats are compared bit for bit, except that any NaN equals any NaN
//! (docs/rust-port/testing-policy.md, as in `linear::matrix4_parity`). The
//! stacks only copy the matrices; the NaN signs come from the matrix4 math,
//! where Rust does not fix the sign of a NaN that an operation makes (on
//! x86_64, `matrix4_rotate_aa_i` with an infinite axis gives +NaN in Rust and
//! -NaN in C). The blend states are compared by field, not by bytes, so
//! their padding is not read.
//!
//! Runs: the layouts; and 500 random sequences of up to 60 calls (push,
//! pop and every operation on the matrix stack with random matrices,
//! quaternions, axis-angles and vectors, including NaN and infinities;
//! push and pop of the viewport and blend state stacks; the reset steps
//! for random blend states).
//!
//! Excluded, the intentional differences (C dereferences null or reads past
//! its darray): null pointers, and an index past the stack (the C keeps the
//! index at the top). Those are tested on the shim alone below.

use core::ffi::c_void;
use core::ptr;

use obs_c_oracle::graphics_state_stacks as c;
use obs_graphics::ffi::axisang::axisang;
use obs_graphics::ffi::matrix4::matrix4;
use obs_graphics::ffi::quat::quat;
use obs_graphics::ffi::state_stacks::{
    gs_blend_state_reset_steps, gs_blend_state_stack_pop, gs_blend_state_stack_push,
    gs_matrix_stack_get, gs_matrix_stack_identity, gs_matrix_stack_mul, gs_matrix_stack_pop,
    gs_matrix_stack_push, gs_matrix_stack_rotaa, gs_matrix_stack_rotquat, gs_matrix_stack_scale,
    gs_matrix_stack_set, gs_matrix_stack_translate, gs_matrix_stack_transpose,
    gs_viewport_stack_pop, gs_viewport_stack_push,
};
use obs_graphics::ffi::vec3::vec3;
use obs_graphics::matrix4::Matrix4;
use obs_graphics::state_stacks::{BlendReset, BlendState, GsRect, blend, identity};
use obs_util::ffi::darray::{darray, darray_free, darray_push_back};
use proptest::prelude::*;

#[test]
fn layout() {
    let values = [
        size_of::<GsRect>(),
        size_of::<BlendState>(),
        core::mem::offset_of!(BlendState, src_c),
        core::mem::offset_of!(BlendState, dest_c),
        core::mem::offset_of!(BlendState, src_a),
        core::mem::offset_of!(BlendState, dest_a),
        core::mem::offset_of!(BlendState, op),
        size_of::<BlendReset>(),
        core::mem::offset_of!(BlendReset, function),
        core::mem::offset_of!(BlendReset, op),
        size_of::<matrix4>(),
        blend::SRCALPHA as usize,
        blend::INVSRCALPHA as usize,
        blend::OP_ADD as usize,
    ];
    for (which, (name, value)) in c::LAYOUT_NAMES.iter().zip(values).enumerate() {
        // SAFETY: a pure C function.
        assert_eq!(
            unsafe { c::oracle_gs_state_stacks_layout(which) },
            value,
            "{name}"
        );
    }
}

/// A float compared by the policy's rule: the same bits, or both NaN.
#[derive(Debug, Clone, Copy)]
struct Same(f32);

impl PartialEq for Same {
    fn eq(&self, other: &Self) -> bool {
        self.0.to_bits() == other.0.to_bits() || (self.0.is_nan() && other.0.is_nan())
    }
}

fn lanes(m: matrix4) -> [Same; 16] {
    // SAFETY: matrix4 is sixteen floats.
    unsafe { core::mem::transmute::<matrix4, [f32; 16]>(m) }.map(Same)
}

/// The three stacks of one side.
struct Side {
    matrices: darray,
    cur: usize,
    viewports: darray,
    blends: darray,
}

impl Side {
    fn new() -> Self {
        let mut s = Side {
            matrices: darray {
                array: ptr::null_mut(),
                num: 0,
                capacity: 0,
            },
            cur: 0,
            viewports: darray {
                array: ptr::null_mut(),
                num: 0,
                capacity: 0,
            },
            blends: darray {
                array: ptr::null_mut(),
                num: 0,
                capacity: 0,
            },
        };
        let id: matrix4 = identity().into();
        // SAFETY: an empty darray of matrix4 and one matrix4.
        unsafe {
            darray_push_back(
                size_of::<matrix4>(),
                &mut s.matrices,
                ptr::from_ref(&id).cast(),
            )
        };
        s
    }

    fn items<T: Copy>(da: &darray) -> Vec<T> {
        if da.array.is_null() {
            return Vec::new();
        }
        // SAFETY: `num` elements of `T`, written as `T` by both sides.
        unsafe { core::slice::from_raw_parts(da.array.cast::<T>(), da.num) }.to_vec()
    }

    #[allow(clippy::type_complexity)]
    fn state(
        &self,
    ) -> (
        usize,
        [usize; 3],
        Vec<[Same; 16]>,
        Vec<GsRect>,
        Vec<BlendState>,
    ) {
        let matrices = Self::items::<matrix4>(&self.matrices)
            .into_iter()
            .map(lanes)
            .collect();
        let capacities = [
            self.matrices.capacity,
            self.viewports.capacity,
            self.blends.capacity,
        ];
        (
            self.cur,
            capacities,
            matrices,
            Self::items(&self.viewports),
            Self::items(&self.blends),
        )
    }
}

impl Drop for Side {
    fn drop(&mut self) {
        // SAFETY: darrays from darray_push_back (bmalloc).
        unsafe {
            darray_free(&mut self.matrices);
            darray_free(&mut self.viewports);
            darray_free(&mut self.blends);
        }
    }
}

#[derive(Debug, Clone)]
enum Call {
    Push,
    Pop,
    Identity,
    Transpose,
    Set([f32; 16]),
    Get,
    Mul([f32; 16]),
    RotQuat([f32; 4]),
    RotAa([f32; 4]),
    Translate([f32; 3]),
    Scale([f32; 3]),
    ViewportPush([i32; 4]),
    ViewportPop,
    BlendPush(bool, [u32; 5]),
    BlendPop,
    Reset(bool, [u32; 5]),
}

fn float() -> impl Strategy<Value = f32> {
    prop_oneof![
        8 => -10.0f32..10.0,
        1 => Just(0.0f32),
        1 => prop_oneof![Just(f32::NAN), Just(f32::INFINITY), Just(-0.0f32), Just(f32::MAX), Just(1e-30f32)],
    ]
}

fn call() -> impl Strategy<Value = Call> {
    let enums =
        proptest::array::uniform5(prop_oneof![0u32..11, Just(4u32), Just(5u32), Just(1u32)]);
    prop_oneof![
        4 => Just(Call::Push),
        4 => Just(Call::Pop),
        1 => Just(Call::Identity),
        1 => Just(Call::Transpose),
        2 => proptest::array::uniform16(float()).prop_map(Call::Set),
        2 => Just(Call::Get),
        2 => proptest::array::uniform16(float()).prop_map(Call::Mul),
        2 => proptest::array::uniform4(float()).prop_map(Call::RotQuat),
        2 => proptest::array::uniform4(float()).prop_map(Call::RotAa),
        2 => proptest::array::uniform3(float()).prop_map(Call::Translate),
        2 => proptest::array::uniform3(float()).prop_map(Call::Scale),
        2 => any::<[i32; 4]>().prop_map(Call::ViewportPush),
        2 => Just(Call::ViewportPop),
        2 => (any::<bool>(), enums.clone()).prop_map(|(e, v)| Call::BlendPush(e, v)),
        2 => Just(Call::BlendPop),
        2 => (any::<bool>(), enums).prop_map(|(e, v)| Call::Reset(e, v)),
    ]
}

fn blend_state(enabled: bool, v: [u32; 5]) -> BlendState {
    BlendState {
        enabled,
        src_c: v[0],
        dest_c: v[1],
        src_a: v[2],
        dest_a: v[3],
        op: v[4] % 5,
    }
}

fn m4(f: [f32; 16]) -> matrix4 {
    // SAFETY: matrix4 is sixteen floats.
    unsafe { core::mem::transmute::<[f32; 16], matrix4>(f) }
}

// 500 cases (5000 before #2891). The 15 mutants of the core and the shim
// (rebuilt after #2528) each fail here within 125 cases in every run; the
// equivalent one (no max(new_size) in the growth) survives, as before.
proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(500)))]

    #[test]
    fn sequences(calls in proptest::collection::vec(call(), 0..60)) {
        let (mut r, mut cs) = (Side::new(), Side::new());
        for call in &calls {
            let (rm, cm) = (&raw mut r.matrices, (&raw mut cs.matrices).cast::<c_void>());
            let (rc, cc) = (r.cur, cs.cur);
            // SAFETY: valid darrays and indices; the values live for the call.
            unsafe {
                match *call {
                    Call::Push => {
                        gs_matrix_stack_push(rm, &mut r.cur);
                        c::oracle_gs_matrix_stack_push(cm, &mut cs.cur);
                    }
                    Call::Pop => {
                        let (a, b) = (gs_matrix_stack_pop(rm, &mut r.cur), c::oracle_gs_matrix_stack_pop(cm, &mut cs.cur));
                        prop_assert_eq!(a, b);
                    }
                    Call::Identity => {
                        gs_matrix_stack_identity(rm, rc);
                        c::oracle_gs_matrix_stack_identity(cm, cc);
                    }
                    Call::Transpose => {
                        gs_matrix_stack_transpose(rm, rc);
                        c::oracle_gs_matrix_stack_transpose(cm, cc);
                    }
                    Call::Set(f) => {
                        let m = m4(f);
                        gs_matrix_stack_set(rm, rc, &m);
                        c::oracle_gs_matrix_stack_set(cm, cc, (&raw const m).cast());
                    }
                    Call::Get => {
                        let (mut a, mut b) = (m4([0.5; 16]), m4([0.5; 16]));
                        gs_matrix_stack_get(rm, rc, &mut a);
                        c::oracle_gs_matrix_stack_get(cm, cc, (&raw mut b).cast());
                        prop_assert_eq!(lanes(a), lanes(b));
                    }
                    Call::Mul(f) => {
                        let m = m4(f);
                        gs_matrix_stack_mul(rm, rc, &m);
                        c::oracle_gs_matrix_stack_mul(cm, cc, (&raw const m).cast());
                    }
                    Call::RotQuat(f) => {
                        let q = core::mem::transmute::<[f32; 4], quat>(f);
                        gs_matrix_stack_rotquat(rm, rc, &q);
                        c::oracle_gs_matrix_stack_rotquat(cm, cc, (&raw const q).cast());
                    }
                    Call::RotAa(f) => {
                        let aa = core::mem::transmute::<[f32; 4], axisang>(f);
                        gs_matrix_stack_rotaa(rm, rc, &aa);
                        c::oracle_gs_matrix_stack_rotaa(cm, cc, (&raw const aa).cast());
                    }
                    Call::Translate(f) | Call::Scale(f) => {
                        let v = vec3 { x: f[0], y: f[1], z: f[2], w: 0.0 };
                        if matches!(call, Call::Translate(_)) {
                            gs_matrix_stack_translate(rm, rc, &v);
                            c::oracle_gs_matrix_stack_translate(cm, cc, (&raw const v).cast());
                        } else {
                            gs_matrix_stack_scale(rm, rc, &v);
                            c::oracle_gs_matrix_stack_scale(cm, cc, (&raw const v).cast());
                        }
                    }
                    Call::ViewportPush([x, y, cx, cy]) => {
                        let rect = GsRect { x, y, cx, cy };
                        gs_viewport_stack_push(&mut r.viewports, &rect);
                        c::oracle_gs_viewport_stack_push((&raw mut cs.viewports).cast(), (&raw const rect).cast());
                    }
                    Call::ViewportPop => {
                        let (mut a, mut b) = (GsRect { x: 7, ..GsRect::default() }, GsRect { x: 7, ..GsRect::default() });
                        let ra = gs_viewport_stack_pop(&mut r.viewports, &mut a);
                        let rb = c::oracle_gs_viewport_stack_pop((&raw mut cs.viewports).cast(), (&raw mut b).cast());
                        prop_assert_eq!((ra, a), (rb, b));
                    }
                    Call::BlendPush(e, v) => {
                        let st = blend_state(e, v);
                        gs_blend_state_stack_push(&mut r.blends, &st);
                        c::oracle_gs_blend_state_stack_push((&raw mut cs.blends).cast(), (&raw const st).cast());
                    }
                    Call::BlendPop => {
                        let (mut a, mut b) = (BlendState::default(), BlendState::default());
                        let ra = gs_blend_state_stack_pop(&mut r.blends, &mut a);
                        let rb = c::oracle_gs_blend_state_stack_pop((&raw mut cs.blends).cast(), (&raw mut b).cast());
                        prop_assert_eq!((ra, a), (rb, b));
                    }
                    Call::Reset(e, v) => {
                        let st = blend_state(e, v);
                        let a = gs_blend_state_reset_steps(&st);
                        let b = c::oracle_gs_blend_state_reset_steps((&raw const st).cast());
                        prop_assert_eq!((a.enable, a.function, a.op), (b.enable, b.function, b.op));
                    }
                }
            }
            prop_assert_eq!(r.state(), cs.state(), "after {:?}", call);
        }
    }
}

// ---------------------------------------------------------------------------
// The intentional differences, on the shim alone.

#[test]
fn shim_null_pointers_and_far_index() {
    let mut s = Side::new();
    let m: matrix4 = Matrix4::default().into();
    let mut out = m;
    // SAFETY: every non-null pointer is valid; the nulls are the test.
    unsafe {
        gs_matrix_stack_push(ptr::null_mut(), &mut s.cur);
        gs_matrix_stack_push(&mut s.matrices, ptr::null_mut());
        assert!(!gs_matrix_stack_pop(ptr::null_mut(), &mut s.cur));
        gs_matrix_stack_set(&mut s.matrices, 0, ptr::null());
        gs_matrix_stack_get(ptr::null(), 0, &mut out);
        gs_matrix_stack_get(&s.matrices, 0, ptr::null_mut());
        assert!(!gs_viewport_stack_pop(&mut s.viewports, ptr::null_mut()));
        assert!(!gs_blend_state_stack_pop(ptr::null_mut(), ptr::null_mut()));
        assert_eq!(
            gs_blend_state_reset_steps(ptr::null()),
            BlendReset::default()
        );
        // An index past the stack reads and changes nothing.
        gs_matrix_stack_set(&mut s.matrices, 9, &m);
        gs_matrix_stack_get(&s.matrices, 9, &mut out);
    }
    assert_eq!(s.matrices.num, 1);
    assert_eq!(s.cur, 0);
}

#[test]
fn darray_stack_boundaries() {
    use obs_graphics::ffi::state_stacks::DarrayStack;
    use obs_graphics::state_stacks::Stack;

    let rect = |i: i32| GsRect {
        x: i,
        y: -i,
        cx: 10 * i,
        cy: 20 * i,
    };
    let mut da = darray {
        array: ptr::null_mut(),
        num: 0,
        capacity: 0,
    };
    {
        // SAFETY: an empty darray of GsRect.
        let mut s = unsafe { DarrayStack::<GsRect>::new(&mut da) };
        for i in 0..4 {
            s.push(rect(i));
        }
        // A middle element: the ones after it move down.
        s.erase(1);
        assert_eq!(s.len(), 3);
        assert_eq!(
            (s.get(0), s.get(1), s.get(2)),
            (Some(rect(0)), Some(rect(2)), Some(rect(3)))
        );
        // The last slot keeps rect(3) in the spare capacity: still not read.
        s.pop();
        assert_eq!((s.len(), s.get(2)), (2, None));
        s.set(2, rect(9));
        assert_eq!(s.get(2), None);
    }
    // SAFETY: from darray_push_back.
    unsafe { darray_free(&mut da) };
}
