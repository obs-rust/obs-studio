//! Tier 3: the Rust matrix calls against the original C
//! (`matrix-entrypoints.c.h`, compiled as `oracle_gs_matrix_*` over the
//! original C matrix stack, `oracle_gs_matrix_stack_*`) on the real private
//! graphics layout, a stand-in context per side and a recording `blog`. A
//! sequence runs on both sides, each on its own stack; after every call the
//! test compares the whole stack bit for bit (any NaN equal to any NaN,
//! the rule of `state_stacks_parity`), the index of its top,
//! `gs_matrix_get`'s output and the log lines. Inputs: random sequences of
//! the fourteen calls with random matrices, quaternions, axis-angles and
//! vectors (NaN, infinities and signed zeros too), pops past the bottom, and
//! calls without a context. No intentional differences of this port. A NULL
//! argument is not compared: the C stack functions dereference it, and the
//! Rust ones (since the state stacks port) do nothing; Tier 1 pins the Rust.
use obs_c_oracle as _;
use obs_graphics::ffi::axisang::axisang;
use obs_graphics::ffi::matrix_entrypoints as rust;
use obs_graphics::ffi::matrix4::matrix4;
use obs_graphics::ffi::quat::quat;
use obs_graphics::ffi::vec3::vec3;
use proptest::prelude::*;
use std::ffi::{CStr, c_char, c_int};
use std::mem::MaybeUninit;
use std::ptr;
use std::sync::Mutex;

unsafe extern "C" {
    fn oracle_gs_matrix_push();
    fn oracle_gs_matrix_pop();
    fn oracle_gs_matrix_identity();
    fn oracle_gs_matrix_transpose();
    fn oracle_gs_matrix_set(matrix: *const matrix4);
    fn oracle_gs_matrix_get(dst: *mut matrix4);
    fn oracle_gs_matrix_mul(matrix: *const matrix4);
    fn oracle_gs_matrix_rotquat(rot: *const quat);
    fn oracle_gs_matrix_rotaa(rot: *const axisang);
    fn oracle_gs_matrix_translate(pos: *const vec3);
    fn oracle_gs_matrix_scale(scale: *const vec3);
    fn oracle_gs_matrix_rotaa4f(x: f32, y: f32, z: f32, angle: f32);
    fn oracle_gs_matrix_translate3f(x: f32, y: f32, z: f32);
    fn oracle_gs_matrix_scale3f(x: f32, y: f32, z: f32);
    fn oracle_matrix_init();
    fn oracle_matrix_free();
    fn oracle_matrix_select(side: c_int, context: bool);
    fn oracle_matrix_read(side: c_int, out: *mut matrix4, max: usize, cur: *mut usize) -> usize;
    fn oracle_matrix_logs() -> u32;
    fn oracle_matrix_level() -> c_int;
    fn oracle_matrix_log() -> *const c_char;
}

/// The stand-in context is per thread, but the C oracle's darrays go
/// through the process-wide allocation counter other tests check: keep
/// one sequence at a time.
static LOCK: Mutex<()> = Mutex::new(());

const C: c_int = 0;
const RUST: c_int = 1;
/// Deep enough for any sequence here (at most 48 calls, one push each).
const MAX_DEPTH: usize = 64;

type M = [f32; 16];

#[derive(Clone, Copy, Debug)]
enum Op {
    Push,
    Pop,
    Identity,
    Transpose,
    Set(M),
    Get,
    Mul(M),
    RotQuat([f32; 4]),
    RotAa([f32; 4]),
    Translate([f32; 3]),
    Scale([f32; 3]),
    RotAa4f([f32; 4]),
    Translate3f([f32; 3]),
    Scale3f([f32; 3]),
}

fn name(op: Op) -> &'static str {
    match op {
        Op::Push => "gs_matrix_push",
        Op::Pop => "gs_matrix_pop",
        Op::Identity => "gs_matrix_identity",
        Op::Transpose => "gs_matrix_transpose",
        Op::Set(_) => "gs_matrix_set",
        Op::Get => "gs_matrix_get",
        Op::Mul(_) => "gs_matrix_mul",
        Op::RotQuat(_) => "gs_matrix_rotquat",
        Op::RotAa(_) => "gs_matrix_rotaa",
        Op::Translate(_) => "gs_matrix_translate",
        Op::Scale(_) => "gs_matrix_scale",
        Op::RotAa4f(_) => "gs_matrix_rotaa4f",
        Op::Translate3f(_) => "gs_matrix_translate3f",
        Op::Scale3f(_) => "gs_matrix_scale3f",
    }
}

/// `struct matrix4` from sixteen floats, through the C layout (rows of
/// four, each a 16-byte vec4).
fn mat(m: &M) -> matrix4 {
    let mut out = MaybeUninit::<matrix4>::zeroed();
    // SAFETY: `struct matrix4` is sixteen packed floats (the layout tests
    // pin it); every bit pattern is a float.
    unsafe {
        ptr::copy_nonoverlapping(m.as_ptr(), out.as_mut_ptr().cast::<f32>(), 16);
        out.assume_init()
    }
}

/// The bits of each float, every NaN as one value: as in
/// `state_stacks_parity`, the NaN signs come from the matrix4 math, where
/// Rust does not fix the sign of a NaN an operation makes.
fn bits(m: &matrix4) -> [u32; 16] {
    // SAFETY: as in `mat`.
    let f: [f32; 16] = unsafe { std::mem::transmute_copy(m) };
    f.map(|v| {
        if v.is_nan() {
            f32::NAN.to_bits()
        } else {
            v.to_bits()
        }
    })
}

/// A vec4-backed C value (`quat`, `axisang`, `vec3`) from its floats; a
/// vec3's fourth float stays 0, as `vec3_set` leaves it.
fn v4<T>(f: &[f32]) -> T {
    assert_eq!(size_of::<T>(), 16);
    let mut all = [0.0f32; 4];
    all[..f.len()].copy_from_slice(f);
    // SAFETY: each of these types is four packed floats.
    unsafe { std::mem::transmute_copy(&all) }
}

/// What a call left: the stack, its top, `gs_matrix_get`'s output and the log.
#[derive(Debug, PartialEq)]
struct Observed {
    stack: Vec<[u32; 16]>,
    num: usize,
    cur: usize,
    got: [u32; 16],
    logs: u32,
    level: c_int,
    log: Vec<u8>,
}

/// The output `gs_matrix_get` starts from, so "left alone" shows.
const SENTINEL: M = [-7.25; 16];

fn call(side: c_int, context: bool, op: Op) -> Observed {
    let c = side == C;
    let mut got = mat(&SENTINEL);
    // SAFETY: the stand-in context is this thread's (selected just before
    // the call); every pointer argument is a live local or NULL.
    unsafe {
        oracle_matrix_select(side, context);
        match op {
            Op::Push if c => oracle_gs_matrix_push(),
            Op::Push => rust::gs_matrix_push(),
            Op::Pop if c => oracle_gs_matrix_pop(),
            Op::Pop => rust::gs_matrix_pop(),
            Op::Identity if c => oracle_gs_matrix_identity(),
            Op::Identity => rust::gs_matrix_identity(),
            Op::Transpose if c => oracle_gs_matrix_transpose(),
            Op::Transpose => rust::gs_matrix_transpose(),
            Op::Set(m) if c => oracle_gs_matrix_set(&mat(&m)),
            Op::Set(m) => rust::gs_matrix_set(&mat(&m)),
            Op::Get if c => oracle_gs_matrix_get(&mut got),
            Op::Get => rust::gs_matrix_get(&mut got),
            Op::Mul(m) if c => oracle_gs_matrix_mul(&mat(&m)),
            Op::Mul(m) => rust::gs_matrix_mul(&mat(&m)),
            Op::RotQuat(q) if c => oracle_gs_matrix_rotquat(&v4(&q)),
            Op::RotQuat(q) => rust::gs_matrix_rotquat(&v4(&q)),
            Op::RotAa(a) if c => oracle_gs_matrix_rotaa(&v4(&a)),
            Op::RotAa(a) => rust::gs_matrix_rotaa(&v4(&a)),
            Op::Translate(v) if c => oracle_gs_matrix_translate(&v4(&v)),
            Op::Translate(v) => rust::gs_matrix_translate(&v4(&v)),
            Op::Scale(v) if c => oracle_gs_matrix_scale(&v4(&v)),
            Op::Scale(v) => rust::gs_matrix_scale(&v4(&v)),
            Op::RotAa4f([x, y, z, a]) if c => oracle_gs_matrix_rotaa4f(x, y, z, a),
            Op::RotAa4f([x, y, z, a]) => rust::gs_matrix_rotaa4f(x, y, z, a),
            Op::Translate3f([x, y, z]) if c => oracle_gs_matrix_translate3f(x, y, z),
            Op::Translate3f([x, y, z]) => rust::gs_matrix_translate3f(x, y, z),
            Op::Scale3f([x, y, z]) if c => oracle_gs_matrix_scale3f(x, y, z),
            Op::Scale3f([x, y, z]) => rust::gs_matrix_scale3f(x, y, z),
        }
        let mut stack = [mat(&[0.0; 16]); MAX_DEPTH];
        let mut cur = 0;
        let num = oracle_matrix_read(side, stack.as_mut_ptr(), MAX_DEPTH, &mut cur);
        Observed {
            stack: stack[..num.min(MAX_DEPTH)].iter().map(bits).collect(),
            num,
            cur,
            got: bits(&got),
            logs: oracle_matrix_logs(),
            level: oracle_matrix_level(),
            log: CStr::from_ptr(oracle_matrix_log()).to_bytes().to_vec(),
        }
    }
}

/// Runs `steps` on both sides from a fresh stack, comparing after each.
fn compare(steps: &[(bool, Op)]) {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // SAFETY: this thread's stand-in contexts.
    unsafe { oracle_matrix_init() };
    for (i, &(context, op)) in steps.iter().enumerate() {
        let before = call(C, true, Op::Get).got;
        let original = call(C, context, op);
        let ported = call(RUST, context, op);
        assert_eq!(
            ported,
            original,
            "step {i}: {:?} in {steps:?}",
            (context, op)
        );
        // And what the C does, so a fault common to both sides shows too.
        if !context {
            assert_eq!(
                (ported.logs, ported.level, ported.log.as_slice()),
                (
                    1,
                    400,
                    format!("{}: called while not in a graphics context", name(op)).as_bytes()
                ),
            );
            if let Op::Get = op {
                assert_eq!(ported.got, bits(&mat(&SENTINEL)));
            }
        } else if let Op::Get = op {
            assert_eq!(ported.got, before);
        }
        assert_eq!(ported.num, ported.cur + 1, "the top is the last matrix");
    }
    // SAFETY: as above.
    unsafe { oracle_matrix_free() };
}

#[test]
fn characterized_sequences_match_c() {
    let m1: M = std::array::from_fn(|i| 1.0 + i as f32 * 0.25 - (i % 3) as f32);
    let odd: M = std::array::from_fn(|i| match i % 4 {
        0 => f32::NAN,
        1 => f32::INFINITY,
        2 => -0.0,
        _ => f32::from_bits(0x7fc0_0001 + i as u32),
    });
    let q = [0.2, -0.4, 0.1, 0.88];
    let aa = [0.3, 0.5, -0.8, 1.25];
    let v = [1.5, -2.0, 0.75];
    let every = [
        Op::Push,
        Op::Pop,
        Op::Identity,
        Op::Transpose,
        Op::Set(m1),
        Op::Get,
        Op::Mul(m1),
        Op::RotQuat(q),
        Op::RotAa(aa),
        Op::Translate(v),
        Op::Scale(v),
        Op::RotAa4f(aa),
        Op::Translate3f(v),
        Op::Scale3f(v),
    ];
    for context in [false, true] {
        for op in every {
            // Each call on a set top, one push deep, then popped past the bottom.
            compare(&[
                (true, Op::Set(m1)),
                (true, Op::Push),
                (context, op),
                (true, Op::Get),
                (true, Op::Pop),
                (true, Op::Pop),
                (true, Op::Pop),
                (context, op),
                (true, Op::Get),
            ]);
            // And on a top full of NaN, infinities and signed zeros.
            compare(&[(true, Op::Set(odd)), (context, op), (true, Op::Get)]);
        }
    }
}

fn float() -> impl Strategy<Value = f32> + Clone {
    prop_oneof![
        8 => -100.0f32..100.0,
        1 => Just(f32::NAN),
        1 => Just(f32::INFINITY),
        1 => Just(f32::NEG_INFINITY),
        1 => Just(-0.0f32),
        1 => any::<u32>().prop_map(f32::from_bits),
    ]
}

fn op() -> impl Strategy<Value = Op> {
    let m = prop::array::uniform16(float());
    let f4 = prop::array::uniform4(float());
    let f3 = prop::array::uniform3(float());
    prop_oneof![
        3 => Just(Op::Push),
        3 => Just(Op::Pop),
        1 => Just(Op::Identity),
        1 => Just(Op::Transpose),
        1 => m.clone().prop_map(Op::Set),
        1 => Just(Op::Get),
        1 => m.prop_map(Op::Mul),
        1 => f4.clone().prop_map(Op::RotQuat),
        1 => f4.clone().prop_map(Op::RotAa),
        1 => f3.clone().prop_map(Op::Translate),
        1 => f3.clone().prop_map(Op::Scale),
        1 => f4.prop_map(Op::RotAa4f),
        1 => f3.clone().prop_map(Op::Translate3f),
        1 => f3.prop_map(Op::Scale3f),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default()
    })]
    #[test]
    fn random_sequences_match_c(
        steps in prop::collection::vec((prop::bool::weighted(0.85), op()), 1..=48),
    ) {
        compare(&steps);
    }
}
