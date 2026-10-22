//! Tier 1: the safe core of `libobs/obs-video-plan.c.h`.
//!
//! The first ten tests mirror `test/cmocka/test_video_output_plan.c` one to
//! one, with the same names, in the same order, with the same inputs and
//! the same expected values. Like the C test, [`expected_plan`] writes the
//! plan out per format, from the code as it was in `obs.c`, and every
//! combination of format, color space and size is compared field by field,
//! floats bit for bit. A C enum value past the end (`VIDEO_FORMAT_R10L + 1`,
//! `VIDEO_CS_2100_HLG + 1`, `OBS_SCALE_AREA + 1`, `-1`) is `None` here,
//! which takes the C's `default:` branch.
//!
//! The cases after those pin the edges of the safe API: the enum tables, a
//! zero size, sizes far past the maximum, and proptests over every `u32`.

use std::ffi::CStr;

use obs_core::video_plan::{
    GsColorFormat, GsColorSpace, NUM_CHANNELS, ObsScaleType, OutputPlan, SIZE_MAX, SIZE_MIN,
    VideoPlane, align_output_size, output_plan, output_size_valid, scale_type_name, size_valid,
};
use obs_media_io::video_io::{VideoColorspace, VideoFormat};
use proptest::prelude::*;

/// Every `enum video_format` value, and one past the end (`None`).
fn all_formats() -> Vec<Option<VideoFormat>> {
    VideoFormat::ALL
        .into_iter()
        .map(Some)
        .chain([None])
        .collect()
}

/// Every `enum video_colorspace` value, and one past the end (`None`).
fn all_colorspaces() -> Vec<Option<VideoColorspace>> {
    VideoColorspace::ALL
        .into_iter()
        .map(Some)
        .chain([None])
        .collect()
}

/// Odd and even, the smallest, the 1366 that aligns to 1364, the largest.
const SIZES: [u32; 17] = [
    1, 2, 3, 4, 5, 6, 7, 8, 9, 767, 1080, 1081, 1364, 1366, 1920, 32767, 32768,
];

// ---------------------------------------------------------------------------
// expectations, per format

#[derive(Clone, Copy, PartialEq)]
enum Texel {
    /// `conversion_width_i` and `_height_i` stay 0.
    None,
    /// Only `conversion_width_i`.
    Width,
    Both,
}

/// The plane is `width / wdiv` by `height / hdiv`; `wdiv == 0` is no plane.
#[derive(Clone, Copy)]
struct PlaneRule {
    wdiv: u32,
    hdiv: u32,
    format: GsColorFormat,
}

const NO_PLANE: PlaneRule = PlaneRule {
    wdiv: 0,
    hdiv: 0,
    format: GsColorFormat::Unknown,
};

const fn plane(wdiv: u32, hdiv: u32, format: GsColorFormat) -> PlaneRule {
    PlaneRule { wdiv, hdiv, format }
}

type Techs = [Option<&'static str>; 3];

struct FormatRule {
    format: VideoFormat,
    /// Techniques for SDR, PQ and HLG; one set for every color space when
    /// the PQ and HLG sets are `None`.
    techs: [Option<Techs>; 3],
    texel: Texel,
    planes: [PlaneRule; 3],
}

const SDR: usize = 0;
const PQ: usize = 1;
const HLG: usize = 2;

fn rules() -> Vec<FormatRule> {
    use GsColorFormat::{R8, R8g8, R16, Rg16};
    vec![
        FormatRule {
            format: VideoFormat::I420,
            techs: [
                Some([
                    Some("Planar_Y"),
                    Some("Planar_U_Left"),
                    Some("Planar_V_Left"),
                ]),
                None,
                None,
            ],
            texel: Texel::Width,
            planes: [plane(1, 1, R8), plane(2, 2, R8), plane(2, 2, R8)],
        },
        FormatRule {
            format: VideoFormat::Nv12,
            techs: [Some([Some("NV12_Y"), Some("NV12_UV"), None]), None, None],
            texel: Texel::Width,
            planes: [plane(1, 1, R8), plane(2, 2, R8g8), NO_PLANE],
        },
        FormatRule {
            format: VideoFormat::I444,
            techs: [
                Some([Some("Planar_Y"), Some("Planar_U"), Some("Planar_V")]),
                None,
                None,
            ],
            texel: Texel::None,
            planes: [plane(1, 1, R8), plane(1, 1, R8), plane(1, 1, R8)],
        },
        FormatRule {
            format: VideoFormat::I010,
            techs: [
                Some([
                    Some("I010_SRGB_Y"),
                    Some("I010_SRGB_U"),
                    Some("I010_SRGB_V"),
                ]),
                Some([Some("I010_PQ_Y"), Some("I010_PQ_U"), Some("I010_PQ_V")]),
                Some([Some("I010_HLG_Y"), Some("I010_HLG_U"), Some("I010_HLG_V")]),
            ],
            texel: Texel::Both,
            planes: [plane(1, 1, R16), plane(2, 2, R16), plane(2, 2, R16)],
        },
        FormatRule {
            format: VideoFormat::P010,
            techs: [
                Some([Some("P010_SRGB_Y"), Some("P010_SRGB_UV"), None]),
                Some([Some("P010_PQ_Y"), Some("P010_PQ_UV"), None]),
                Some([Some("P010_HLG_Y"), Some("P010_HLG_UV"), None]),
            ],
            texel: Texel::Both,
            planes: [plane(1, 1, R16), plane(2, 2, Rg16), NO_PLANE],
        },
        FormatRule {
            format: VideoFormat::P216,
            techs: [
                Some([Some("P216_SRGB_Y"), Some("P216_SRGB_UV"), None]),
                Some([Some("P216_PQ_Y"), Some("P216_PQ_UV"), None]),
                Some([Some("P216_HLG_Y"), Some("P216_HLG_UV"), None]),
            ],
            texel: Texel::Both,
            planes: [plane(1, 1, R16), plane(2, 1, Rg16), NO_PLANE],
        },
        FormatRule {
            format: VideoFormat::P416,
            techs: [
                Some([Some("P416_SRGB_Y"), Some("P416_SRGB_UV"), None]),
                Some([Some("P416_PQ_Y"), Some("P416_PQ_UV"), None]),
                Some([Some("P416_HLG_Y"), Some("P416_HLG_UV"), None]),
            ],
            texel: Texel::Both,
            planes: [plane(1, 1, R16), plane(1, 1, Rg16), NO_PLANE],
        },
    ]
}

fn is_hdr(cs: Option<VideoColorspace>) -> bool {
    matches!(
        cs,
        Some(VideoColorspace::Cs2100Pq | VideoColorspace::Cs2100Hlg)
    )
}

fn high_precision(format: Option<VideoFormat>) -> bool {
    use VideoFormat as F;
    matches!(
        format,
        Some(F::I010 | F::P010 | F::I210 | F::I412 | F::Ya2l | F::P216 | F::P416)
    )
}

/// The technique names, as `&str`, for comparison.
fn techs_of(plan: &OutputPlan) -> Techs {
    plan.conversion_techs
        .map(|t| t.map(|t| t.to_str().expect("technique names are ASCII")))
}

/// What the plan is, written out per format (`expected_plan` of the C test).
/// The techniques come back as `&str`, compared through [`techs_of`].
fn expected_plan(
    format: Option<VideoFormat>,
    cs: Option<VideoColorspace>,
    width: u32,
    height: u32,
) -> (OutputPlan, Techs) {
    let mut plan = OutputPlan::ZERO;
    let mut techs: Techs = [None; 3];

    let rules = rules();
    if let Some(rule) = rules.iter().find(|r| Some(r.format) == format) {
        let mut set = SDR;
        if rule.techs[PQ].is_some() && cs == Some(VideoColorspace::Cs2100Pq) {
            set = PQ;
        } else if rule.techs[HLG].is_some() && cs == Some(VideoColorspace::Cs2100Hlg) {
            set = HLG;
        }

        plan.conversion_needed = true;
        techs = rule.techs[set].expect("the set exists");
        for c in 0..3 {
            let p = rule.planes[c];
            // wdiv == 0 is no plane
            if let (Some(w), Some(h)) = (width.checked_div(p.wdiv), height.checked_div(p.hdiv)) {
                plan.planes[c] = VideoPlane {
                    width: w,
                    height: h,
                    format: p.format,
                };
            }
        }
        if rule.texel != Texel::None {
            plan.conversion_width_i = 1.0 / width as f32;
        }
        if rule.texel == Texel::Both {
            plan.conversion_height_i = 1.0 / height as f32;
        }
    }

    plan.texture_format = if high_precision(format) {
        GsColorFormat::Rgba16f
    } else {
        GsColorFormat::Bgra
    };
    use VideoFormat as F;
    plan.render_space = if is_hdr(cs) {
        GsColorSpace::Cs709Extended
    } else if matches!(format, Some(F::I010 | F::P010 | F::P216 | F::P416)) {
        GsColorSpace::Srgb16f
    } else {
        GsColorSpace::Srgb
    };
    (plan, techs)
}

fn same_float(a: f32, b: f32) -> bool {
    a.to_bits() == b.to_bits()
}

fn check_plan(format: Option<VideoFormat>, cs: Option<VideoColorspace>, width: u32, height: u32) {
    let (want, want_techs) = expected_plan(format, cs, width, height);
    let got = output_plan(format, cs, width, height);
    let what = format!("format {format:?}, colorspace {cs:?}, {width}x{height}");

    assert_eq!(got.conversion_needed, want.conversion_needed, "{what}");
    assert!(
        same_float(got.conversion_width_i, want.conversion_width_i),
        "{what}: width_i {} is not {}",
        got.conversion_width_i,
        want.conversion_width_i
    );
    assert!(
        same_float(got.conversion_height_i, want.conversion_height_i),
        "{what}: height_i {} is not {}",
        got.conversion_height_i,
        want.conversion_height_i
    );
    assert_eq!(got.texture_format, want.texture_format, "{what}");
    assert_eq!(got.render_space, want.render_space, "{what}");
    assert_eq!(techs_of(&got), want_techs, "{what}");
    assert_eq!(got.planes, want.planes, "{what}");
}

fn tech(plan: &OutputPlan, c: usize) -> Option<&'static str> {
    techs_of(plan)[c]
}

// ---------------------------------------------------------------------------
// the plan, for every combination

#[test]
fn every_format_colorspace_and_size() {
    let mut checked = 0usize;
    let formats = all_formats();
    let colorspaces = all_colorspaces();
    for &f in &formats {
        for &c in &colorspaces {
            for &w in &SIZES {
                for &h in &SIZES {
                    check_plan(f, c, w, h);
                    checked += 1;
                }
            }
        }
    }
    assert_eq!(
        checked,
        formats.len() * colorspaces.len() * SIZES.len() * SIZES.len()
    );
    // 26 formats and 6 color spaces, each with one past the end.
    assert_eq!(formats.len(), 27);
    assert_eq!(colorspaces.len(), 7);
}

/// The same plan, written out for the common sizes, so a reader sees the
/// numbers.
#[test]
fn common_sizes_written_out() {
    let p = output_plan(
        Some(VideoFormat::Nv12),
        Some(VideoColorspace::Cs709),
        1920,
        1080,
    );
    assert!(p.conversion_needed);
    assert_eq!(tech(&p, 0), Some("NV12_Y"));
    assert_eq!(tech(&p, 1), Some("NV12_UV"));
    assert_eq!(tech(&p, 2), None);
    assert!(same_float(p.conversion_width_i, 1.0 / 1920.0));
    assert!(same_float(p.conversion_height_i, 0.0));
    assert_eq!(p.planes[0].width, 1920);
    assert_eq!(p.planes[0].height, 1080);
    assert_eq!(p.planes[0].format, GsColorFormat::R8);
    assert_eq!(p.planes[1].width, 960);
    assert_eq!(p.planes[1].height, 540);
    assert_eq!(p.planes[1].format, GsColorFormat::R8g8);
    assert_eq!(p.planes[2].width, 0);
    assert_eq!(p.planes[2].height, 0);
    assert_eq!(p.planes[2].format, GsColorFormat::Unknown);
    assert_eq!(p.texture_format, GsColorFormat::Bgra);
    assert_eq!(p.render_space, GsColorSpace::Srgb);

    let p = output_plan(
        Some(VideoFormat::I420),
        Some(VideoColorspace::Cs601),
        1280,
        720,
    );
    assert_eq!(tech(&p, 0), Some("Planar_Y"));
    assert_eq!(tech(&p, 1), Some("Planar_U_Left"));
    assert_eq!(tech(&p, 2), Some("Planar_V_Left"));
    assert!(same_float(p.conversion_width_i, 1.0 / 1280.0));
    assert_eq!(p.planes[1].width, 640);
    assert_eq!(p.planes[1].height, 360);
    assert_eq!(p.planes[2].width, 640);
    assert_eq!(p.planes[2].height, 360);
    assert_eq!(p.planes[2].format, GsColorFormat::R8);

    let p = output_plan(
        Some(VideoFormat::I444),
        Some(VideoColorspace::Srgb),
        1280,
        720,
    );
    assert_eq!(tech(&p, 1), Some("Planar_U"));
    assert_eq!(tech(&p, 2), Some("Planar_V"));
    assert!(same_float(p.conversion_width_i, 0.0));
    assert_eq!(p.planes[2].width, 1280);
    assert_eq!(p.planes[2].height, 720);

    let p = output_plan(
        Some(VideoFormat::P010),
        Some(VideoColorspace::Cs2100Pq),
        3840,
        2160,
    );
    assert_eq!(tech(&p, 0), Some("P010_PQ_Y"));
    assert_eq!(tech(&p, 1), Some("P010_PQ_UV"));
    assert_eq!(tech(&p, 2), None);
    assert!(same_float(p.conversion_width_i, 1.0 / 3840.0));
    assert!(same_float(p.conversion_height_i, 1.0 / 2160.0));
    assert_eq!(p.planes[0].format, GsColorFormat::R16);
    assert_eq!(p.planes[1].width, 1920);
    assert_eq!(p.planes[1].height, 1080);
    assert_eq!(p.planes[1].format, GsColorFormat::Rg16);
    assert_eq!(p.texture_format, GsColorFormat::Rgba16f);
    assert_eq!(p.render_space, GsColorSpace::Cs709Extended);

    let p = output_plan(
        Some(VideoFormat::I010),
        Some(VideoColorspace::Cs2100Hlg),
        1920,
        1080,
    );
    assert_eq!(tech(&p, 2), Some("I010_HLG_V"));
    assert_eq!(p.planes[2].width, 960);
    assert_eq!(p.planes[2].height, 540);
    assert_eq!(p.planes[2].format, GsColorFormat::R16);

    let p = output_plan(
        Some(VideoFormat::P216),
        Some(VideoColorspace::Cs709),
        1920,
        1080,
    );
    assert_eq!(tech(&p, 0), Some("P216_SRGB_Y"));
    assert_eq!(p.planes[1].width, 960);
    assert_eq!(p.planes[1].height, 1080);
    assert_eq!(p.render_space, GsColorSpace::Srgb16f);

    let p = output_plan(
        Some(VideoFormat::P416),
        Some(VideoColorspace::Default),
        1920,
        1080,
    );
    assert_eq!(tech(&p, 1), Some("P416_SRGB_UV"));
    assert_eq!(p.planes[1].width, 1920);
    assert_eq!(p.planes[1].height, 1080);
}

/// Odd sizes round the chroma planes down. `obs_reset_video()` aligns the
/// main output to a width that is a multiple of 4 and an even height, but a
/// canvas's mix is not aligned, so its plan can see any size.
#[test]
fn odd_sizes_round_chroma_down() {
    let p = output_plan(Some(VideoFormat::I420), Some(VideoColorspace::Cs709), 7, 5);
    assert_eq!(p.planes[0].width, 7);
    assert_eq!(p.planes[0].height, 5);
    assert_eq!(p.planes[1].width, 3);
    assert_eq!(p.planes[1].height, 2);
    assert!(same_float(p.conversion_width_i, 1.0 / 7.0));

    let p = output_plan(Some(VideoFormat::P216), Some(VideoColorspace::Cs709), 7, 5);
    assert_eq!(p.planes[1].width, 3);
    assert_eq!(p.planes[1].height, 5);
    assert!(same_float(p.conversion_height_i, 1.0 / 5.0));

    // a 1x1 output gets empty chroma planes
    let p = output_plan(Some(VideoFormat::Nv12), Some(VideoColorspace::Cs709), 1, 1);
    assert_eq!(p.planes[1].width, 0);
    assert_eq!(p.planes[1].height, 0);
    assert_eq!(p.planes[1].format, GsColorFormat::R8g8);
}

/// Formats without a GPU conversion: no techniques and no planes, only the
/// render texture format and space.
#[test]
fn formats_without_conversion() {
    use VideoFormat as F;
    let none = [
        F::None,
        F::Yvyu,
        F::Yuy2,
        F::Uyvy,
        F::Rgba,
        F::Bgra,
        F::Bgrx,
        F::Y800,
        F::Bgr3,
        F::I422,
        F::I40a,
        F::I42a,
        F::Yuva,
        F::Ayuv,
        F::I210,
        F::I412,
        F::Ya2l,
        F::V210,
        F::R10l,
    ];
    for f in none {
        let p = output_plan(Some(f), Some(VideoColorspace::Cs709), 64, 64);
        assert!(!p.conversion_needed, "{f:?}");
        for c in 0..3 {
            assert_eq!(tech(&p, c), None, "{f:?}");
            assert_eq!(p.planes[c].format, GsColorFormat::Unknown, "{f:?}");
            assert_eq!(p.planes[c].width, 0, "{f:?}");
        }
        assert!(same_float(p.conversion_width_i, 0.0), "{f:?}");
        assert!(same_float(p.conversion_height_i, 0.0), "{f:?}");
    }

    // I210, I412 and YA2L render in 16-bit float, but in SDR space
    let p = output_plan(Some(F::I210), Some(VideoColorspace::Cs709), 64, 64);
    assert_eq!(p.texture_format, GsColorFormat::Rgba16f);
    assert_eq!(p.render_space, GsColorSpace::Srgb);
    let p = output_plan(Some(F::Ya2l), Some(VideoColorspace::Cs2100Pq), 64, 64);
    assert_eq!(p.texture_format, GsColorFormat::Rgba16f);
    assert_eq!(p.render_space, GsColorSpace::Cs709Extended);
    // an 8-bit format with an HDR color space still renders in HDR space
    let p = output_plan(Some(F::Bgra), Some(VideoColorspace::Cs2100Hlg), 64, 64);
    assert_eq!(p.texture_format, GsColorFormat::Bgra);
    assert_eq!(p.render_space, GsColorSpace::Cs709Extended);
    // 8-bit formats ignore PQ and HLG for their techniques
    let p = output_plan(Some(F::Nv12), Some(VideoColorspace::Cs2100Pq), 64, 64);
    assert_eq!(tech(&p, 0), Some("NV12_Y"));
    assert_eq!(p.render_space, GsColorSpace::Cs709Extended);
}

/// `libobs/data/format_conversion.effect`, read from the source tree (the
/// test binary's manifest directory is under `rust/`), with CR removed: a
/// Windows checkout has CRLF line endings.
fn format_conversion_effect() -> String {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../libobs/data/format_conversion.effect"
    );
    std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("read {path}: {e}"))
        .replace('\r', "")
}

/// Every technique the plan names is a technique of
/// `format_conversion.effect`, which `obs-video.c` loads them from by name.
#[test]
fn techniques_exist_in_the_effect() {
    let text = format_conversion_effect();
    let mut named = 0usize;
    for f in all_formats() {
        for c in all_colorspaces() {
            let p = output_plan(f, c, 64, 64);
            for t in techs_of(&p).into_iter().flatten() {
                let needle = format!("\ntechnique {t}\n");
                assert!(
                    text.contains(&needle),
                    "technique {t} is not in format_conversion.effect"
                );
                named += 1;
            }
        }
    }
    assert!(named > 0);
}

// ---------------------------------------------------------------------------
// the size check and the alignment of obs_reset_video()

#[test]
fn size_check_edges() {
    let cases: [(u32, bool); 11] = [
        (0, false),
        (1, false),
        (2, true),
        (3, true),
        (4, true),
        (1366, true),
        (32767, true),
        (32768, true),
        (32769, false),
        (65536, false),
        (u32::MAX, false),
    ];
    for (w, wv) in cases {
        for (h, hv) in cases {
            assert_eq!(size_valid(w, h), wv && hv, "size_valid({w}, {h})");
        }
    }
}

#[test]
fn alignment_masks() {
    let cases: [(u32, u32, u32); 15] = [
        (0, 0, 0),
        (1, 0, 0),
        (4, 4, 4),
        (5, 4, 4),
        (6, 4, 6),
        (7, 4, 6),
        (8, 8, 8),
        (9, 8, 8),
        (767, 764, 766),
        (1080, 1080, 1080),
        (1366, 1364, 1366),
        (1367, 1364, 1366),
        (32767, 32764, 32766),
        (32768, 32768, 32768),
        (u32::MAX, 0xFFFF_FFFC, 0xFFFF_FFFE),
    ];
    for (input, width, height) in cases {
        assert_eq!(align_output_size(input, input), (width, height), "{input}");
    }

    // the two are independent
    assert_eq!(align_output_size(1366, 767), (1364, 766));
}

/// A width of 2 or 3 passes the size check, and the alignment makes it 0,
/// so the output size check refuses it. The height minimum, 2, survives the
/// alignment, and 3 becomes 2, so those heights stay valid.
#[test]
fn width_2_and_3_refused_after_alignment() {
    for w in 2..=3 {
        assert!(size_valid(w, 2));
        assert_eq!(align_output_size(w, 2), (0, 2));
        for h in 2..=3 {
            assert!(!output_size_valid(w, h), "{w}x{h}");
        }
    }
    assert_eq!(align_output_size(4, 3), (4, 2));
    assert!(output_size_valid(4, 2));
    assert!(output_size_valid(4, 3));
    assert!(output_size_valid(5, 3));
}

/// Every other size is accepted or refused as by `size_valid`: only the
/// widths 2 and 3 change. The alignment only lowers a size, so a size above
/// the maximum stays refused.
#[test]
fn output_size_check_edges() {
    let sizes: [u32; 17] = [
        0,
        1,
        2,
        3,
        4,
        5,
        6,
        7,
        8,
        1366,
        32767,
        32768,
        32769,
        32771,
        32772,
        65536,
        u32::MAX,
    ];
    for w in sizes {
        for h in sizes {
            let want = size_valid(w, h) && w != 2 && w != 3;
            assert_eq!(output_size_valid(w, h), want, "output_size_valid({w}, {h})");
        }
    }
}

// ---------------------------------------------------------------------------

#[test]
fn scale_type_names() {
    let name = |t| scale_type_name(t).to_str().expect("ASCII");
    assert_eq!(name(Some(ObsScaleType::Disable)), "Disabled");
    assert_eq!(name(Some(ObsScaleType::Point)), "Point");
    assert_eq!(name(Some(ObsScaleType::Bicubic)), "Bicubic");
    assert_eq!(name(Some(ObsScaleType::Bilinear)), "Bilinear");
    assert_eq!(name(Some(ObsScaleType::Lanczos)), "Lanczos");
    assert_eq!(name(Some(ObsScaleType::Area)), "Area");
    // any other value logs an empty name
    assert_eq!(
        name(ObsScaleType::from_c(ObsScaleType::Area.to_c() + 1)),
        ""
    );
    assert_eq!(name(ObsScaleType::from_c(-1)), "");
}

// ---------------------------------------------------------------------------
// edges of the safe API

/// The enum tables are in header order, with the header's values.
#[test]
fn enum_tables_in_header_order() {
    for (i, f) in GsColorFormat::ALL.into_iter().enumerate() {
        assert_eq!(f.to_c(), i as i32, "{f:?}");
    }
    assert_eq!(GsColorFormat::Rg16.to_c(), 22);
    for (i, s) in GsColorSpace::ALL.into_iter().enumerate() {
        assert_eq!(s.to_c(), i as i32, "{s:?}");
    }
    for (i, t) in ObsScaleType::ALL.into_iter().enumerate() {
        assert_eq!(t.to_c(), i as i32, "{t:?}");
        assert_eq!(ObsScaleType::from_c(i as i32), Some(t));
    }
    assert_eq!(ObsScaleType::from_c(6), None);
    assert_eq!(ObsScaleType::from_c(i32::MIN), None);
    assert_eq!(ObsScaleType::from_c(i32::MAX), None);
}

/// The bounds are the C's `OBS_SIZE_MIN` and `OBS_SIZE_MAX`.
#[test]
fn size_bounds() {
    assert_eq!(SIZE_MIN, 2);
    assert_eq!(SIZE_MAX, 32768);
    assert_eq!(NUM_CHANNELS, 3);
    assert!(size_valid(SIZE_MIN, SIZE_MAX));
    assert!(!size_valid(SIZE_MIN - 1, SIZE_MAX));
    assert!(!size_valid(SIZE_MIN, SIZE_MAX + 1));
}

/// A zero size, which only a canvas's unchecked mix can ask for, gives an
/// infinite texel size and empty planes, as `1.f / (float)0` does in C;
/// nothing panics.
#[test]
fn zero_size_gives_infinite_texels() {
    let p = output_plan(Some(VideoFormat::P010), Some(VideoColorspace::Cs709), 0, 0);
    assert_eq!(p.conversion_width_i, f32::INFINITY);
    assert_eq!(p.conversion_height_i, f32::INFINITY);
    assert_eq!(
        p.planes[0],
        VideoPlane::NONE.with_format(GsColorFormat::R16)
    );
    assert_eq!(p.planes[1].width, 0);
}

/// Sizes past `i32::MAX` keep their unsigned meaning; `u32::MAX` rounds to
/// 2^32 as an `f32`, as the C conversion does.
#[test]
fn largest_sizes() {
    let p = output_plan(
        Some(VideoFormat::I010),
        Some(VideoColorspace::Cs709),
        u32::MAX,
        u32::MAX,
    );
    assert_eq!(p.planes[0].width, u32::MAX);
    assert_eq!(p.planes[1].width, u32::MAX / 2);
    assert!(same_float(p.conversion_width_i, 1.0 / 4_294_967_296.0));
}

/// The names are NUL-terminated and contain only the C's characters.
#[test]
fn technique_names_are_c_strings() {
    for f in all_formats() {
        for c in all_colorspaces() {
            let p = output_plan(f, c, 64, 64);
            for t in p.conversion_techs.into_iter().flatten() {
                let t: &CStr = t;
                assert!(
                    t.to_bytes()
                        .iter()
                        .all(|b| b.is_ascii_alphanumeric() || *b == b'_'),
                    "{t:?}"
                );
            }
        }
    }
}

/// A plane with no technique is no plane, and a technique always has a
/// plane.
#[test]
fn techniques_and_planes_pair_up() {
    for f in all_formats() {
        for c in all_colorspaces() {
            let p = output_plan(f, c, 64, 64);
            for i in 0..NUM_CHANNELS {
                assert_eq!(
                    p.conversion_techs[i].is_some(),
                    p.planes[i].format != GsColorFormat::Unknown,
                    "{f:?} {c:?} plane {i}"
                );
            }
        }
    }
}

trait WithFormat {
    fn with_format(self, format: GsColorFormat) -> Self;
}

impl WithFormat for VideoPlane {
    fn with_format(mut self, format: GsColorFormat) -> Self {
        self.format = format;
        self
    }
}

fn any_format() -> impl Strategy<Value = Option<VideoFormat>> {
    prop::sample::select(all_formats())
}

fn any_colorspace() -> impl Strategy<Value = Option<VideoColorspace>> {
    prop::sample::select(all_colorspaces())
}

proptest! {
    /// Any format, color space and `u32` sizes: the plan is the written-out
    /// expectation, and nothing panics.
    #[test]
    fn plan_matches_the_rules_for_any_size(
        f in any_format(),
        c in any_colorspace(),
        w in any::<u32>(),
        h in any::<u32>(),
    ) {
        check_plan(f, c, w, h);
    }

    /// The output size check is the size check without widths 2 and 3, for
    /// any sizes; the alignment only lowers a size, to the multiple below.
    #[test]
    fn output_size_valid_for_any_size(w in any::<u32>(), h in any::<u32>()) {
        let (aw, ah) = align_output_size(w, h);
        prop_assert!(aw <= w && ah <= h);
        prop_assert!(w - aw < 4 && h - ah < 2);
        prop_assert_eq!(aw % 4, 0);
        prop_assert_eq!(ah % 2, 0);
        prop_assert_eq!(align_output_size(aw, ah), (aw, ah));
        prop_assert_eq!(
            output_size_valid(w, h),
            size_valid(w, h) && w != 2 && w != 3
        );
    }
}
