//! Tier 1: `obs_output::encoder_roi`, the encoder region of interest (ROI)
//! checks and scaling.
//!
//! The cases without a prefix port the cases of
//! `test/cmocka/test_encoder_roi.c` that reach the checks and the scaling,
//! under the same names, in the same order, with the same values and
//! expectations: `encoder_without_cap_refuses`, `minimum_size`,
//! `priority_range`, `nan_priority`, `upside_down_rect`,
//! `enum_order_scaled`, `enum_without_video_size` and
//! `enum_scaled_saturates`. The C goes through `obs_encoder_add_roi()` and
//! `obs_encoder_enum_roi()`; here [`admit`] is what decides "stored", and
//! [`scale_factors`] then [`scale`] are what `obs_encoder_enum_roi()` passes
//! on. Its other four cases (the NULL ROI, `roi_increment` after add and
//! clear, the unscaled walk order and the NULL encoder) test the list, the
//! lock and the guards, which stay C, and have no Tier 1 twin.
//!
//! The `edge_*` cases are new: every capability bit, every small rectangle
//! against the model, the floats next to the priority range, NaN payloads,
//! the factors' conversion and rounding, the edges of the saturation, the
//! product's rounding in `f32`, and which factor scales which edge.
//!
//! Cases marked characterized, not endorsed, are what the C does today,
//! including the odd parts.

use obs_output::encoder_roi::{
    MIN_SIZE, OBS_ENCODER_CAP_ROI, Roi, admit, scale, scale_edge, scale_factors,
};

fn rect(top: u32, bottom: u32, left: u32, right: u32, priority: f32) -> Roi {
    Roi {
        top,
        bottom,
        left,
        right,
        priority,
    }
}

/// What `obs_encoder_add_roi()` decides for an encoder with
/// `OBS_ENCODER_CAP_ROI`, as the C test's `expect_add` checks it.
fn expect_add(roi: Roi, stored: bool) {
    assert_eq!(admit(OBS_ENCODER_CAP_ROI, &roi), stored, "admit({roi:?})");
}

/// Bit for bit: -0 is not +0, and a NaN keeps its payload.
fn check_float(got: f32, want: f32, what: &str) {
    assert!(
        got.to_bits() == want.to_bits(),
        "{what}: {got} ({:#010x}), expected {want} ({:#010x})",
        got.to_bits(),
        want.to_bits()
    );
}

fn assert_roi(roi: &Roi, top: u32, bottom: u32, left: u32, right: u32, priority: f32) {
    assert_eq!(
        (roi.top, roi.bottom, roi.left, roi.right),
        (top, bottom, left, right),
        "{roi:?}"
    );
    check_float(roi.priority, priority, "priority");
}

/// `obs_encoder_enum_roi()` for an encoder whose video is `width` x
/// `height`, scaled to `scaled_width` x `scaled_height`: the ROIs it passes
/// on, last added first, or none without factors.
fn enum_scaled(
    rois: &[Roi],
    scaled_width: u32,
    scaled_height: u32,
    width: u32,
    height: u32,
) -> Vec<Roi> {
    match scale_factors(scaled_width, scaled_height, width, height) {
        Some((x, y)) => rois.iter().rev().map(|roi| scale(roi, x, y)).collect(),
        None => Vec::new(),
    }
}

#[test]
fn encoder_without_cap_refuses() {
    assert!(!admit(0, &rect(0, 64, 0, 64, 0.5)));
}

/// The smallest ROI is one 16 x 16 block, in each direction on its own.
#[test]
fn minimum_size() {
    expect_add(rect(100, 115, 200, 300, 0.5), false);
    expect_add(rect(100, 116, 200, 300, 0.5), true);
    expect_add(rect(100, 117, 200, 300, 0.5), true);

    expect_add(rect(100, 200, 200, 215, 0.5), false);
    expect_add(rect(100, 200, 200, 216, 0.5), true);
    expect_add(rect(100, 200, 200, 217, 0.5), true);

    expect_add(rect(100, 115, 200, 215, 0.5), false);
    expect_add(rect(0, 16, 0, 16, 0.5), true);
    expect_add(rect(100, 100, 200, 300, 0.5), false);
    expect_add(rect(100, 200, 200, 200, 0.5), false);
    expect_add(
        rect(u32::MAX - 16, u32::MAX, u32::MAX - 16, u32::MAX, 0.5),
        true,
    );
    expect_add(rect(u32::MAX - 15, u32::MAX, 0, 16, 0.5), false);
}

#[test]
fn priority_range() {
    expect_add(rect(0, 16, 0, 16, -1.0), true);
    expect_add(rect(0, 16, 0, 16, 1.0), true);
    expect_add(rect(0, 16, 0, 16, 0.0), true);
    expect_add(rect(0, 16, 0, 16, -0.0), true);
    // The floats just outside [-1, 1].
    expect_add(rect(0, 16, 0, 16, -f32::from_bits(0x3f80_0001)), false);
    expect_add(rect(0, 16, 0, 16, f32::from_bits(0x3f80_0001)), false);
    expect_add(rect(0, 16, 0, 16, f32::NEG_INFINITY), false);
    expect_add(rect(0, 16, 0, 16, f32::INFINITY), false);
}

/// A NaN priority is not in [-1, 1]. NaN < -1 and NaN > 1 are both false,
/// so the range check alone stored it (#1959).
#[test]
fn nan_priority() {
    expect_add(rect(0, 16, 0, 16, f32::NAN), false);
    expect_add(rect(0, 16, 0, 16, -f32::NAN), false);
}

/// An upside-down rectangle (bottom above top, or right left of left) is
/// refused. The size check subtracts in `uint32_t`, so on its own it wrapped
/// to a large size and stored the rectangle (#1959).
#[test]
fn upside_down_rect() {
    expect_add(rect(200, 100, 0, 64, 0.5), false);
    expect_add(rect(0, 64, 200, 100, 0.5), false);
    expect_add(rect(200, 100, 200, 100, 0.5), false);
    expect_add(rect(101, 100, 0, 64, 0.5), false);
    expect_add(rect(0, 64, 101, 100, 0.5), false);
    expect_add(rect(u32::MAX, 0, 0, 64, 0.5), false);
}

/// With a scaled size each edge is multiplied in float by scaled / video
/// size and truncated toward 0, still last added first.
#[test]
fn enum_order_scaled() {
    let rois = [
        rect(0, 1080, 0, 1920, 1.0),
        rect(33, 101, 7, 1001, -1.0),
        rect(100, 200, 300, 400, 0.25),
    ];
    for roi in &rois {
        expect_add(*roi, true);
    }
    let seen = enum_scaled(&rois, 1280, 720, 1920, 1080);
    assert_eq!(seen.len(), 3);
    // x 2/3 (0.6666667f): 66.666664 -> 66, 133.33333 -> 133.
    assert_roi(&seen[0], 66, 133, 200, 266, 0.25);
    assert_roi(&seen[1], 22, 67, 4, 667, -1.0);
    assert_roi(&seen[2], 0, 720, 0, 1280, 1.0);

    let roi = rect(33, 101, 7, 1001, 0.5);
    expect_add(roi, true);
    let seen = enum_scaled(&[roi], 3840, 540, 1920, 1080);
    assert_eq!(seen.len(), 1);
    assert_roi(&seen[0], 16, 50, 14, 2002, 0.5);
}

/// A scaled size with no video to scale from: nothing is passed on.
#[test]
fn enum_without_video_size() {
    let roi = rect(0, 16, 0, 16, 0.5);
    expect_add(roi, true);
    assert!(enum_scaled(&[roi], 1280, 720, 0, 0).is_empty());
}

/// A scaled edge at 2^32 or past it is `UINT32_MAX`, the nearest value in
/// range. The `(uint32_t)` cast of such a float is undefined in C (#1959).
#[test]
fn enum_scaled_saturates() {
    // x 256: 2^24 - 1 x 256 is 2^32 - 256, in range; 2^24 x 256 is 2^32.
    let rois = [
        rect(0, 0xFF_FFFF, 16, 0x100_0000, 0.5),
        rect(0x100_0000, u32::MAX, 0xFF_FFFF, u32::MAX, -0.5),
    ];
    for roi in &rois {
        expect_add(*roi, true);
    }
    let seen = enum_scaled(&rois, 4096, 4096, 16, 16);
    assert_eq!(seen.len(), 2);
    assert_roi(&seen[0], u32::MAX, u32::MAX, 0xFFFF_FF00, u32::MAX, -0.5);
    assert_roi(&seen[1], 0, 0xFFFF_FF00, 4096, u32::MAX, 0.5);
}

/// Only `OBS_ENCODER_CAP_ROI` (bit 4) matters; every other bit is ignored.
#[test]
fn edge_caps_only_the_roi_bit() {
    let roi = rect(0, 16, 0, 16, 0.5);
    assert_eq!(OBS_ENCODER_CAP_ROI, 1 << 4);
    for bit in 0..32 {
        let caps = 1u32 << bit;
        assert_eq!(admit(caps, &roi), bit == 4, "caps {caps:#x}");
    }
    assert!(admit(u32::MAX, &roi));
    assert!(!admit(!OBS_ENCODER_CAP_ROI, &roi));
    // Without the bit, even a valid ROI is refused before anything else.
    assert!(!admit(0, &rect(0, u32::MAX, 0, u32::MAX, 0.0)));
}

/// Every rectangle with edges in 0..=40 (at the bottom of the range) and
/// in `u32::MAX - 40..=u32::MAX` (at the top) against the model: not
/// upside down or empty, and at least 16 each way.
#[test]
fn edge_every_small_rect() {
    assert_eq!(MIN_SIZE, 16);
    let model = |a: u32, b: u32| b > a && b - a >= 16;
    for base in [0u32, 1000, u32::MAX - 40] {
        for top in base..=base + 40 {
            for bottom in base..=base + 40 {
                let ok = model(top, bottom);
                expect_add(rect(top, bottom, 7, 23, 0.0), ok);
                expect_add(rect(7, 23, top, bottom, 0.0), ok);
                expect_add(rect(top, bottom, top, bottom, 0.0), ok);
            }
        }
    }
    // One side 16, the other upside down by one: still refused.
    expect_add(rect(0, 16, 17, 16, 0.0), false);
    expect_add(rect(17, 16, 0, 16, 0.0), false);
    // The whole range each way.
    expect_add(rect(0, u32::MAX, 0, u32::MAX, 0.0), true);
}

/// The floats at and inside the edges of [-1, 1] are stored, the ones past
/// them and every NaN are not.
#[test]
fn edge_priority_floats() {
    let inside = [
        1.0,
        -1.0,
        f32::from_bits(0x3f7f_ffff),
        -f32::from_bits(0x3f7f_ffff),
        f32::MIN_POSITIVE,
        -f32::MIN_POSITIVE,
        f32::from_bits(1),
        f32::from_bits(0x8000_0001),
        0.5,
        -0.25,
    ];
    for p in inside {
        expect_add(rect(0, 16, 0, 16, p), true);
    }
    let outside = [
        f32::from_bits(0x3f80_0001),
        -f32::from_bits(0x3f80_0001),
        2.0,
        -2.0,
        f32::MAX,
        f32::MIN,
        f32::INFINITY,
        f32::NEG_INFINITY,
    ];
    for p in outside {
        expect_add(rect(0, 16, 0, 16, p), false);
    }
    // Quiet and signalling NaNs, either sign, any payload.
    for bits in [
        0x7fc0_0000u32,
        0xffc0_0000,
        0x7f80_0001,
        0xff80_0001,
        0x7fff_ffff,
        0xffff_ffff,
        0x7fa0_0000,
    ] {
        let p = f32::from_bits(bits);
        assert!(p.is_nan());
        expect_add(rect(0, 16, 0, 16, p), false);
    }
}

/// The checks run in the C's order, so a rectangle that fails several is
/// refused either way; a NaN priority is refused even when the rectangle
/// is the smallest valid one, and a valid priority does not save a small
/// rectangle.
#[test]
fn edge_each_check_alone() {
    expect_add(rect(0, 15, 0, 16, 0.0), false);
    expect_add(rect(0, 16, 0, 15, 0.0), false);
    expect_add(rect(0, 16, 0, 16, f32::NAN), false);
    expect_add(rect(0, 16, 0, 16, 1.5), false);
    expect_add(rect(16, 0, 16, 0, f32::NAN), false);
    expect_add(rect(0, 16, 0, 16, 1.0), true);
}

/// The factors are `(float)scaled / (float)size`: each side becomes the
/// nearest float before the division, which rounds to the nearest float.
#[test]
fn edge_scale_factors_rounding() {
    let (x, y) = scale_factors(1280, 720, 1920, 1080).unwrap();
    check_float(x, 2.0 / 3.0, "1280 / 1920");
    check_float(y, 2.0 / 3.0, "720 / 1080");
    assert_eq!(x.to_bits(), 0x3f2a_aaab);

    let (x, y) = scale_factors(1, 2, 3, 3).unwrap();
    assert_eq!((x.to_bits(), y.to_bits()), (0x3eaa_aaab, 0x3f2a_aaab));

    // Characterized, not endorsed: 16777217 is 2^24 as a float, so
    // 16777216 / 16777217 is exactly 1, not the float below it.
    let (x, y) = scale_factors(16_777_216, 16_777_217, 16_777_217, 16_777_216).unwrap();
    check_float(x, 1.0, "2^24 / (2^24 + 1)");
    check_float(y, 1.0, "(2^24 + 1) / 2^24");

    // u32::MAX is 2^32 as a float.
    let (x, y) = scale_factors(u32::MAX, 1, 1, u32::MAX).unwrap();
    check_float(x, 4_294_967_296.0, "u32::MAX / 1");
    check_float(y, 1.0 / 4_294_967_296.0, "1 / u32::MAX");

    // A scaled size of 0 is a factor of +0 (obs_encoder_enum_roi() never
    // asks for one: it needs both scaled sides).
    let (x, y) = scale_factors(0, 0, 1920, 1080).unwrap();
    check_float(x, 0.0, "0 / 1920");
    check_float(y, 0.0, "0 / 1080");

    // Equal sizes are exactly 1.
    let (x, y) = scale_factors(u32::MAX, 7, u32::MAX, 7).unwrap();
    check_float(x, 1.0, "u32::MAX / u32::MAX");
    check_float(y, 1.0, "7 / 7");
}

/// No factors when either side of the video is 0, whatever the scaled
/// size.
#[test]
fn edge_scale_factors_need_a_size() {
    for (w, h) in [(0, 0), (0, 1080), (1920, 0), (0, u32::MAX), (u32::MAX, 0)] {
        for (sw, sh) in [(0, 0), (1280, 720), (u32::MAX, u32::MAX)] {
            assert_eq!(scale_factors(sw, sh, w, h), None, "{sw}x{sh} from {w}x{h}");
        }
    }
    assert!(scale_factors(0, 0, 1, 1).is_some());
}

/// The saturation: NaN and below 0 are 0, 2^32 and up `u32::MAX`, the rest
/// truncated toward 0, at each edge.
#[test]
fn edge_scale_edge_saturation() {
    // NaN: a NaN factor, and 0 x inf.
    assert_eq!(scale_edge(100, f32::NAN), 0);
    assert_eq!(scale_edge(100, f32::from_bits(0xffc0_0001)), 0);
    assert_eq!(scale_edge(0, f32::INFINITY), 0);
    assert_eq!(scale_edge(0, f32::NEG_INFINITY), 0);
    // Below 0, down to -inf, and -0.
    assert_eq!(scale_edge(1, -f32::from_bits(1)), 0);
    assert_eq!(scale_edge(100, -0.5), 0);
    assert_eq!(scale_edge(u32::MAX, -1.0), 0);
    assert_eq!(scale_edge(1, f32::NEG_INFINITY), 0);
    assert_eq!(scale_edge(100, -0.0), 0);
    // Truncated toward 0 just above 0 and below 1.
    assert_eq!(scale_edge(1, f32::from_bits(1)), 0);
    assert_eq!(scale_edge(1, f32::from_bits(0x3f7f_ffff)), 0);
    assert_eq!(scale_edge(1, 1.0), 1);
    // The largest float below 2^32 is in range; 2^32 and up are u32::MAX.
    assert_eq!(scale_edge(1, f32::from_bits(0x4f7f_ffff)), 0xFFFF_FF00);
    assert_eq!(scale_edge(1, 4_294_967_296.0), u32::MAX);
    assert_eq!(scale_edge(2, 4_294_967_296.0), u32::MAX);
    assert_eq!(scale_edge(1, f32::MAX), u32::MAX);
    assert_eq!(scale_edge(1, f32::INFINITY), u32::MAX);
    assert_eq!(scale_edge(0xFF_FFFF, 256.0), 0xFFFF_FF00);
    assert_eq!(scale_edge(0x100_0000, 256.0), u32::MAX);
    // Characterized, not endorsed: u32::MAX is 2^32 as a float, so it
    // saturates even at a factor of 1.
    assert_eq!(scale_edge(u32::MAX, 1.0), u32::MAX);
    // The edge is a float first: ties to even at 2^32 - 128.
    assert_eq!(scale_edge(4_294_967_167, 1.0), 0xFFFF_FF00);
    assert_eq!(scale_edge(4_294_967_168, 1.0), u32::MAX);
    // 0 is 0 at every finite factor.
    for f in [0.0, 1.0, f32::MAX, -f32::MAX, f32::MIN_POSITIVE] {
        assert_eq!(scale_edge(0, f), 0, "0 x {f}");
    }
}

/// Characterized, not endorsed: the edge is a float before the multiply,
/// so past 2^24 it loses its low bits even at a factor of 1.
#[test]
fn edge_scale_edge_loses_low_bits_past_2_24() {
    assert_eq!(scale_edge(16_777_215, 1.0), 16_777_215);
    assert_eq!(scale_edge(16_777_216, 1.0), 16_777_216);
    assert_eq!(scale_edge(16_777_217, 1.0), 16_777_216);
    assert_eq!(scale_edge(16_777_219, 1.0), 16_777_220);
    assert_eq!(scale_edge(2_147_483_647, 1.0), 2_147_483_648);
}

/// The product is one `f32` multiply, rounded to the nearest float, then
/// truncated: 3 x 1.6666666 is 4.99999988 exactly, which rounds to 5.0f,
/// so the edge is 5, where an exact (or `f64`) product would give 4.
#[test]
fn edge_scale_edge_product_is_f32() {
    let f = f32::from_bits(0x3fd5_5555);
    assert_eq!(scale_edge(3, f), 5);
    assert_eq!(scale_edge(3, f32::from_bits(0x4015_5555)), 7);
    assert_eq!(scale_edge(3, f32::from_bits(0x4055_5555)), 10);
    // And 5 / 3 from scale_factors() is that factor: the edge at the
    // video's side maps to the scaled side.
    let (x, _) = scale_factors(5, 1, 3, 1).unwrap();
    assert_eq!(x.to_bits(), 0x3fd5_5555);
    assert_eq!(scale(&rect(0, 3, 0, 3, 0.0), x, x).right, 5);
}

/// Top and bottom are scaled by `scale_y`, left and right by `scale_x`;
/// the priority is kept bit for bit, even a NaN or -0.
#[test]
fn edge_scale_factor_per_edge() {
    let roi = rect(10, 20, 30, 40, 0.75);
    assert_roi(&scale(&roi, 2.0, 3.0), 30, 60, 60, 80, 0.75);
    assert_roi(&scale(&roi, 3.0, 2.0), 20, 40, 90, 120, 0.75);
    assert_roi(&scale(&roi, 1.0, 0.0), 0, 0, 30, 40, 0.75);
    assert_roi(&scale(&roi, f32::NAN, 1.0), 10, 20, 0, 0, 0.75);
    assert_roi(&scale(&roi, 1.0, -1.0), 0, 0, 30, 40, 0.75);
    for bits in [0x8000_0000u32, 0x7fc0_1234, 0xff80_0001, 0x3f80_0001] {
        let p = f32::from_bits(bits);
        let s = scale(&rect(1, 2, 3, 4, p), 1.0, 1.0);
        assert_eq!(s.priority.to_bits(), bits);
    }
    // Characterized, not endorsed: scaling does not keep a rectangle
    // admitted. At x 1/32 a 16 x 16 ROI at the origin is 0 x 0.
    let small = rect(0, 16, 0, 16, 0.5);
    expect_add(small, true);
    let (x, y) = scale_factors(60, 34, 1920, 1088).unwrap();
    assert_roi(&scale(&small, x, y), 0, 0, 0, 0, 0.5);
    // ... and edges past the video are scaled past the scaled size.
    let past = rect(0, 4000, 0, 4000, 0.5);
    let (x, y) = scale_factors(1280, 720, 1920, 1080).unwrap();
    assert_roi(&scale(&past, x, y), 0, 2666, 0, 2666, 0.5);
}

/// `Roi` is `struct obs_encoder_roi`: four `uint32_t` edges and a `float`,
/// 20 bytes with no padding. Tier 3 checks it against the C's layout.
#[test]
fn edge_roi_layout() {
    use std::mem::{align_of, offset_of, size_of};
    assert_eq!(size_of::<Roi>(), 20);
    assert_eq!(align_of::<Roi>(), 4);
    assert_eq!(
        [
            offset_of!(Roi, top),
            offset_of!(Roi, bottom),
            offset_of!(Roi, left),
            offset_of!(Roi, right),
            offset_of!(Roi, priority),
        ],
        [0, 4, 8, 12, 16]
    );
}

/// The edge is rounded to a float before the multiply, not with it:
/// 2^24 + 1 is 2^24 as a float, so x 0.75 is 12582912, where the exact
/// product 12582912.75 would round to 12582913 (added for the mutation
/// check: keeping the edge exact passed the cases above).
#[test]
fn edge_scale_edge_rounds_the_edge_first() {
    assert_eq!(scale_edge(16_777_217, 0.75), 12_582_912);
    assert_eq!(scale_edge(16_777_219, 0.75), 12_582_915);
    assert_eq!(scale_edge(2_147_483_647, 0.75), 1_610_612_736);
}
