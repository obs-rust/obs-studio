//! Tier 1: `obs_scene::render_plan`, the per-item render decisions of
//! `render_item()` and `render_item_texture()`.
//!
//! The `steps_*` cases port `test/cmocka/test_scene_render_plan_steps.c`
//! 1:1: its 16 cases under the same names, in the same order, with the same
//! inputs and expectations, the characterized-not-endorsed cases included.
//!
//! The `edge_*` cases are new: invariants of the texrender action and the
//! child over every row, and proptests of the texture triggers, the
//! centred check, the effect, the multiplier and the technique against
//! models written another way. Floats are drawn from bit patterns, never
//! from a wide `f32` range (#1951).

use obs_graphics::matrix4::Matrix4;
use obs_graphics::vec4::Vec4;
use obs_scene::render_plan::{
    BLEND_DARKEN, BLEND_METHOD_SRGB_OFF, BLEND_NORMAL, CS_709_EXTENDED, CS_709_SCRGB, CS_SRGB,
    CS_SRGB_16F, Child, Effect, EffectPlan, SCALE_AREA, SCALE_BICUBIC, SCALE_BILINEAR,
    SCALE_DISABLE, SCALE_LANCZOS, SCALE_POINT, Technique, Texrender, Triggers, child, clear_white,
    effect, linear_srgb, multiplier, size_empty, technique, texcoords_centered, texrender_action,
    texture_enabled,
};
use proptest::prelude::*;

const INT_MAX: i32 = i32::MAX;
const INT_MIN: i32 = i32::MIN;
const METHOD_DEFAULT: i32 = 0;

const fn no_triggers() -> Triggers {
    Triggers {
        crop_left: 0,
        crop_top: 0,
        crop_right: 0,
        crop_bottom: 0,
        bounds_crop_left: 0,
        bounds_crop_top: 0,
        bounds_crop_right: 0,
        bounds_crop_bottom: 0,
        scale_filter: SCALE_DISABLE,
        blend_method: METHOD_DEFAULT,
        blend_type: BLEND_NORMAL,
        is_scene: false,
        is_group: false,
    }
}

fn crop_field(t: &mut Triggers, side: usize) -> &mut i32 {
    match side {
        0 => &mut t.crop_left,
        1 => &mut t.crop_top,
        2 => &mut t.crop_right,
        3 => &mut t.crop_bottom,
        4 => &mut t.bounds_crop_left,
        5 => &mut t.bounds_crop_top,
        6 => &mut t.bounds_crop_right,
        _ => &mut t.bounds_crop_bottom,
    }
}

// ---------------------------------------------------------------------------
// Texture triggers

#[test]
fn steps_texture_no_trigger() {
    let mut t = no_triggers();
    assert!(!texture_enabled(&t), "nothing set");

    // a group is a scene, but drawn directly
    t.is_group = true;
    assert!(!texture_enabled(&t), "a group flag alone");
    t.is_scene = true;
    assert!(!texture_enabled(&t), "a group (a scene and a group)");
}

#[test]
fn steps_texture_each_trigger_alone() {
    for side in 0..8 {
        for v in [1, -1, INT_MAX, INT_MIN] {
            let mut t = no_triggers();
            *crop_field(&mut t, side) = v;
            assert!(texture_enabled(&t), "crop field {side} = {v}");
        }
    }

    // every scale filter but Disable, values outside the enum included
    for f in [
        SCALE_POINT,
        SCALE_BICUBIC,
        SCALE_BILINEAR,
        SCALE_LANCZOS,
        SCALE_AREA,
        6,
        -1,
        INT_MAX,
        INT_MIN,
    ] {
        let mut t = no_triggers();
        t.scale_filter = f;
        assert!(texture_enabled(&t), "scale filter {f}");
    }

    // sRGB off only: another value outside the enum is not a trigger
    for (method, expected) in [
        (BLEND_METHOD_SRGB_OFF, true),
        (2, false),
        (-1, false),
        (INT_MAX, false),
    ] {
        let mut t = no_triggers();
        t.blend_method = method;
        assert_eq!(texture_enabled(&t), expected, "blend method {method}");
    }

    // any blend mode but Normal, values outside the enum included
    for b in [1, 2, 3, 4, 5, BLEND_DARKEN, 7, -1, INT_MIN] {
        let mut t = no_triggers();
        t.blend_type = b;
        assert!(texture_enabled(&t), "blend type {b}");
    }

    let mut t = no_triggers();
    t.is_scene = true;
    assert!(texture_enabled(&t), "a nested scene");
}

// ---------------------------------------------------------------------------
// The texrender

/// has, use, format matches, and the action.
const TEXRENDER_ROWS: [(bool, bool, bool, Texrender); 8] = [
    (false, false, false, Texrender::None),
    (false, false, true, Texrender::None),
    (false, true, false, Texrender::Create),
    (false, true, true, Texrender::Create),
    (true, false, false, Texrender::Destroy),
    (true, false, true, Texrender::Destroy),
    (true, true, false, Texrender::Recreate),
    (true, true, true, Texrender::Keep),
];

#[test]
fn steps_texrender_actions() {
    for (has, use_texrender, matches, expected) in TEXRENDER_ROWS {
        assert_eq!(
            texrender_action(has, use_texrender, matches),
            expected,
            "has {has} use {use_texrender} match {matches}"
        );
    }
}

/// #573: the empty-source rule's size check.
#[test]
fn steps_source_size_empty() {
    assert!(size_empty(0, 0));
    assert!(size_empty(0, 1));
    assert!(size_empty(1, 0));
    assert!(size_empty(u32::MAX, 0));
    assert!(size_empty(0, u32::MAX));
    assert!(!size_empty(1, 1));
    assert!(!size_empty(1920, 1080));
    assert!(!size_empty(u32::MAX, u32::MAX));
}

/// #1406: white for Darken only.
#[test]
fn steps_clear_colour() {
    assert!(clear_white(BLEND_DARKEN));
    for b in [BLEND_NORMAL, 1, 2, 3, 4, 5, 7, -1, INT_MAX, INT_MIN] {
        assert!(!clear_white(b), "blend type {b} clears to white");
    }
}

// ---------------------------------------------------------------------------
// The child

/// visible, show active, hide active, and the child.
const CHILD_ROWS: [(bool, bool, bool, Child); 8] = [
    (true, false, false, Child::Source),
    (true, true, false, Child::Show),
    (true, false, true, Child::Source),
    (true, true, true, Child::Show),
    (false, false, false, Child::Source),
    (false, true, false, Child::Source),
    (false, false, true, Child::Hide),
    (false, true, true, Child::Hide),
];

#[test]
fn steps_child_choice() {
    for (visible, show, hide, expected) in CHILD_ROWS {
        assert_eq!(
            child(visible, show, hide),
            expected,
            "visible {visible} show {show} hide {hide}"
        );
    }
}

#[test]
fn steps_linear_srgb() {
    // drawn directly: always linear
    assert!(linear_srgb(false, METHOD_DEFAULT));
    assert!(linear_srgb(false, BLEND_METHOD_SRGB_OFF));
    assert!(linear_srgb(false, -1));
    // through the texrender: unless sRGB is off
    assert!(linear_srgb(true, METHOD_DEFAULT));
    assert!(!linear_srgb(true, BLEND_METHOD_SRGB_OFF));
    assert!(linear_srgb(true, 2));
    assert!(linear_srgb(true, -1));
}

// ---------------------------------------------------------------------------
// Centered texture coordinates

fn identity_with(tx: f32, ty: f32) -> Matrix4 {
    Matrix4::new(
        Vec4::new(1.0, 0.0, 0.0, 0.0),
        Vec4::new(0.0, 1.0, 0.0, 0.0),
        Vec4::new(0.0, 0.0, 1.0, 0.0),
        Vec4::new(tx, ty, 0.0, 1.0),
    )
}

#[test]
fn steps_texcoords_identity() {
    assert!(texcoords_centered(&identity_with(0.0, 0.0)));
    assert!(texcoords_centered(&identity_with(3.0, -7.0)));
    assert!(texcoords_centered(&identity_with(
        16_777_216.0,
        -16_777_216.0
    )));

    // a scale, a rotation or any other entry moved: not centred
    let mut m = identity_with(0.0, 0.0);
    m.x.x = 2.0;
    assert!(!texcoords_centered(&m));
    let mut m = identity_with(0.0, 0.0);
    m.x.y = 1.0;
    assert!(!texcoords_centered(&m));
    let mut m = identity_with(0.0, 0.0);
    m.t.z = 1.0;
    assert!(!texcoords_centered(&m));
    let mut m = identity_with(0.0, 0.0);
    m.t.w = 2.0;
    assert!(!texcoords_centered(&m));
}

#[test]
fn steps_texcoords_fractional_translation() {
    assert!(!texcoords_centered(&identity_with(0.5, 0.0)));
    assert!(!texcoords_centered(&identity_with(0.0, 0.5)));
    assert!(!texcoords_centered(&identity_with(-0.5, 0.0)));
    assert!(!texcoords_centered(&identity_with(100.25, 7.0)));
    // the float just above a whole number
    assert!(!texcoords_centered(&identity_with(
        f32::from_bits(3.0f32.to_bits() + 1),
        0.0
    )));
    // the smallest subnormal
    assert!(!texcoords_centered(&identity_with(f32::from_bits(1), 0.0)));
}

#[test]
fn steps_texcoords_negative_zero() {
    // floorf(-0.0) is -0.0, so a -0.0 translation compares equal
    assert!(texcoords_centered(&identity_with(-0.0, -0.0)));

    // characterized, not endorsed: the compare is memcmp, so -0.0 anywhere
    // else is not the identity's 0.0
    let mut m = identity_with(0.0, 0.0);
    m.x.y = -0.0;
    assert!(!texcoords_centered(&m));
    let mut m = identity_with(0.0, 0.0);
    m.y.x = -0.0;
    assert!(!texcoords_centered(&m));
    let mut m = identity_with(0.0, 0.0);
    m.t.z = -0.0;
    assert!(!texcoords_centered(&m));
    let mut m = identity_with(0.0, 0.0);
    m.x.w = -0.0;
    assert!(!texcoords_centered(&m));
}

#[test]
fn steps_texcoords_nan() {
    // characterized, not endorsed: floorf() gives a quiet NaN back as it
    // is, so a quiet NaN translation compares equal, and an infinite one too
    assert!(texcoords_centered(&identity_with(
        f32::from_bits(0x7fc0_0000),
        0.0
    )));
    assert!(texcoords_centered(&identity_with(
        0.0,
        f32::from_bits(0xffc0_0001)
    )));
    assert!(texcoords_centered(&identity_with(
        f32::INFINITY,
        f32::NEG_INFINITY
    )));

    // a NaN anywhere else is not the identity
    let mut m = identity_with(0.0, 0.0);
    m.x.x = f32::NAN;
    assert!(!texcoords_centered(&m));
    let mut m = identity_with(0.0, 0.0);
    m.t.z = f32::NAN;
    assert!(!texcoords_centered(&m));
    let mut m = identity_with(0.0, 0.0);
    m.t.w = f32::NAN;
    assert!(!texcoords_centered(&m));
}

// ---------------------------------------------------------------------------
// The effect

const fn plan(
    effect: Effect,
    point_sampler: bool,
    base_dimension: bool,
    upscale: bool,
) -> EffectPlan {
    EffectPlan {
        effect,
        point_sampler,
        base_dimension,
        upscale,
    }
}

#[track_caller]
fn check_effect(filter: i32, sx: f32, sy: f32, want: EffectPlan) {
    assert_eq!(
        effect(filter, sx, sy),
        want,
        "filter {filter} scale ({sx} [{:#010x}], {sy} [{:#010x}])",
        sx.to_bits(),
        sy.to_bits()
    );
}

use Effect::{
    Area as E_AREA, Bicubic as E_BICUBIC, BilinearLowres as E_LOWRES, Default as E_DEFAULT,
    Lanczos as E_LANCZOS,
};

#[test]
fn steps_effect_each_filter() {
    let nan = f32::NAN;
    // Disable: nothing at any scale
    check_effect(
        SCALE_DISABLE,
        1.0,
        1.0,
        plan(E_DEFAULT, false, false, false),
    );
    check_effect(
        SCALE_DISABLE,
        2.0,
        2.0,
        plan(E_DEFAULT, false, false, false),
    );
    check_effect(
        SCALE_DISABLE,
        0.25,
        0.25,
        plan(E_DEFAULT, false, false, false),
    );
    check_effect(
        SCALE_DISABLE,
        nan,
        nan,
        plan(E_DEFAULT, false, false, false),
    );

    // Point: the point sampler on the default effect, at any scale, and no
    // base dimension
    check_effect(SCALE_POINT, 1.0, 1.0, plan(E_DEFAULT, true, false, false));
    check_effect(SCALE_POINT, 2.0, 2.0, plan(E_DEFAULT, true, false, false));
    check_effect(SCALE_POINT, 0.25, 0.25, plan(E_DEFAULT, true, false, false));
    check_effect(SCALE_POINT, nan, 1.0, plan(E_DEFAULT, true, false, false));

    // the others at scale 1: nothing
    check_effect(
        SCALE_BICUBIC,
        1.0,
        1.0,
        plan(E_DEFAULT, false, false, false),
    );
    check_effect(
        SCALE_BILINEAR,
        1.0,
        1.0,
        plan(E_DEFAULT, false, false, false),
    );
    check_effect(
        SCALE_LANCZOS,
        1.0,
        1.0,
        plan(E_DEFAULT, false, false, false),
    );
    check_effect(SCALE_AREA, 1.0, 1.0, plan(E_DEFAULT, false, false, false));

    // scaled up on both axes: Area upscales
    check_effect(SCALE_BICUBIC, 2.0, 2.0, plan(E_BICUBIC, false, true, false));
    check_effect(
        SCALE_BILINEAR,
        2.0,
        2.0,
        plan(E_DEFAULT, false, true, false),
    );
    check_effect(SCALE_LANCZOS, 2.0, 2.0, plan(E_LANCZOS, false, true, false));
    check_effect(SCALE_AREA, 2.0, 2.0, plan(E_AREA, false, true, true));

    // scaled down a little: Area does not upscale
    check_effect(
        SCALE_BICUBIC,
        0.75,
        0.75,
        plan(E_BICUBIC, false, true, false),
    );
    check_effect(
        SCALE_BILINEAR,
        0.75,
        0.75,
        plan(E_DEFAULT, false, true, false),
    );
    check_effect(
        SCALE_LANCZOS,
        0.75,
        0.75,
        plan(E_LANCZOS, false, true, false),
    );
    check_effect(SCALE_AREA, 0.75, 0.75, plan(E_AREA, false, true, false));

    // one axis 1, the other up: Area upscales (1 >= 1)
    check_effect(SCALE_AREA, 1.0, 2.0, plan(E_AREA, false, true, true));
    check_effect(SCALE_AREA, 2.0, 1.0, plan(E_AREA, false, true, true));
    // one axis up, the other down: no upscale
    check_effect(SCALE_AREA, 2.0, 0.75, plan(E_AREA, false, true, false));
    check_effect(SCALE_AREA, 0.75, 2.0, plan(E_AREA, false, true, false));
    check_effect(
        SCALE_LANCZOS,
        2.0,
        0.75,
        plan(E_LANCZOS, false, true, false),
    );

    // below 0.5 on either axis: the low-resolution bilinear effect, for
    // every filter but Point, and Area's upscale is not computed
    check_effect(
        SCALE_BICUBIC,
        0.25,
        0.25,
        plan(E_LOWRES, false, true, false),
    );
    check_effect(
        SCALE_BILINEAR,
        0.25,
        0.25,
        plan(E_LOWRES, false, true, false),
    );
    check_effect(
        SCALE_LANCZOS,
        0.25,
        0.25,
        plan(E_LOWRES, false, true, false),
    );
    check_effect(SCALE_AREA, 0.25, 0.25, plan(E_LOWRES, false, true, false));
    check_effect(SCALE_AREA, 0.25, 4.0, plan(E_LOWRES, false, true, false));
    check_effect(SCALE_AREA, 4.0, 0.25, plan(E_LOWRES, false, true, false));

    // characterized, not endorsed: a negative scale is below 0.5
    check_effect(SCALE_BICUBIC, -2.0, 2.0, plan(E_LOWRES, false, true, false));

    // a filter outside the enum is not Disable or Point: the default
    // effect, with the base dimension, when scaled
    check_effect(6, 2.0, 2.0, plan(E_DEFAULT, false, true, false));
    check_effect(-1, 0.75, 0.75, plan(E_DEFAULT, false, true, false));
    check_effect(-1, 0.25, 0.25, plan(E_LOWRES, false, true, false));
    check_effect(INT_MAX, 1.0, 1.0, plan(E_DEFAULT, false, false, false));
}

/// 1 + EPSILON and 1 - EPSILON as floats, and the next floats towards 1.
const ONE_PLUS_EPS: u32 = 0x3f80_0347;
const ONE_MINUS_EPS: u32 = 0x3f7f_f972;
const CLOSE_ABOVE: u32 = 0x3f80_0346;
const CLOSE_BELOW: u32 = 0x3f7f_f973;

#[test]
fn steps_effect_output_scale_edges() {
    // 0.5 and the floats on either side: below 0.5 is low resolution
    let below_half = f32::from_bits(0.5f32.to_bits() - 1);
    let above_half = f32::from_bits(0.5f32.to_bits() + 1);
    check_effect(
        SCALE_BICUBIC,
        below_half,
        1.0,
        plan(E_LOWRES, false, true, false),
    );
    check_effect(SCALE_BICUBIC, 0.5, 1.0, plan(E_BICUBIC, false, true, false));
    check_effect(
        SCALE_BICUBIC,
        above_half,
        1.0,
        plan(E_BICUBIC, false, true, false),
    );
    check_effect(
        SCALE_LANCZOS,
        1.0,
        below_half,
        plan(E_LOWRES, false, true, false),
    );
    check_effect(SCALE_LANCZOS, 1.0, 0.5, plan(E_LANCZOS, false, true, false));
    check_effect(SCALE_AREA, 0.5, 0.5, plan(E_AREA, false, true, false));
    check_effect(
        SCALE_AREA,
        below_half,
        0.5,
        plan(E_LOWRES, false, true, false),
    );

    // 1 +- EPSILON (1e-4f): fabsf(scale - 1) <= EPSILON counts as 1. 1 +
    // EPSILON and 1 - EPSILON round away from 1 as floats, so they are not
    // close; the next floats towards 1 are
    let one_plus_eps = 1.0f32 + 1e-4;
    let one_minus_eps = 1.0f32 - 1e-4;
    assert_eq!(one_plus_eps.to_bits(), ONE_PLUS_EPS);
    assert_eq!(one_minus_eps.to_bits(), ONE_MINUS_EPS);
    let close_above = f32::from_bits(CLOSE_ABOVE);
    let close_below = f32::from_bits(CLOSE_BELOW);

    check_effect(
        SCALE_BICUBIC,
        one_plus_eps,
        1.0,
        plan(E_BICUBIC, false, true, false),
    );
    check_effect(
        SCALE_BICUBIC,
        close_above,
        1.0,
        plan(E_DEFAULT, false, false, false),
    );
    check_effect(
        SCALE_BICUBIC,
        1.0,
        one_minus_eps,
        plan(E_BICUBIC, false, true, false),
    );
    check_effect(
        SCALE_BICUBIC,
        1.0,
        close_below,
        plan(E_DEFAULT, false, false, false),
    );
    check_effect(
        SCALE_BICUBIC,
        close_below,
        close_above,
        plan(E_DEFAULT, false, false, false),
    );

    // Area: 1 + EPSILON on one axis enters the branch and upscales (both
    // >= 1); 1 - EPSILON does not
    check_effect(
        SCALE_AREA,
        one_plus_eps,
        1.0,
        plan(E_AREA, false, true, true),
    );
    check_effect(
        SCALE_AREA,
        one_plus_eps,
        close_below,
        plan(E_AREA, false, true, false),
    );
    check_effect(
        SCALE_AREA,
        one_minus_eps,
        1.0,
        plan(E_AREA, false, true, false),
    );
    check_effect(
        SCALE_AREA,
        close_above,
        close_below,
        plan(E_DEFAULT, false, false, false),
    );
}

#[test]
fn steps_effect_output_scale_nan() {
    // characterized, not endorsed: a NaN scale is never close to 1 and never
    // below 0.5, so the filter's own effect is used, and Area does not
    // upscale
    let nan_q = f32::from_bits(0x7fc0_0000);
    let nan_neg = f32::from_bits(0xffc1_2345);
    check_effect(
        SCALE_BICUBIC,
        nan_q,
        1.0,
        plan(E_BICUBIC, false, true, false),
    );
    check_effect(
        SCALE_BICUBIC,
        1.0,
        nan_neg,
        plan(E_BICUBIC, false, true, false),
    );
    check_effect(
        SCALE_LANCZOS,
        nan_q,
        nan_q,
        plan(E_LANCZOS, false, true, false),
    );
    check_effect(
        SCALE_BILINEAR,
        nan_q,
        1.0,
        plan(E_DEFAULT, false, true, false),
    );
    check_effect(SCALE_AREA, nan_q, 2.0, plan(E_AREA, false, true, false));
    check_effect(SCALE_AREA, 2.0, nan_neg, plan(E_AREA, false, true, false));
    // the other axis below 0.5 still wins
    check_effect(SCALE_AREA, nan_q, 0.25, plan(E_LOWRES, false, true, false));
    // infinity is scaled up
    check_effect(
        SCALE_AREA,
        f32::INFINITY,
        f32::INFINITY,
        plan(E_AREA, false, true, true),
    );
    check_effect(
        SCALE_AREA,
        f32::NEG_INFINITY,
        1.0,
        plan(E_LOWRES, false, true, false),
    );
}

// ---------------------------------------------------------------------------
// The multiplier and the technique

const SPACES: [i32; 4] = [CS_SRGB, CS_SRGB_16F, CS_709_EXTENDED, CS_709_SCRGB];

const fn is_sdr_or_extended(space: i32) -> bool {
    space == CS_SRGB || space == CS_SRGB_16F || space == CS_709_EXTENDED
}

#[track_caller]
fn check_multiplier(current: i32, source: i32, white: f32, want: f32) {
    let got = multiplier(current, source, white);
    assert!(
        got.to_bits() == want.to_bits(),
        "current {current} source {source} white {white}: multiplier {got} ({:#010x}), expected {want} ({:#010x})",
        got.to_bits(),
        want.to_bits()
    );
}

#[test]
fn steps_multiplier_space_pairs() {
    for white in [80.0f32, 300.0, 0.0] {
        for current in SPACES {
            for source in SPACES {
                let mut want = 1.0f32;
                // SDR or extended drawn into scRGB: white / 80
                if current == CS_709_SCRGB && is_sdr_or_extended(source) {
                    want = white / 80.0;
                }
                // scRGB drawn into SDR or extended: 80 / white
                if source == CS_709_SCRGB && is_sdr_or_extended(current) {
                    want = 80.0 / white;
                }
                check_multiplier(current, source, white, want);
            }
        }
    }

    // the values, written out
    check_multiplier(CS_709_SCRGB, CS_SRGB, 300.0, 3.75);
    check_multiplier(CS_709_SCRGB, CS_709_EXTENDED, 80.0, 1.0);
    check_multiplier(CS_SRGB, CS_709_SCRGB, 300.0, f32::from_bits(0x3e88_8889));
    check_multiplier(CS_SRGB_16F, CS_709_SCRGB, 80.0, 1.0);
    check_multiplier(CS_709_SCRGB, CS_709_SCRGB, 300.0, 1.0);
    check_multiplier(CS_SRGB, CS_SRGB, 300.0, 1.0);
    check_multiplier(CS_709_SCRGB, CS_SRGB_16F, 0.0, 0.0);

    // characterized, not endorsed: a white level of 0 makes 80 / 0, +inf
    check_multiplier(CS_SRGB, CS_709_SCRGB, 0.0, f32::INFINITY);
    check_multiplier(CS_709_EXTENDED, CS_709_SCRGB, 0.0, f32::INFINITY);
    check_multiplier(CS_709_EXTENDED, CS_709_SCRGB, -0.0, f32::NEG_INFINITY);

    // a space outside the enum gives 1
    check_multiplier(CS_709_SCRGB, 4, 300.0, 1.0);
    check_multiplier(-1, CS_709_SCRGB, 300.0, 1.0);
    check_multiplier(4, -1, 300.0, 1.0);
}

use Technique::{
    Draw as T_DRAW, DrawMultiply as T_MUL, DrawMultiplyTonemap as T_MUL_TONE,
    DrawTonemap as T_TONE, DrawUpscale as T_UP, DrawUpscaleMultiply as T_UP_MUL,
    DrawUpscaleMultiplyTonemap as T_UP_MUL_TONE, DrawUpscaleTonemap as T_UP_TONE,
};

/// `[source][current]`, in the order of `SPACES`.
const PLAIN: [[Technique; 4]; 4] = [
    [T_DRAW, T_DRAW, T_DRAW, T_MUL],
    [T_DRAW, T_DRAW, T_DRAW, T_MUL],
    [T_TONE, T_TONE, T_DRAW, T_MUL],
    [T_MUL_TONE, T_MUL_TONE, T_MUL, T_DRAW],
];
const UPSCALED: [[Technique; 4]; 4] = [
    [T_UP, T_UP, T_UP, T_UP_MUL],
    [T_UP, T_UP, T_UP, T_UP_MUL],
    [T_UP_TONE, T_UP_TONE, T_UP, T_UP_MUL],
    [T_UP_MUL_TONE, T_UP_MUL_TONE, T_UP_MUL, T_UP],
];

#[test]
fn steps_technique_space_pairs() {
    for (s, source) in SPACES.into_iter().enumerate() {
        for (c, current) in SPACES.into_iter().enumerate() {
            assert_eq!(
                technique(false, current, source),
                PLAIN[s][c],
                "source {source} current {current}"
            );
            assert_eq!(
                technique(true, current, source),
                UPSCALED[s][c],
                "upscaled, source {source} current {current}"
            );
        }
    }

    // a space outside the enum: Draw or DrawUpscale
    assert_eq!(technique(false, CS_709_SCRGB, 4), T_DRAW);
    assert_eq!(technique(true, CS_709_SCRGB, -1), T_UP);
    assert_eq!(technique(false, 4, CS_709_SCRGB), T_DRAW);
    assert_eq!(technique(true, -1, CS_709_EXTENDED), T_UP);
    assert_eq!(technique(true, INT_MIN, INT_MAX), T_UP);
}

// ---------------------------------------------------------------------------
// Edge cases

/// After the action a texrender is there exactly when one is wanted; one
/// is destroyed only when it was there, created only when wanted, and kept
/// only when it was there, wanted and in the format.
#[test]
fn edge_texrender_invariants() {
    for has in [false, true] {
        for wanted in [false, true] {
            for matches in [false, true] {
                let a = texrender_action(has, wanted, matches);
                let destroyed = matches!(a, Texrender::Destroy | Texrender::Recreate);
                let created = matches!(a, Texrender::Create | Texrender::Recreate);
                let after = (has && !destroyed) || created;
                assert_eq!(after, wanted, "{has} {wanted} {matches}: {a:?}");
                assert!(!destroyed || has, "{has} {wanted} {matches}: {a:?}");
                assert!(!created || wanted, "{has} {wanted} {matches}: {a:?}");
                assert_eq!(a == Texrender::Keep, has && wanted && matches);
            }
        }
    }
}

/// Only a visible item draws its show transition and only a hidden one its
/// hide transition, and only while it runs.
#[test]
fn edge_child_invariants() {
    for visible in [false, true] {
        for show in [false, true] {
            for hide in [false, true] {
                match child(visible, show, hide) {
                    Child::Show => assert!(visible && show),
                    Child::Hide => assert!(!visible && hide),
                    Child::Source => assert!(!(visible && show) && !(!visible && hide)),
                }
            }
        }
    }
}

/// The upscaled technique is the plain one's `DrawUpscale` variant, for
/// any pair of spaces, values outside the enum included.
#[test]
fn edge_technique_upscale_variant() {
    const fn up(t: Technique) -> Technique {
        match t {
            T_DRAW => T_UP,
            T_MUL => T_UP_MUL,
            T_TONE => T_UP_TONE,
            T_MUL_TONE => T_UP_MUL_TONE,
            other => other,
        }
    }
    for current in -2..7 {
        for source in -2..7 {
            assert_eq!(
                technique(true, current, source),
                up(technique(false, current, source)),
                "current {current} source {source}"
            );
        }
    }
}

/// Any float, biased to the edges the decisions compare against.
fn any_scale() -> impl Strategy<Value = f32> {
    prop_oneof![
        any::<u32>().prop_map(f32::from_bits),
        (CLOSE_BELOW - 4..=CLOSE_BELOW + 4).prop_map(f32::from_bits),
        (CLOSE_ABOVE - 4..=CLOSE_ABOVE + 4).prop_map(f32::from_bits),
        (0x3eff_fffcu32..=0x3f00_0004).prop_map(f32::from_bits),
        (0x3f7f_fffcu32..=0x3f80_0004).prop_map(f32::from_bits),
        Just(f32::NAN),
        Just(-0.0f32),
    ]
}

/// `fabsf(f - 1) <= 1e-4f` written another way: the bit patterns from
/// `CLOSE_BELOW` to `CLOSE_ABOVE` (positive floats order like their bits).
fn close_model(f: f32) -> bool {
    (CLOSE_BELOW..=CLOSE_ABOVE).contains(&f.to_bits())
}

fn effect_model(filter: i32, sx: f32, sy: f32) -> EffectPlan {
    match filter {
        SCALE_DISABLE => plan(E_DEFAULT, false, false, false),
        SCALE_POINT => plan(E_DEFAULT, true, false, false),
        _ if close_model(sx) && close_model(sy) => plan(E_DEFAULT, false, false, false),
        _ if sx < 0.5 || sy < 0.5 => plan(E_LOWRES, false, true, false),
        SCALE_BICUBIC => plan(E_BICUBIC, false, true, false),
        SCALE_LANCZOS => plan(E_LANCZOS, false, true, false),
        SCALE_AREA => plan(E_AREA, false, true, sx >= 1.0 && sy >= 1.0),
        _ => plan(E_DEFAULT, false, true, false),
    }
}

fn any_crop() -> impl Strategy<Value = i32> {
    prop_oneof![3 => Just(0), 1 => any::<i32>()]
}

fn any_triggers() -> impl Strategy<Value = Triggers> {
    (
        [any_crop(), any_crop(), any_crop(), any_crop()],
        [any_crop(), any_crop(), any_crop(), any_crop()],
        prop_oneof![3 => Just(0), 1 => any::<i32>()],
        prop_oneof![Just(0), Just(1), any::<i32>()],
        prop_oneof![3 => Just(0), 1 => -2i32..9, 1 => any::<i32>()],
        any::<bool>(),
        any::<bool>(),
    )
        .prop_map(
            |(c, b, scale_filter, blend_method, blend_type, is_scene, is_group)| Triggers {
                crop_left: c[0],
                crop_top: c[1],
                crop_right: c[2],
                crop_bottom: c[3],
                bounds_crop_left: b[0],
                bounds_crop_top: b[1],
                bounds_crop_right: b[2],
                bounds_crop_bottom: b[3],
                scale_filter,
                blend_method,
                blend_type,
                is_scene,
                is_group,
            },
        )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4000))]

    /// The triggers against a model: any of them alone is enough.
    #[test]
    fn edge_texture_triggers_model(t in any_triggers()) {
        let crops = [
            t.crop_left, t.crop_top, t.crop_right, t.crop_bottom,
            t.bounds_crop_left, t.bounds_crop_top, t.bounds_crop_right, t.bounds_crop_bottom,
        ];
        let want = crops.iter().any(|&c| c != 0)
            || t.scale_filter != SCALE_DISABLE
            || t.blend_method == BLEND_METHOD_SRGB_OFF
            || t.blend_type != BLEND_NORMAL
            || (t.is_scene && !t.is_group);
        prop_assert_eq!(texture_enabled(&t), want);
    }

    /// The effect against a model with the close-to-1 test written on bits.
    #[test]
    fn edge_effect_model(filter in prop_oneof![-2i32..8, any::<i32>()], sx in any_scale(), sy in any_scale()) {
        prop_assert_eq!(effect(filter, sx, sy), effect_model(filter, sx, sy));
    }

    /// Any whole-number translation is centred; any other entry off the
    /// identity's bits is not.
    #[test]
    fn edge_texcoords_whole_translation(tx in any::<i32>(), ty in any::<i32>(), entry in 0usize..14, bits in any::<u32>()) {
        #[allow(clippy::cast_precision_loss)]
        let m = identity_with(tx as f32, ty as f32);
        prop_assert!(texcoords_centered(&m));

        let mut m = m;
        let slots: [&mut f32; 14] = [
            &mut m.x.x, &mut m.x.y, &mut m.x.z, &mut m.x.w,
            &mut m.y.x, &mut m.y.y, &mut m.y.z, &mut m.y.w,
            &mut m.z.x, &mut m.z.y, &mut m.z.z, &mut m.z.w,
            &mut m.t.z, &mut m.t.w,
        ];
        let slot = slots.into_iter().nth(entry).unwrap();
        let identity_bits = slot.to_bits();
        prop_assume!(bits != identity_bits);
        *slot = f32::from_bits(bits);
        prop_assert!(!texcoords_centered(&m));
    }

    /// A translation that is not a whole number is not centred. Below 2^23
    /// in magnitude, where floats can have a fraction.
    #[test]
    fn edge_texcoords_fraction(magnitude in 0u32..0x4b00_0000, negative in any::<bool>()) {
        let t = f32::from_bits(magnitude | if negative { 0x8000_0000 } else { 0 });
        prop_assume!(t.fract() != 0.0);
        prop_assert!(!texcoords_centered(&identity_with(t, 0.0)));
        prop_assert!(!texcoords_centered(&identity_with(0.0, t)));
    }

    /// The multiplier is 1, white / 80 or 80 / white, bit for bit, for any
    /// white level.
    #[test]
    fn edge_multiplier_any_white(current in -1i32..5, source in -1i32..5, bits in any::<u32>()) {
        let white = f32::from_bits(bits);
        let got = multiplier(current, source, white);
        let want = if current == CS_709_SCRGB && is_sdr_or_extended(source) {
            white / 80.0
        } else if source == CS_709_SCRGB && is_sdr_or_extended(current) {
            80.0 / white
        } else {
            1.0
        };
        prop_assert_eq!(got.to_bits(), want.to_bits());
    }
}
