//! Tier 2 layout and Tier 3 exact parity. No differences on C-defined inputs.
//! Exclude nonfinite/out-of-range float-to-integer conversions: C has UB there.
//! Positive bounded boxes, nonzero dyadic scales and bounded dimensions keep
//! every conversion representable, including zero-sized sources.
use core::mem::{align_of, offset_of, size_of};
use obs_c_oracle::scene_bounds_place::*;
use obs_c_oracle::vec2::OracleVec2;
use obs_graphics::ffi::vec2::vec2;
use obs_scene::ffi::scene_bounds_place::{SceneItemCrop, add_alignment, obs_bounds_place};
use proptest::prelude::*;

#[test]
fn crop_layout_matches_real_obs_h() {
    let rust = [
        size_of::<SceneItemCrop>(),
        align_of::<SceneItemCrop>(),
        offset_of!(SceneItemCrop, left),
        offset_of!(SceneItemCrop, top),
        offset_of!(SceneItemCrop, right),
        offset_of!(SceneItemCrop, bottom),
    ];
    for (field, want) in rust.into_iter().enumerate() {
        // SAFETY: layout query accepts these six selectors and has no pointer inputs.
        assert_eq!(unsafe { oracle_scene_crop_layout(field as u32) }, want);
    }
}
fn same(a: f32, b: f32) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
}
#[allow(clippy::too_many_arguments)]
fn check(
    kind: i32,
    bx: f32,
    by: f32,
    ox: f32,
    oy: f32,
    sx: f32,
    sy: f32,
    cx: u32,
    cy: u32,
    align: u32,
    crop: bool,
) {
    let rb = vec2 { x: bx, y: by };
    let cb = OracleVec2 { x: bx, y: by };
    let mut ro = vec2 { x: ox, y: oy };
    let mut co = OracleVec2 { x: ox, y: oy };
    let mut rs = vec2 { x: sx, y: sy };
    let mut cs = OracleVec2 { x: sx, y: sy };
    let (mut rx, mut ry, mut cx, mut cy) = (cx, cy, cx, cy);
    let mut rc = SceneItemCrop {
        left: 7,
        top: -9,
        right: 11,
        bottom: 13,
    };
    let mut cc = OracleCrop {
        left: 7,
        top: -9,
        right: 11,
        bottom: 13,
    };
    // SAFETY: all fields are live, correctly aligned, distinct stack objects.
    unsafe {
        obs_bounds_place(
            kind,
            &raw const rb,
            &raw mut ro,
            &raw mut rs,
            &raw mut rx,
            &raw mut ry,
            align,
            crop,
            &raw mut rc,
        );
        oracle_obs_bounds_place(
            kind,
            &raw const cb,
            &raw mut co,
            &raw mut cs,
            &raw mut cx,
            &raw mut cy,
            align,
            crop,
            &raw mut cc,
        );
    }
    assert!(
        same(ro.x, co.x) && same(ro.y, co.y) && same(rs.x, cs.x) && same(rs.y, cs.y),
        "kind={kind} box={bx},{by} align={align} crop={crop}: Rust {ro:?}/{rs:?}, C {co:?}/{cs:?}"
    );
    assert_eq!((rx, ry), (cx, cy));
    assert_eq!(
        (rc.left, rc.top, rc.right, rc.bottom),
        (cc.left, cc.top, cc.right, cc.bottom)
    );
    assert_eq!((rb.x, rb.y), (cb.x, cb.y));
}
proptest! {
 #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(2048)))]
 #[test]
 fn placement_matches_c(kind in -1i32..9, bx in 1u32..4096, by in 1u32..4096, fraction in 0u32..16, cx in 0u32..4096, cy in 0u32..4096, sx in prop_oneof![Just(-2.0f32),Just(-0.5),Just(0.5),Just(1.0),Just(2.0)], sy in prop_oneof![Just(-2.0f32),Just(-0.5),Just(0.5),Just(1.0),Just(2.0)], ox in -256i32..256, oy in -256i32..256, align in 0u32..32, crop in any::<bool>()) {
  check(kind,bx as f32 + fraction as f32/16.0,by as f32,ox as f32,oy as f32,sx,sy,cx,cy,align,crop);
 }
 #[test]
 fn alignment_matches_c(x in -10000i32..10000,y in -10000i32..10000,cx in any::<i32>(),cy in any::<i32>(),align in any::<u32>()) {
 let mut r=vec2{x:x as f32,y:y as f32};let mut c=OracleVec2{x:x as f32,y:y as f32};
 // SAFETY: live writable stack vectors.
 unsafe {add_alignment(&raw mut r,align,cx,cy);oracle_add_alignment(&raw mut c,align,cx,cy);}
 prop_assert!(same(r.x,c.x)&&same(r.y,c.y));
 }
}

#[test]
fn thresholds_flips_and_rounding_match_c() {
    for kind in 0..8 {
        for align in 0..16 {
            for crop in [false, true] {
                for sign in [-1.0, 1.0] {
                    for height in [15.9375, 15.75, 16.0] {
                        check(
                            kind, 16.0, height, 3.0, 4.0, sign, sign, 16, 16, align, crop,
                        );
                    }
                    check(kind, 1.5, 10.0, 0.0, 0.0, sign, 1.0, 2, 0, align, crop);
                    check(kind, 10.0, 10.0, 0.0, 0.0, -2.0, 1.0, 20, 0, align, crop);
                }
            }
        }
    }
}
