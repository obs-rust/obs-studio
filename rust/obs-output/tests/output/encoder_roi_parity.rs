//! Tier 2 layout and Tier 3 parity: the shims of
//! `obs_output::ffi::encoder_roi` (and the safe core under them) against the
//! original `libobs/obs-encoder-roi.c.h`, compiled as
//! `oracle_obs_encoder_roi_*` (obs-c-oracle, oracle/encoder_roi.c).
//!
//! Every output is compared bit for bit: the admit as a bool, the factors
//! as `f32` bits (and left unchanged when there are none: both sides start
//! from the same sentinel), and the scaled ROI field by field, its priority
//! as `f32` bits, so -0 against +0, a different rounding or a NaN payload
//! would fail.
//!
//! Runs: the layout of `struct obs_encoder_roi` and `OBS_ENCODER_CAP_ROI`;
//! the `test_encoder_roi` values; every capability bit; sweeps of the
//! priority's bit patterns around -1, 1, 0, the infinities and the NaNs,
//! and a stride through all 2^32 of them; the edges against the factors'
//! bit patterns that put the product next to 2^32, 0 or a whole number;
//! sizes at the edges of the `u32` range and of exact floats; and
//! proptests over full-range `u32` rectangles, `f32` priorities and
//! factors drawn as bit patterns (NaN and +/-inf included), and sizes,
//! with rectangles near the 16 x 16 minimum and products near the edges.
//!
//! No input is excluded: since #1964 the C is defined on every one. No
//! intentional differences. `test_encoder_roi` runs against the hybrid
//! libobs, where these shims replace the C.

use core::ffi::c_void;
use std::mem::{align_of, offset_of, size_of};

use obs_c_oracle::encoder_roi::{
    LAYOUT_NAMES, oracle_encoder_roi_cap, oracle_encoder_roi_layout, oracle_obs_encoder_roi_admit,
    oracle_obs_encoder_roi_scale, oracle_obs_encoder_roi_scale_factors,
};
use obs_output::encoder_roi::{self as roi_core, OBS_ENCODER_CAP_ROI, Roi};
use obs_output::ffi::encoder_roi::{
    obs_encoder_roi, obs_encoder_roi_admit, obs_encoder_roi_scale, obs_encoder_roi_scale_factors,
};
use proptest::prelude::*;

/// What the factors hold before a call, so "unchanged" is checked too.
const SENTINEL: u32 = 0x7fa5_a5a5;

fn rect(top: u32, bottom: u32, left: u32, right: u32, priority: f32) -> Roi {
    Roi {
        top,
        bottom,
        left,
        right,
        priority,
    }
}

fn bits(roi: &Roi) -> [u32; 5] {
    [
        roi.top,
        roi.bottom,
        roi.left,
        roi.right,
        roi.priority.to_bits(),
    ]
}

fn check_admit(caps: u32, roi: Roi) -> bool {
    // SAFETY: a live ROI, read during the call.
    let rust = unsafe { obs_encoder_roi_admit(caps, &raw const roi) };
    // SAFETY: as above; the oracle reads it as struct obs_encoder_roi,
    // whose layout layout_matches_c checks.
    let c = unsafe { oracle_obs_encoder_roi_admit(caps, (&raw const roi).cast::<c_void>()) };
    assert_eq!(
        rust,
        c,
        "admit({caps:#x}, {roi:?} [priority {:#010x}])",
        roi.priority.to_bits()
    );
    assert_eq!(
        roi_core::admit(caps, &roi),
        c,
        "core admit({caps:#x}, {roi:?})"
    );
    c
}

/// The factors, or None, from both sides: the result and the two floats
/// (the sentinel when not written) must be equal bit for bit.
fn check_factors(sw: u32, sh: u32, w: u32, h: u32) -> Option<(u32, u32)> {
    let (mut rx, mut ry) = (f32::from_bits(SENTINEL), f32::from_bits(SENTINEL));
    let (mut cx, mut cy) = (f32::from_bits(SENTINEL), f32::from_bits(SENTINEL));
    // SAFETY: two writable floats each.
    let rust = unsafe { obs_encoder_roi_scale_factors(sw, sh, w, h, &raw mut rx, &raw mut ry) };
    // SAFETY: as above.
    let c = unsafe { oracle_obs_encoder_roi_scale_factors(sw, sh, w, h, &raw mut cx, &raw mut cy) };
    let what = format!("scale_factors({sw}, {sh}, {w}, {h})");
    assert_eq!(rust, c, "{what}");
    assert_eq!(
        (rx.to_bits(), ry.to_bits()),
        (cx.to_bits(), cy.to_bits()),
        "{what}: rust ({rx}, {ry}), C ({cx}, {cy})"
    );
    let core = roi_core::scale_factors(sw, sh, w, h).map(|(x, y)| (x.to_bits(), y.to_bits()));
    let want = c.then_some((cx.to_bits(), cy.to_bits()));
    assert_eq!(core, want, "core {what}");
    want
}

/// The scaled ROI from both sides, bit for bit, into a fresh ROI and in
/// place (the C's compound literal reads the ROI before it writes).
fn check_scale(roi: Roi, x: f32, y: f32) -> [u32; 5] {
    let filler = rect(0xdead_beef, 1, 2, 3, f32::from_bits(SENTINEL));
    let (mut rust, mut c) = (filler, filler);
    // SAFETY: a readable ROI and a writable one.
    unsafe { obs_encoder_roi_scale(&raw const roi, x, y, &raw mut rust) };
    // SAFETY: as above.
    unsafe {
        oracle_obs_encoder_roi_scale(
            (&raw const roi).cast::<c_void>(),
            x,
            y,
            (&raw mut c).cast::<c_void>(),
        );
    }
    let what = || {
        format!(
            "scale({roi:?}, {x} [{:#010x}], {y} [{:#010x}])",
            x.to_bits(),
            y.to_bits()
        )
    };
    assert_eq!(bits(&rust), bits(&c), "{}", what());
    assert_eq!(
        bits(&roi_core::scale(&roi, x, y)),
        bits(&c),
        "core {}",
        what()
    );

    let (mut rust_in_place, mut c_in_place) = (roi, roi);
    let r: *mut obs_encoder_roi = &raw mut rust_in_place;
    // SAFETY: the same ROI, readable and writable.
    unsafe { obs_encoder_roi_scale(r, x, y, r) };
    let p = (&raw mut c_in_place).cast::<c_void>();
    // SAFETY: as above.
    unsafe { oracle_obs_encoder_roi_scale(p, x, y, p) };
    assert_eq!(bits(&rust_in_place), bits(&c), "in place {}", what());
    assert_eq!(bits(&c_in_place), bits(&c), "C in place {}", what());
    bits(&c)
}

/// One edge through the scale, against the C.
fn check_edge(edge: u32, factor: f32) -> u32 {
    let out = check_scale(rect(edge, edge, edge, edge, 0.0), factor, factor);
    assert_eq!(
        roi_core::scale_edge(edge, factor),
        out[0],
        "core scale_edge"
    );
    out[0]
}

/// The float `steps` bit patterns away from `f`, staying within its sign.
fn nudge(f: f32, steps: i32) -> Option<f32> {
    f.to_bits()
        .checked_add_signed(steps)
        .map(f32::from_bits)
        .filter(|g| g.is_sign_negative() == f.is_sign_negative())
}

#[test]
fn layout_matches_c() {
    let rust = [
        size_of::<obs_encoder_roi>(),
        align_of::<obs_encoder_roi>(),
        offset_of!(obs_encoder_roi, top),
        offset_of!(obs_encoder_roi, bottom),
        offset_of!(obs_encoder_roi, left),
        offset_of!(obs_encoder_roi, right),
        offset_of!(obs_encoder_roi, priority),
    ];
    for (i, (name, rust)) in LAYOUT_NAMES.iter().zip(rust).enumerate() {
        // SAFETY: a plain value lookup.
        let c = unsafe { oracle_encoder_roi_layout(i) };
        assert_eq!(rust, c, "{name}");
    }
    // SAFETY: as above.
    assert_eq!(
        unsafe { oracle_encoder_roi_layout(LAYOUT_NAMES.len()) },
        usize::MAX
    );
    // SAFETY: a constant.
    assert_eq!(OBS_ENCODER_CAP_ROI, unsafe { oracle_encoder_roi_cap() });
}

/// Every rectangle and size of `test_encoder_roi`, with and without the
/// capability.
#[test]
fn characterization_values_match() {
    let rects = [
        rect(0, 64, 0, 64, 0.5),
        rect(100, 115, 200, 300, 0.5),
        rect(100, 116, 200, 300, 0.5),
        rect(100, 117, 200, 300, 0.5),
        rect(100, 200, 200, 215, 0.5),
        rect(100, 200, 200, 216, 0.5),
        rect(100, 200, 200, 217, 0.5),
        rect(100, 115, 200, 215, 0.5),
        rect(0, 16, 0, 16, 0.5),
        rect(100, 100, 200, 300, 0.5),
        rect(100, 200, 200, 200, 0.5),
        rect(u32::MAX - 16, u32::MAX, u32::MAX - 16, u32::MAX, 0.5),
        rect(u32::MAX - 15, u32::MAX, 0, 16, 0.5),
        rect(0, 16, 0, 16, -1.0),
        rect(0, 16, 0, 16, 1.0),
        rect(0, 16, 0, 16, 0.0),
        rect(0, 16, 0, 16, -0.0),
        rect(0, 16, 0, 16, -f32::from_bits(0x3f80_0001)),
        rect(0, 16, 0, 16, f32::from_bits(0x3f80_0001)),
        rect(0, 16, 0, 16, f32::NEG_INFINITY),
        rect(0, 16, 0, 16, f32::INFINITY),
        rect(0, 16, 0, 16, f32::NAN),
        rect(0, 16, 0, 16, -f32::NAN),
        rect(200, 100, 0, 64, 0.5),
        rect(0, 64, 200, 100, 0.5),
        rect(200, 100, 200, 100, 0.5),
        rect(101, 100, 0, 64, 0.5),
        rect(0, 64, 101, 100, 0.5),
        rect(u32::MAX, 0, 0, 64, 0.5),
        rect(16, 32, 16, 32, -0.5),
        rect(0, 1080, 0, 1920, 1.0),
        rect(33, 101, 7, 1001, -1.0),
        rect(1000, 2000, 3000, 4000, 0.25),
        rect(100, 200, 300, 400, 0.25),
        rect(0, 0xFF_FFFF, 16, 0x100_0000, 0.5),
        rect(0x100_0000, u32::MAX, 0xFF_FFFF, u32::MAX, -0.5),
    ];
    let sizes = [
        (1280, 720, 1920, 1080),
        (3840, 540, 1920, 1080),
        (4096, 4096, 16, 16),
        (1280, 720, 0, 0),
    ];
    for roi in rects {
        check_admit(OBS_ENCODER_CAP_ROI, roi);
        check_admit(0, roi);
        for (sw, sh, w, h) in sizes {
            if let Some((x, y)) = check_factors(sw, sh, w, h) {
                check_scale(roi, f32::from_bits(x), f32::from_bits(y));
            }
        }
    }
}

#[test]
fn every_cap_bit_matches() {
    for roi in [rect(0, 16, 0, 16, 0.5), rect(0, 15, 0, 16, 0.5)] {
        for bit in 0..32 {
            check_admit(1 << bit, roi);
            check_admit(!(1u32 << bit), roi);
        }
        check_admit(0, roi);
        check_admit(u32::MAX, roi);
    }
}

/// Every rectangle with edges within 40 of 0, 1000 and `u32::MAX`, in
/// either direction.
#[test]
fn small_rects_match() {
    for base in [0u32, 1000, u32::MAX - 40] {
        for a in base..=base + 40 {
            for b in base..=base + 40 {
                check_admit(OBS_ENCODER_CAP_ROI, rect(a, b, 7, 23, 0.0));
                check_admit(OBS_ENCODER_CAP_ROI, rect(7, 23, a, b, 0.0));
                check_admit(OBS_ENCODER_CAP_ROI, rect(a, b, a, b, 1.0));
            }
        }
    }
}

/// The priority's bit patterns within 256 of -1, 1, +/-0, the smallest
/// normals, the largest finite floats, the infinities and the NaNs (quiet,
/// signalling, either sign), and every 65521st bit pattern of all 2^32.
#[test]
fn priority_bits_match() {
    let valid = rect(0, 16, 0, 16, 0.0);
    let centers = [
        0x3f80_0000u32,
        0xbf80_0000,
        0x0000_0000,
        0x8000_0000,
        0x0080_0000,
        0x8080_0000,
        0x7f7f_ffff,
        0xff7f_ffff,
        0x7f80_0000,
        0xff80_0000,
        0x7fc0_0000,
        0xffc0_0000,
        0x7fff_ffff,
        0xffff_ffff,
    ];
    for center in centers {
        for d in -256i32..=256 {
            let Some(b) = center.checked_add_signed(d) else {
                continue;
            };
            check_admit(
                OBS_ENCODER_CAP_ROI,
                Roi {
                    priority: f32::from_bits(b),
                    ..valid
                },
            );
        }
    }
    let mut b = 0u32;
    loop {
        check_admit(
            OBS_ENCODER_CAP_ROI,
            Roi {
                priority: f32::from_bits(b),
                ..valid
            },
        );
        match b.checked_add(65_521) {
            Some(next) => b = next,
            None => break,
        }
    }
}

/// Sizes at the `u32` edges, at the edges of exact floats (2^24) and
/// small ones, every pair against every pair.
#[test]
fn factor_sizes_match() {
    let sizes = [
        0u32,
        1,
        2,
        3,
        7,
        16,
        720,
        1080,
        1280,
        1920,
        3840,
        16_777_215,
        16_777_216,
        16_777_217,
        16_777_219,
        2_147_483_647,
        2_147_483_648,
        4_294_967_167,
        4_294_967_168,
        u32::MAX - 1,
        u32::MAX,
    ];
    for sw in sizes {
        for w in sizes {
            check_factors(sw, 1080, w, 1080);
            check_factors(1920, sw, 1920, w);
            check_factors(sw, sw, w, w);
        }
    }
}

/// Each edge against the factors' bit patterns that put the product next
/// to 2^32 (the saturation), next to 0, and next to whole numbers (the
/// truncation and the product's rounding), plus the special factors.
#[test]
fn edge_bits_match() {
    let edges = [
        0u32,
        1,
        2,
        3,
        5,
        7,
        15,
        16,
        255,
        256,
        1079,
        1080,
        1919,
        1920,
        0xFF_FFFF,
        0x100_0000,
        0x100_0001,
        2_147_483_647,
        2_147_483_648,
        4_294_967_040,
        4_294_967_167,
        4_294_967_168,
        u32::MAX,
    ];
    let specials = [
        0.0,
        -0.0,
        1.0,
        -1.0,
        0.5,
        f32::MAX,
        f32::MIN,
        f32::MIN_POSITIVE,
        f32::from_bits(1),
        f32::from_bits(0x8000_0001),
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NAN,
        f32::from_bits(0xffc0_0001),
        f32::from_bits(0x7f80_0001),
    ];
    for edge in edges {
        for f in specials {
            check_edge(edge, f);
        }
        if edge == 0 {
            continue;
        }
        let e = f64::from(edge as f32);
        // 2^32 / edge, and n / edge for some whole n.
        let mut targets = vec![4_294_967_296.0f64, 4_294_967_040.0, 1.0, 16.0];
        targets.extend((1..=8).map(|n| f64::from(n) * 1000.0));
        targets.extend((1..=8).map(f64::from));
        for target in targets {
            let f = (target / e) as f32;
            for d in -8..=8 {
                if let Some(g) = nudge(f, d) {
                    check_edge(edge, g);
                    check_edge(edge, -g);
                }
            }
        }
    }
}

/// Random edges, factors and sizes through both sides.
fn any_f32() -> impl Strategy<Value = f32> {
    any::<u32>().prop_map(f32::from_bits)
}

/// A priority biased to the edges of [-1, 1], the NaNs and the
/// infinities, or any bit pattern.
fn priority() -> impl Strategy<Value = f32> {
    prop_oneof![
        3 => any_f32(),
        1 => (
            prop::sample::select(vec![
                0x3f80_0000u32,
                0xbf80_0000,
                0x0000_0000,
                0x8000_0000,
                0x7f80_0000,
                0xff80_0000,
                0x7fc0_0000,
                0xffc0_0000,
            ]),
            -64i32..=64,
        )
            .prop_map(|(c, d)| f32::from_bits(c.wrapping_add_signed(d))),
    ]
}

/// A side as (start, end): full range, or near the 16 minimum in either
/// direction, wrapping at the `u32` edges.
fn side() -> impl Strategy<Value = (u32, u32)> {
    prop_oneof![
        any::<(u32, u32)>(),
        (any::<u32>(), -24i32..=40).prop_map(|(a, d)| (a, a.wrapping_add_signed(d))),
        (
            prop::sample::select(vec![0u32, u32::MAX - 15, u32::MAX - 16, u32::MAX]),
            -24i32..=40
        )
            .prop_map(|(a, d)| (a, a.wrapping_add_signed(d))),
    ]
}

fn any_roi() -> impl Strategy<Value = Roi> {
    (side(), side(), priority()).prop_map(|((top, bottom), (left, right), priority)| Roi {
        top,
        bottom,
        left,
        right,
        priority,
    })
}

/// A size: full range, small, or at the edges of exact floats and `u32`.
fn size() -> impl Strategy<Value = u32> {
    prop_oneof![
        any::<u32>(),
        0u32..=4096,
        (
            prop::sample::select(vec![16_777_216u32, 2_147_483_648, u32::MAX]),
            -4i32..=4
        )
            .prop_map(|(c, d)| c.wrapping_add_signed(d)),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(8192))]

    #[test]
    fn random_admit_matches_oracle(caps in prop_oneof![
        Just(OBS_ENCODER_CAP_ROI),
        any::<u32>(),
    ], roi in any_roi()) {
        check_admit(caps, roi);
        check_admit(caps | OBS_ENCODER_CAP_ROI, roi);
    }

    #[test]
    fn random_full_range_admit_matches_oracle(
        caps in any::<u32>(),
        top in any::<u32>(),
        bottom in any::<u32>(),
        left in any::<u32>(),
        right in any::<u32>(),
        priority in any_f32(),
    ) {
        check_admit(caps, rect(top, bottom, left, right, priority));
        check_admit(OBS_ENCODER_CAP_ROI, rect(top, bottom, left, right, priority));
    }

    #[test]
    fn random_factors_match_oracle(sw in size(), sh in size(), w in size(), h in size()) {
        check_factors(sw, sh, w, h);
    }

    #[test]
    fn random_scale_matches_oracle(
        top in any::<u32>(),
        bottom in any::<u32>(),
        left in any::<u32>(),
        right in any::<u32>(),
        priority in any_f32(),
        x in any_f32(),
        y in any_f32(),
    ) {
        check_scale(rect(top, bottom, left, right, priority), x, y);
    }

    /// Factors that put the product within 8 floats of 2^32 or of a whole
    /// number, either sign.
    #[test]
    fn random_scale_near_edges_matches_oracle(
        edge in 1u32..,
        target in prop_oneof![
            Just(4_294_967_296.0f64),
            (0u32..=1 << 24).prop_map(f64::from),
            any::<u32>().prop_map(f64::from),
        ],
        d in -8i32..=8,
        negative in any::<bool>(),
    ) {
        let f = (target / f64::from(edge as f32)) as f32;
        if let Some(g) = nudge(f, d) {
            let g = if negative { -g } else { g };
            check_edge(edge, g);
        }
    }

    /// The walk of obs_encoder_enum_roi(): factors from the sizes, then
    /// the ROI scaled by them.
    #[test]
    fn random_enum_chain_matches_oracle(
        roi in any_roi(),
        sw in size(),
        sh in size(),
        w in size(),
        h in size(),
    ) {
        if let Some((x, y)) = check_factors(sw, sh, w, h) {
            check_scale(roi, f32::from_bits(x), f32::from_bits(y));
        }
    }
}
