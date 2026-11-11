//! Tier 3: the original C entry orchestration against the production Rust
//! shims, plus the safe core. Both entry sides share one physical stack:
//! the Rust shim validates the context and adapts the very darray the C
//! fields adapter exposes (`matrix-entrypoints-fields.inc`), so every
//! snapshot reads identical storage. Floats compare by the policy's rule:
//! the same bits, or both NaN. Runs: the entry sequences with context
//! toggling (including the not-in-a-context log for every entry and the
//! pop-last LOG_ERROR), and 100 random sequences of up to 60 calls.

use super::matrix_entrypoints::Mock;
use obs_graphics::axisang::AxisAng;
use obs_graphics::ffi::matrix_entrypoints as ffi;
use obs_graphics::ffi::{axisang::axisang, matrix4::matrix4, quat::quat, vec3::vec3};
use obs_graphics::matrix_entrypoints as core;
use obs_graphics::matrix4::Matrix4;
use obs_graphics::quat::Quat;
use obs_graphics::state_stacks::identity;
use obs_graphics::vec3::Vec3;
use obs_graphics::vec4::Vec4;
use proptest::prelude::*;
use std::ffi::{CStr, c_char};

unsafe extern "C" {
    fn oracle_gs_matrix_push();
    fn oracle_gs_matrix_pop();
    fn oracle_gs_matrix_identity();
    fn oracle_gs_matrix_transpose();
    fn oracle_gs_matrix_get(dst: *mut matrix4);
    fn oracle_gs_matrix_set(matrix: *const matrix4);
    fn oracle_gs_matrix_mul(matrix: *const matrix4);
    fn oracle_gs_matrix_rotquat(rot: *const quat);
    fn oracle_gs_matrix_rotaa(rot: *const axisang);
    fn oracle_gs_matrix_rotaa4f(x: f32, y: f32, z: f32, angle: f32);
    fn oracle_gs_matrix_translate(pos: *const vec3);
    fn oracle_gs_matrix_translate3f(x: f32, y: f32, z: f32);
    fn oracle_gs_matrix_scale(factor: *const vec3);
    fn oracle_gs_matrix_scale3f(x: f32, y: f32, z: f32);
    fn oracle_matrix_begin(present: bool, initial: *const matrix4);
    fn oracle_matrix_set(present: bool);
    fn oracle_matrix_end();
    fn oracle_matrix_logs() -> u32;
    fn oracle_matrix_message() -> *const c_char;
    fn oracle_matrix_depth() -> usize;
    fn oracle_matrix_current() -> matrix4;
    fn oracle_matrix_saved(index: usize) -> matrix4;
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
    unsafe { std::mem::transmute::<matrix4, [f32; 16]>(m) }.map(Same)
}

/// The safe Matrix4 as its sixteen public lanes, in the same row order the
/// C struct lays out.
fn safe_lanes(m: Matrix4) -> [Same; 16] {
    [
        m.x.x, m.x.y, m.x.z, m.x.w, m.y.x, m.y.y, m.y.z, m.y.w, m.z.x, m.z.y, m.z.z, m.z.w, m.t.x,
        m.t.y, m.t.z, m.t.w,
    ]
    .map(Same)
}

fn m4(f: [f32; 16]) -> matrix4 {
    // SAFETY: matrix4 is sixteen floats.
    unsafe { std::mem::transmute::<[f32; 16], matrix4>(f) }
}

fn safe_m4(f: [f32; 16]) -> Matrix4 {
    Matrix4 {
        x: Vec4 {
            x: f[0],
            y: f[1],
            z: f[2],
            w: f[3],
        },
        y: Vec4 {
            x: f[4],
            y: f[5],
            z: f[6],
            w: f[7],
        },
        z: Vec4 {
            x: f[8],
            y: f[9],
            z: f[10],
            w: f[11],
        },
        t: Vec4 {
            x: f[12],
            y: f[13],
            z: f[14],
            w: f[15],
        },
    }
}

#[derive(Debug, Clone, Copy)]
enum Call {
    Push,
    Pop,
    Identity,
    Transpose,
    Get,
    Set([f32; 16]),
    Mul([f32; 16]),
    RotQuat([f32; 4]),
    RotAa([f32; 4]),
    RotAa4f([f32; 4]),
    Translate([f32; 3]),
    Translate3f([f32; 3]),
    Scale([f32; 3]),
    Scale3f([f32; 3]),
}

#[derive(Debug, PartialEq)]
struct Snapshot {
    current: [Same; 16],
    stack: Vec<[Same; 16]>,
    logs: u32,
    message: String,
}

fn native(rust: bool, sequence: &[(bool, Call)]) -> Vec<Snapshot> {
    // SAFETY: the oracle owns thread-local real C storage and bounded buffers;
    // the Rust shims adapt the same darray through the fields adapter. Every
    // entry reads or mutates that one stack; end frees the real C darray.
    unsafe {
        let id: matrix4 = identity().into();
        let mut out = m4([0.5; 16]);
        oracle_matrix_begin(true, &id);
        let mut result = Vec::new();
        for &(present, call) in sequence {
            oracle_matrix_set(present);
            match (rust, call) {
                (true, Call::Push) => ffi::gs_matrix_push(),
                (false, Call::Push) => oracle_gs_matrix_push(),
                (true, Call::Pop) => ffi::gs_matrix_pop(),
                (false, Call::Pop) => oracle_gs_matrix_pop(),
                (true, Call::Identity) => ffi::gs_matrix_identity(),
                (false, Call::Identity) => oracle_gs_matrix_identity(),
                (true, Call::Transpose) => ffi::gs_matrix_transpose(),
                (false, Call::Transpose) => oracle_gs_matrix_transpose(),
                (true, Call::Get) => ffi::gs_matrix_get(&mut out),
                (false, Call::Get) => oracle_gs_matrix_get(&mut out),
                (true, Call::Set(v)) => ffi::gs_matrix_set(&m4(v)),
                (false, Call::Set(v)) => oracle_gs_matrix_set(&m4(v)),
                (true, Call::Mul(v)) => ffi::gs_matrix_mul(&m4(v)),
                (false, Call::Mul(v)) => oracle_gs_matrix_mul(&m4(v)),
                (true, Call::RotQuat(r)) => {
                    ffi::gs_matrix_rotquat(&std::mem::transmute::<[f32; 4], quat>(r))
                }
                (false, Call::RotQuat(r)) => {
                    oracle_gs_matrix_rotquat(&std::mem::transmute::<[f32; 4], quat>(r))
                }
                (true, Call::RotAa(r)) => {
                    ffi::gs_matrix_rotaa(&std::mem::transmute::<[f32; 4], axisang>(r))
                }
                (false, Call::RotAa(r)) => {
                    oracle_gs_matrix_rotaa(&std::mem::transmute::<[f32; 4], axisang>(r))
                }
                (true, Call::RotAa4f(r)) => ffi::gs_matrix_rotaa4f(r[0], r[1], r[2], r[3]),
                (false, Call::RotAa4f(r)) => oracle_gs_matrix_rotaa4f(r[0], r[1], r[2], r[3]),
                (true, Call::Translate(t)) => ffi::gs_matrix_translate(&vec3 {
                    x: t[0],
                    y: t[1],
                    z: t[2],
                    w: 0.0,
                }),
                (false, Call::Translate(t)) => oracle_gs_matrix_translate(&vec3 {
                    x: t[0],
                    y: t[1],
                    z: t[2],
                    w: 0.0,
                }),
                (true, Call::Translate3f(t)) => ffi::gs_matrix_translate3f(t[0], t[1], t[2]),
                (false, Call::Translate3f(t)) => oracle_gs_matrix_translate3f(t[0], t[1], t[2]),
                (true, Call::Scale(t)) => ffi::gs_matrix_scale(&vec3 {
                    x: t[0],
                    y: t[1],
                    z: t[2],
                    w: 0.0,
                }),
                (false, Call::Scale(t)) => oracle_gs_matrix_scale(&vec3 {
                    x: t[0],
                    y: t[1],
                    z: t[2],
                    w: 0.0,
                }),
                (true, Call::Scale3f(t)) => ffi::gs_matrix_scale3f(t[0], t[1], t[2]),
                (false, Call::Scale3f(t)) => oracle_gs_matrix_scale3f(t[0], t[1], t[2]),
            }
            result.push(Snapshot {
                current: lanes(oracle_matrix_current()),
                stack: (0..oracle_matrix_depth())
                    .map(|i| lanes(oracle_matrix_saved(i)))
                    .collect(),
                logs: oracle_matrix_logs(),
                message: CStr::from_ptr(oracle_matrix_message())
                    .to_string_lossy()
                    .into_owned(),
            });
        }
        oracle_matrix_end();
        result
    }
}

fn compare(sequence: &[(bool, Call)]) {
    let expected = native(false, sequence);
    for (actual, expected) in native(true, sequence).iter().zip(&expected) {
        assert_eq!(actual, expected);
    }
    let mut m = Mock {
        cur: 0,
        stack: vec![identity()],
    };
    for (&(present, call), expect) in sequence.iter().zip(&expected) {
        match call {
            Call::Push => core::push(present.then_some(&mut m)),
            Call::Pop => {
                core::pop(present.then_some(&mut m));
            }
            Call::Identity => core::identity(present.then_some(&mut m)),
            Call::Transpose => core::transpose(present.then_some(&mut m)),
            Call::Get => {
                core::get(present.then_some(&m));
            }
            Call::Set(v) => core::set(present.then_some(&mut m), safe_m4(v)),
            Call::Mul(v) => core::mul(present.then_some(&mut m), safe_m4(v)),
            Call::RotQuat(r) => core::rotquat(
                present.then_some(&mut m),
                Quat {
                    x: r[0],
                    y: r[1],
                    z: r[2],
                    w: r[3],
                },
            ),
            Call::RotAa(r) => core::rotaa(
                present.then_some(&mut m),
                AxisAng {
                    x: r[0],
                    y: r[1],
                    z: r[2],
                    w: r[3],
                },
            ),
            Call::RotAa4f(r) => core::rotaa4f(present.then_some(&mut m), r[0], r[1], r[2], r[3]),
            Call::Translate(t) => core::translate(
                present.then_some(&mut m),
                Vec3 {
                    x: t[0],
                    y: t[1],
                    z: t[2],
                    w: 0.0,
                },
            ),
            Call::Translate3f(t) => core::translate3f(present.then_some(&mut m), t[0], t[1], t[2]),
            Call::Scale(t) => core::scale(
                present.then_some(&mut m),
                Vec3 {
                    x: t[0],
                    y: t[1],
                    z: t[2],
                    w: 0.0,
                },
            ),
            Call::Scale3f(t) => core::scale3f(present.then_some(&mut m), t[0], t[1], t[2]),
        }
        assert_eq!(safe_lanes(m.stack[m.cur]), expect.current);
        assert_eq!(
            m.stack
                .iter()
                .map(|top| safe_lanes(*top))
                .collect::<Vec<[Same; 16]>>(),
            expect.stack,
        );
    }
}

fn float() -> impl Strategy<Value = f32> {
    prop_oneof![
        8 => -10.0f32..10.0,
        1 => Just(0.0f32),
        1 => prop_oneof![
            Just(f32::NAN),
            Just(f32::INFINITY),
            Just(-0.0f32),
            Just(f32::MAX),
            Just(1e-30f32),
        ],
    ]
}

fn call() -> impl Strategy<Value = Call> {
    prop_oneof![
        3 => Just(Call::Push),
        3 => Just(Call::Pop),
        2 => Just(Call::Identity),
        2 => Just(Call::Transpose),
        2 => Just(Call::Get),
        4 => proptest::array::uniform16(float()).prop_map(|v| Call::Set(v)),
        4 => proptest::array::uniform16(float()).prop_map(|v| Call::Mul(v)),
        2 => proptest::array::uniform4(float()).prop_map(|v| Call::RotQuat(v)),
        2 => proptest::array::uniform4(float()).prop_map(|v| Call::RotAa(v)),
        2 => proptest::array::uniform4(float()).prop_map(|v| Call::RotAa4f(v)),
        1 => proptest::array::uniform3(float()).prop_map(|v| Call::Translate(v)),
        1 => proptest::array::uniform3(float()).prop_map(|v| Call::Translate3f(v)),
        1 => proptest::array::uniform3(float()).prop_map(|v| Call::Scale(v)),
        1 => proptest::array::uniform3(float()).prop_map(|v| Call::Scale3f(v)),
    ]
}

#[test]
fn deterministic_edges() {
    let sequence = vec![
        (false, Call::Push),
        (false, Call::Pop),
        (false, Call::Identity),
        (false, Call::Transpose),
        (false, Call::Get),
        (false, Call::Set([0.5; 16])),
        (false, Call::Mul([0.25; 16])),
        (false, Call::RotQuat([0.5; 4])),
        (false, Call::RotAa([0.5; 4])),
        (false, Call::RotAa4f([0.5; 4])),
        (false, Call::Translate([0.5; 3])),
        (false, Call::Translate3f([0.5; 3])),
        (false, Call::Scale([0.5; 3])),
        (false, Call::Scale3f([0.5; 3])),
        (true, Call::Push),
        (true, Call::Pop),
        (true, Call::Pop),
        (
            true,
            Call::Mul([
                1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 2.0, 3.0, 4.0, 1.0,
            ]),
        ),
        (true, Call::Translate3f([1.0, 2.0, 3.0])),
        (true, Call::RotAa4f([0.0, 1.0, 0.0, 0.5])),
    ];
    compare(&sequence);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(100)))]
    #[test]
    fn entry_sequences(sequence in prop::collection::vec((any::<bool>(), call()), 0..60)) {
        compare(&sequence);
    }
}
