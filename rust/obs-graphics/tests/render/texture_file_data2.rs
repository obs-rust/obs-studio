//! Tier 1: the safe core of `gs_create_texture_file_data2`
//! (`obs_graphics::texture_file_data2`).
//!
//! Ports the assertions of `test/cmocka/test_texture_file_data2.c` over a
//! model data3 that decodes the fixture's 3x2 picture: on success data2
//! returns data3's pixels, format and size for every alpha mode; on failure
//! it returns nothing and leaves the caller's outputs alone; repeated
//! failures and successes do the same each time. Beyond the C test: data3
//! runs exactly once per call, gets a private colour-space slot that is not
//! one of the caller's outputs, and what it writes there never reaches the
//! caller. No graphics device, no allocation, no threads.

use obs_graphics::texture_file_data2::create_texture_file_data2;
use obs_graphics::texture_render::{ColorFormat, ColorSpace};

const GS_BGRX: ColorFormat = ColorFormat(4);
const GS_BGRA: ColorFormat = ColorFormat(5);
const GS_RGBA: ColorFormat = ColorFormat(3);
const GS_CS_SRGB: ColorSpace = ColorSpace(0);
const GS_CS_709_EXTENDED: ColorSpace = ColorSpace(2);

const STRAIGHT: u32 = 0;
const ALPHA_MODES: [u32; 3] = [STRAIGHT, 1, 2];

/// `straight_pixels` of the C test: the fixture `image-file.png`.
const STRAIGHT_PIXELS: [u8; 24] = [
    255, 0, 0, 255, 0, 255, 0, 128, 0, 0, 255, 255, 10, 20, 30, 255, 40, 50, 60, 0, 70, 80, 90, 255,
];

/// The caller's outputs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Outputs {
    format: ColorFormat,
    cx: u32,
    cy: u32,
}

/// A model of data3 over the fixture: the decoded pixels for a valid file
/// (premultiplied for the two premultiply modes), nothing for a failed one.
/// It writes its outputs only on success, as the FFmpeg loader does.
fn model_data3(
    valid: bool,
    mode: u32,
    out: &mut Outputs,
    space: &mut ColorSpace,
    calls: &mut u32,
) -> Option<Vec<u8>> {
    *calls += 1;
    if !valid {
        return None;
    }
    let mut pixels = STRAIGHT_PIXELS.to_vec();
    if mode != STRAIGHT {
        for px in pixels.as_chunks_mut::<4>().0 {
            let a = u32::from(px[3]);
            for c in &mut px[..3] {
                *c = (u32::from(*c) * a / 255) as u8;
            }
        }
    }
    *out = Outputs {
        format: GS_RGBA,
        cx: 3,
        cy: 2,
    };
    *space = GS_CS_SRGB;
    Some(pixels)
}

fn data2(valid: bool, mode: u32, out: &mut Outputs, calls: &mut u32) -> Option<Vec<u8>> {
    create_texture_file_data2(|space| model_data3(valid, mode, out, space, calls))
}

fn data3(valid: bool, mode: u32, out: &mut Outputs, space: &mut ColorSpace) -> Option<Vec<u8>> {
    model_data3(valid, mode, out, space, &mut 0)
}

fn check_valid(mode: u32) {
    let mut out2 = Outputs {
        format: GS_BGRA,
        cx: 91,
        cy: 92,
    };
    let mut out3 = Outputs {
        format: GS_BGRX,
        cx: 93,
        cy: 94,
    };
    let mut space = GS_CS_709_EXTENDED;
    let mut calls = 0;
    let pixels2 = data2(true, mode, &mut out2, &mut calls).expect("valid file decodes");
    let pixels3 = data3(true, mode, &mut out3, &mut space).expect("valid file decodes");
    assert_eq!(calls, 1);
    assert_eq!(
        out2,
        Outputs {
            format: GS_RGBA,
            cx: 3,
            cy: 2
        }
    );
    assert_eq!(out2, out3);
    assert_eq!(space, GS_CS_SRGB);
    assert_eq!(pixels2, pixels3);
    if mode == STRAIGHT {
        assert_eq!(pixels2, STRAIGHT_PIXELS);
    } else {
        for i in (3..STRAIGHT_PIXELS.len()).step_by(4) {
            assert_eq!(pixels2[i], STRAIGHT_PIXELS[i]);
        }
        assert_eq!(pixels2[16..19], [0, 0, 0]);
        assert!(pixels2[5] < STRAIGHT_PIXELS[5]);
    }
}

fn check_failure(mode: u32) {
    let before = Outputs {
        format: GS_BGRA,
        cx: 91,
        cy: 92,
    };
    let (mut out2, mut out3) = (before, before);
    let mut space = GS_CS_709_EXTENDED;
    let mut calls = 0;
    assert_eq!(data2(false, mode, &mut out2, &mut calls), None);
    assert_eq!(data3(false, mode, &mut out3, &mut space), None);
    assert_eq!(calls, 1);
    // Characterized, not endorsed: a failed load leaves caller outputs alone.
    assert_eq!(out2, before);
    assert_eq!(out2, out3);
    assert_eq!(space, GS_CS_709_EXTENDED);
}

#[test]
fn valid_png_all_alpha_modes() {
    for mode in ALPHA_MODES {
        check_valid(mode);
    }
}

#[test]
fn failed_loads_preserve_outputs() {
    for mode in ALPHA_MODES {
        // The C test's three failing paths ("", a missing file, a corrupt
        // file) are one case to data2: data3 fails.
        for _path in 0..3 {
            check_failure(mode);
        }
    }
}

#[test]
fn repeated_failure_and_success() {
    for _ in 0..8 {
        failed_loads_preserve_outputs();
        valid_png_all_alpha_modes();
    }
}

#[test]
fn forwards_once_and_returns_data3_result_unchanged() {
    for result in [None, Some(0u64), Some(u64::MAX), Some(0x5a5a_5a5a)] {
        let mut calls = 0;
        assert_eq!(
            create_texture_file_data2(|_| {
                calls += 1;
                result
            }),
            result
        );
        assert_eq!(calls, 1);
    }
}

#[test]
fn colour_space_slot_is_private_and_discarded() {
    let caller_space = GS_CS_709_EXTENDED;
    let caller_addr = core::ptr::from_ref(&caller_space).addr();
    let mut seen = Vec::new();
    for written in [0, 1, 2, 3, u32::MAX] {
        let slot_addr = create_texture_file_data2(|space| {
            *space = ColorSpace(written);
            core::ptr::from_mut(space).addr()
        });
        seen.push(slot_addr);
        assert_ne!(slot_addr, caller_addr);
    }
    // Nothing data3 wrote reached a slot the caller owns.
    assert_eq!(caller_space, GS_CS_709_EXTENDED);
}

#[test]
fn colour_space_slot_starts_initialized() {
    // The C slot is uninitialized and unobservable; the Rust one is a
    // defined value data3 may read.
    let start = create_texture_file_data2(|space| *space);
    assert_eq!(start, ColorSpace::default());
}
