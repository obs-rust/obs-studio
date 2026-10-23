//! Tier 3: the shims of `obs_scene::ffi::scene_extent` against the original
//! `obs-scene-extent.c.h` (the oracle).
//!
//! Each run drives both sides as `resize_scene_base` does: start the fold,
//! add the items in order, size it. After every add the min and the max
//! must be bit-equal (so -0 and +0 differ, and the start values must stay
//! where the C leaves them), and the scale bit-equal with equal `cx` and
//! `cy`. A NaN is never stored by the fold, so from the start values every
//! output is compared bit for bit, the default NaN of `inf - inf` included.
//!
//! Runs: the layout; the `test_group_extent` matrices; proptests over lists
//! of up to 8 box transforms whose 16 entries are finite, huge (up to
//! `f32::MAX`), 0 and -0, the start value `M_INFINITE`, 2^32, NaN, the
//! infinities, or any bit pattern; lists of matrices of only 0, -0, 1 and
//! -1, where corners of -0 and +0 tie; and the add and size shims on their own
//! from any min and max. There the C keeps a NaN min or max as it is, and
//! `max - min` of two NaNs may carry either payload (the order of the
//! operands is the compiler's), so only that case compares as "both NaN".
//!
//! No intentional differences. `test_group_extent` compiles the C into
//! itself, so this is what compares the swap; the real-libobs group tests
//! see it.

use std::mem::{align_of, size_of};

use obs_c_oracle::graphics_math::{
    OracleMatrix4, OracleVec4, oracle_matrix4_align, oracle_matrix4_offset_t, oracle_matrix4_size,
};
use obs_c_oracle::scene_extent::{
    oracle_obs_scene_extent_add, oracle_obs_scene_extent_init, oracle_obs_scene_extent_size,
};
use obs_c_oracle::vec2::OracleVec2;
use obs_graphics::ffi::matrix4::matrix4;
use obs_graphics::ffi::vec2::vec2;
use obs_graphics::ffi::vec4::vec4;
use obs_scene::ffi::scene_extent::{
    obs_scene_extent_add, obs_scene_extent_init, obs_scene_extent_size,
};
use proptest::prelude::*;

const M_INFINITE: f32 = 3.4e38;

type Rows = [[f32; 4]; 4];

fn matrix(rows: Rows) -> (matrix4, OracleMatrix4) {
    let r = |i: usize| {
        let [x, y, z, w] = rows[i];
        (vec4 { x, y, z, w }, OracleVec4 { x, y, z, w })
    };
    let ((x, ox), (y, oy), (z, oz), (t, ot)) = (r(0), r(1), r(2), r(3));
    (
        matrix4 { x, y, z, t },
        OracleMatrix4 {
            x: ox,
            y: oy,
            z: oz,
            t: ot,
        },
    )
}

fn assert_bits(what: &str, rust: vec2, oracle: OracleVec2) {
    assert!(
        rust.x.to_bits() == oracle.x.to_bits() && rust.y.to_bits() == oracle.y.to_bits(),
        "{what}: rust ({}, {}) [{:#010x}, {:#010x}] oracle ({}, {}) [{:#010x}, {:#010x}]",
        rust.x,
        rust.y,
        rust.x.to_bits(),
        rust.y.to_bits(),
        oracle.x,
        oracle.y,
        oracle.x.to_bits(),
        oracle.y.to_bits()
    );
}

/// The fold of `items` on both sides, compared after every step.
fn check_fold(items: &[Rows]) {
    let mut rust_min = vec2 { x: 1.0, y: 2.0 };
    let mut rust_max = vec2 { x: 3.0, y: 4.0 };
    let mut c_min = OracleVec2 { x: 1.0, y: 2.0 };
    let mut c_max = OracleVec2 { x: 3.0, y: 4.0 };
    // SAFETY: live, distinct stack values.
    unsafe {
        obs_scene_extent_init(&raw mut rust_min, &raw mut rust_max);
        oracle_obs_scene_extent_init(&raw mut c_min, &raw mut c_max);
    }
    assert_bits("init min", rust_min, c_min);
    assert_bits("init max", rust_max, c_max);

    for (i, rows) in items.iter().enumerate() {
        let (rust_m, c_m) = matrix(*rows);
        // SAFETY: live, distinct stack values; the matrix is only read.
        unsafe {
            obs_scene_extent_add(&raw mut rust_min, &raw mut rust_max, &raw const rust_m);
            oracle_obs_scene_extent_add(&raw mut c_min, &raw mut c_max, &raw const c_m);
        }
        assert_bits(&format!("min after item {i} {rows:?}"), rust_min, c_min);
        assert_bits(&format!("max after item {i} {rows:?}"), rust_max, c_max);
    }

    check_size(rust_min, rust_max, false);
}

/// The size of `min`..`max` on both sides. `nan_pair` relaxes the scale to
/// "both NaN" on an axis where min and max are both NaN.
fn check_size(min: vec2, max: vec2, nan_pair: bool) {
    let c_min = OracleVec2 { x: min.x, y: min.y };
    let c_max = OracleVec2 { x: max.x, y: max.y };
    let mut rust_scale = vec2 { x: 5.0, y: 6.0 };
    let mut c_scale = OracleVec2 { x: 5.0, y: 6.0 };
    let (mut rust_cx, mut rust_cy, mut c_cx, mut c_cy) = (7u32, 8u32, 7u32, 8u32);
    // SAFETY: live, distinct stack values.
    unsafe {
        obs_scene_extent_size(
            &raw mut rust_scale,
            &raw mut rust_cx,
            &raw mut rust_cy,
            &raw const min,
            &raw const max,
        );
        oracle_obs_scene_extent_size(
            &raw mut c_scale,
            &raw mut c_cx,
            &raw mut c_cy,
            &raw const c_min,
            &raw const c_max,
        );
    }
    let same = |rust: f32, c: f32, both_nan: bool| {
        rust.to_bits() == c.to_bits() || (both_nan && rust.is_nan() && c.is_nan())
    };
    let nan_x = nan_pair && min.x.is_nan() && max.x.is_nan();
    let nan_y = nan_pair && min.y.is_nan() && max.y.is_nan();
    assert!(
        same(rust_scale.x, c_scale.x, nan_x) && same(rust_scale.y, c_scale.y, nan_y),
        "scale of ({}, {})..({}, {}): rust ({}, {}) oracle ({}, {})",
        min.x,
        min.y,
        max.x,
        max.y,
        rust_scale.x,
        rust_scale.y,
        c_scale.x,
        c_scale.y
    );
    assert_eq!(
        (rust_cx, rust_cy),
        (c_cx, c_cy),
        "size of ({}, {})..({}, {})",
        min.x,
        min.y,
        max.x,
        max.y
    );
}

fn box_rows(xx: f32, xy: f32, yx: f32, yy: f32, tx: f32, ty: f32) -> Rows {
    [
        [xx, xy, 0.0, 0.0],
        [yx, yy, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [tx, ty, 0.0, 1.0],
    ]
}

#[test]
fn layouts_match_the_c_structs() {
    // SAFETY: the oracle functions take no pointers.
    let (size, align, offset_t) = unsafe {
        (
            oracle_matrix4_size(),
            oracle_matrix4_align(),
            oracle_matrix4_offset_t(),
        )
    };
    assert_eq!(size_of::<matrix4>(), size);
    assert_eq!(align_of::<matrix4>(), align);
    assert_eq!(std::mem::offset_of!(matrix4, t), offset_t);
    assert_eq!(size_of::<vec2>(), size_of::<OracleVec2>());
    assert_eq!(align_of::<vec2>(), align_of::<OracleVec2>());
}

/// The `test_group_extent` matrices, through the shims.
#[test]
fn cmocka_cases_match_oracle() {
    let nan = f32::NAN;
    let inf = f32::INFINITY;
    check_fold(&[box_rows(100.0, 0.0, 0.0, 50.0, 10.0, 20.0)]);
    check_fold(&[
        box_rows(100.0, 0.0, 0.0, 50.0, 10.0, 20.0),
        box_rows(30.0, 0.0, 0.0, 30.0, -5.0, 60.0),
        box_rows(10.0, 0.0, 0.0, 10.0, 200.0, -40.0),
    ]);
    check_fold(&[box_rows(10.0, 10.0, -10.0, 10.0, 50.0, 50.0)]);
    check_fold(&[box_rows(99.5, 0.0, 0.0, 0.25, 0.5, -0.75)]);
    check_fold(&[box_rows(1280.0, 0.0, 0.0, 720.0, 0.25, 0.25)]);
    check_fold(&[]);
    let finite = box_rows(30.0, 0.0, 0.0, 10.0, 5.0, 0.0);
    check_fold(&[box_rows(100.0, 0.0, 0.0, 50.0, nan, 20.0)]);
    check_fold(&[box_rows(100.0, 0.0, 0.0, 50.0, nan, 20.0), finite]);
    check_fold(&[finite, box_rows(nan, 0.0, 0.0, 50.0, 0.0, 20.0)]);
    check_fold(&[box_rows(100.0, 0.0, 0.0, 50.0, inf, 20.0)]);
    check_fold(&[box_rows(100.0, 0.0, 0.0, 50.0, inf, 20.0), finite]);
    check_fold(&[box_rows(100.0, 0.0, 0.0, 50.0, 10.0, -inf)]);
    check_fold(&[box_rows(inf, 0.0, 0.0, 50.0, 0.0, 0.0)]);
    check_fold(&[box_rows(0.0, 0.0, 0.0, 0.0, f32::MAX, -f32::MAX)]);
    check_fold(&[box_rows(5e9, 0.0, 0.0, 4_294_967_040.0, 0.0, 0.0)]);
    check_fold(&[box_rows(4_294_967_296.0, 0.0, 0.0, 1e30, 0.0, 0.0)]);
    check_fold(&[
        box_rows(1.0, 0.0, 0.0, 1.0, -3e38, -3e38),
        box_rows(1.0, 0.0, 0.0, 1.0, 3e38, 3e38),
    ]);
    let neg_zero = [
        [-1.0, 0.0, 0.0, 0.0],
        [-1.0, 0.0, 0.0, 0.0],
        [-1.0, 0.0, 1.0, 0.0],
        [-0.0, 0.0, 0.0, 1.0],
    ];
    let pos_zero = box_rows(-1.0, 0.0, -1.0, 0.0, 0.0, 0.0);
    check_fold(&[neg_zero, pos_zero]);
    check_fold(&[pos_zero, neg_zero]);
}

/// Finite, huge, the special values, or any bit pattern.
fn entry() -> impl Strategy<Value = f32> {
    prop_oneof![
        4 => -1.0e4f32..1.0e4f32,
        2 => huge(),
        2 => prop::sample::select(vec![
            0.0,
            -0.0,
            1.0,
            -1.0,
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::MAX,
            -f32::MAX,
            M_INFINITE,
            -M_INFINITE,
            4_294_967_296.0,
            4_294_967_040.0,
            f32::MIN_POSITIVE,
        ]),
        1 => any::<u32>().prop_map(f32::from_bits),
    ]
}

/// A finite value of 1e30 up to `f32::MAX`, either sign. It is drawn from the
/// bit patterns, not from a float range: proptest's float sampler can trip
/// its own debug assertion on a range this wide (seen on Windows in #1929's
/// merge group). Positive floats order like their bits, so the set is the same.
fn huge() -> impl Strategy<Value = f32> {
    (any::<bool>(), 1.0e30f32.to_bits()..=f32::MAX.to_bits()).prop_map(|(negative, bits)| {
        let value = f32::from_bits(bits);
        if negative { -value } else { value }
    })
}

fn rows() -> impl Strategy<Value = Rows> {
    prop::array::uniform4(prop::array::uniform4(entry()))
}

/// A box transform as the scene builds one: z row (0, 0, 1, 0) and w column
/// (0, 0, 0, 1), with the six entries that vary.
fn box_like() -> impl Strategy<Value = Rows> {
    prop::array::uniform6(entry())
        .prop_map(|[xx, xy, yx, yy, tx, ty]| box_rows(xx, xy, yx, yy, tx, ty))
}

/// Only 0, -0, 1 and -1, so corners of -0 and +0 meet often and every
/// tie of the compares is reached.
fn signed_zero_rows() -> impl Strategy<Value = Rows> {
    prop::array::uniform4(prop::array::uniform4(prop::sample::select(vec![
        0.0f32, -0.0, 1.0, -1.0,
    ])))
}

fn any_bits() -> impl Strategy<Value = f32> {
    any::<u32>().prop_map(f32::from_bits)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4096))]

    #[test]
    fn random_matrices_match_oracle(items in prop::collection::vec(rows(), 0..8)) {
        check_fold(&items);
    }

    #[test]
    fn random_boxes_match_oracle(items in prop::collection::vec(box_like(), 0..8)) {
        check_fold(&items);
    }

    #[test]
    fn signed_zero_ties_match_oracle(items in prop::collection::vec(signed_zero_rows(), 1..6)) {
        check_fold(&items);
    }

    #[test]
    fn add_from_any_state_matches_oracle(
        min_x in any_bits(),
        min_y in any_bits(),
        max_x in any_bits(),
        max_y in any_bits(),
        rows in rows(),
    ) {
        let mut rust_min = vec2 { x: min_x, y: min_y };
        let mut rust_max = vec2 { x: max_x, y: max_y };
        let mut c_min = OracleVec2 { x: min_x, y: min_y };
        let mut c_max = OracleVec2 { x: max_x, y: max_y };
        let (rust_m, c_m) = matrix(rows);
        // SAFETY: live, distinct stack values; the matrix is only read.
        unsafe {
            obs_scene_extent_add(&raw mut rust_min, &raw mut rust_max, &raw const rust_m);
            oracle_obs_scene_extent_add(&raw mut c_min, &raw mut c_max, &raw const c_m);
        }
        assert_bits("min", rust_min, c_min);
        assert_bits("max", rust_max, c_max);
    }

    #[test]
    fn size_of_any_extent_matches_oracle(
        min_x in prop_oneof![entry(), any_bits()],
        min_y in prop_oneof![entry(), any_bits()],
        max_x in prop_oneof![entry(), any_bits()],
        max_y in prop_oneof![entry(), any_bits()],
    ) {
        check_size(vec2 { x: min_x, y: min_y }, vec2 { x: max_x, y: max_y }, true);
    }
}
