//! Tier 1: the output plan of `libobs/media-io/audio-resampler-plan.c.h`.
//!
//! The `steps_*` tests port `test/cmocka/test_audio_resampler_plan_steps.c`
//! one to one: the same cases under the same names, in the same order, with
//! the same values, the characterized-not-endorsed ones included.
//!
//! The `edge_*` tests and the proptests check the core against [`model`], the
//! origin/master C of `audio_resampler_create()` and
//! `convert_speaker_layout()` written out expression by expression, over
//! every `int` a speaker layout or an audio format can be.

use core::ffi::c_int;

use obs_media_io::audio_resampler_drift::{
    ch_layout, input_channels, is_4point1, layout_mask, output_channels, planar_output,
    upmixes_mono,
};
use proptest::prelude::*;

const UNKNOWN: c_int = 0;
const MONO: c_int = 1;
const STEREO: c_int = 2;
const TWO_POINT_ONE: c_int = 3;
const FOUR_POINT_ZERO: c_int = 4;
const FOUR_POINT_ONE: c_int = 5;
const FIVE_POINT_ONE: c_int = 6;
const SEVEN_POINT_ONE: c_int = 8;

const FORMAT_UNKNOWN: c_int = 0;
const U8BIT: c_int = 1;
const BIT16: c_int = 2;
const BIT32: c_int = 3;
const FLOAT: c_int = 4;
const U8BIT_PLANAR: c_int = 5;
const BIT16_PLANAR: c_int = 6;
const BIT32_PLANAR: c_int = 7;
const FLOAT_PLANAR: c_int = 8;

/// Speaker layout and audio format values outside their enums. 7 is the gap
/// before `SPEAKERS_7POINT1`.
const ODD_LAYOUTS: [c_int; 6] = [-1, 7, 9, 100, c_int::MAX, c_int::MIN];
const ODD_FORMATS: [c_int; 5] = [-1, 9, 100, c_int::MAX, c_int::MIN];

/// `get_audio_channels()` of `audio-io.h`.
fn channels(layout: c_int) -> u32 {
    match layout {
        MONO => 1,
        STEREO => 2,
        TWO_POINT_ONE => 3,
        FOUR_POINT_ZERO => 4,
        FOUR_POINT_ONE => 5,
        FIVE_POINT_ONE => 6,
        SEVEN_POINT_ONE => 8,
        _ => 0,
    }
}

/// `is_audio_planar()` of `audio-io.h`.
fn planar(format: c_int) -> bool {
    matches!(
        format,
        U8BIT_PLANAR | BIT16_PLANAR | BIT32_PLANAR | FLOAT_PLANAR
    )
}

/// What `audio_resampler_create()` sets up from the core, asked as it asks
/// it: the planes from the output channels when the output is planar; the
/// input channels and the 4.1 layouts on the newer libswresample.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Plan {
    output_ch: u32,
    output_planes: u32,
    input_channels: c_int,
    input_4point1: bool,
    output_4point1: bool,
}

fn plan(src: c_int, dst: c_int, dst_format: c_int) -> Plan {
    let output_ch = output_channels(dst);
    Plan {
        output_ch,
        output_planes: if planar_output(dst_format) {
            output_ch
        } else {
            1
        },
        input_channels: input_channels(src),
        input_4point1: is_4point1(src),
        output_4point1: is_4point1(dst),
    }
}

/// The mono upmix as `audio_resampler_create()` asks it: the gate only once
/// the input is mono.
fn upmixed(input_mono: bool, output_ch: u32) -> bool {
    input_mono && upmixes_mono(output_ch)
}

/// The origin/master C, expression by expression.
mod model {
    use super::*;

    /// `convert_speaker_layout()`, with libavutil's masks.
    pub fn layout_mask(layout: c_int) -> u64 {
        match layout {
            MONO => 0x4,
            STEREO => 0x3,
            TWO_POINT_ONE => 0x7,
            FOUR_POINT_ZERO => 0x107,
            FOUR_POINT_ONE => 0x10f,
            FIVE_POINT_ONE => 0x3f,
            SEVEN_POINT_ONE => 0x63f,
            _ => 0,
        }
    }

    /// The plan `audio_resampler_create()` wrote inline.
    pub fn plan(src: c_int, dst: c_int, dst_format: c_int) -> Plan {
        let output_ch = channels(dst);
        Plan {
            output_ch,
            output_planes: if planar(dst_format) { output_ch } else { 1 },
            input_channels: c_int::try_from(channels(src)).unwrap(),
            input_4point1: src == FOUR_POINT_ONE,
            output_4point1: dst == FOUR_POINT_ONE,
        }
    }

    /// `input mono && rs->output_ch > 1`.
    pub fn upmixes_mono(input_mono: bool, output_ch: u32) -> bool {
        input_mono && output_ch > 1
    }
}

fn want(
    output_ch: u32,
    output_planes: u32,
    input_channels: c_int,
    input_4point1: bool,
    output_4point1: bool,
) -> Plan {
    Plan {
        output_ch,
        output_planes,
        input_channels,
        input_4point1,
        output_4point1,
    }
}

#[test]
fn steps_constants() {
    assert_eq!(ch_layout::MONO, 0x4);
    assert_eq!(ch_layout::STEREO, 0x3);
    assert_eq!(ch_layout::SURROUND, 0x7);
    assert_eq!(ch_layout::FOUR_POINT_ZERO, 0x107);
    assert_eq!(ch_layout::FOUR_POINT_ONE, 0x10f);
    assert_eq!(ch_layout::FIVE_POINT_ONE_BACK, 0x3f);
    assert_eq!(ch_layout::SEVEN_POINT_ONE, 0x63f);
}

#[test]
fn steps_layout_masks() {
    assert_eq!(layout_mask(UNKNOWN), 0);
    assert_eq!(layout_mask(MONO), ch_layout::MONO);
    assert_eq!(layout_mask(STEREO), ch_layout::STEREO);
    // SPEAKERS_2POINT1 is FL, FR and LFE, but its mask is surround: FL, FR
    // and FC (characterized, not endorsed)
    assert_eq!(layout_mask(TWO_POINT_ONE), ch_layout::SURROUND);
    assert_eq!(layout_mask(FOUR_POINT_ZERO), ch_layout::FOUR_POINT_ZERO);
    assert_eq!(layout_mask(FOUR_POINT_ONE), ch_layout::FOUR_POINT_ONE);
    assert_eq!(layout_mask(FIVE_POINT_ONE), ch_layout::FIVE_POINT_ONE_BACK);
    assert_eq!(layout_mask(SEVEN_POINT_ONE), ch_layout::SEVEN_POINT_ONE);

    for layout in ODD_LAYOUTS {
        assert_eq!(layout_mask(layout), 0, "layout {layout}");
    }
}

#[test]
fn steps_output_channels() {
    let cases = [
        (UNKNOWN, 0),
        (MONO, 1),
        (STEREO, 2),
        (TWO_POINT_ONE, 3),
        (FOUR_POINT_ZERO, 4),
        (FOUR_POINT_ONE, 5),
        (FIVE_POINT_ONE, 6),
        (SEVEN_POINT_ONE, 8),
    ];
    for (layout, ch) in cases {
        let p = plan(STEREO, layout, FLOAT_PLANAR);
        assert_eq!(p.output_ch, ch, "layout {layout}");
        assert_eq!(p.output_planes, ch, "layout {layout}");
    }
    for layout in ODD_LAYOUTS {
        let p = plan(STEREO, layout, FLOAT_PLANAR);
        assert_eq!(p.output_ch, 0, "layout {layout}");
        assert_eq!(p.output_planes, 0, "layout {layout}");
    }
}

#[test]
fn steps_output_planes() {
    let packed = [FORMAT_UNKNOWN, U8BIT, BIT16, BIT32, FLOAT];
    let planar_formats = [U8BIT_PLANAR, BIT16_PLANAR, BIT32_PLANAR, FLOAT_PLANAR];

    for dst in UNKNOWN..=SEVEN_POINT_ONE {
        let ch = channels(dst);
        let is_4point1 = dst == FOUR_POINT_ONE;

        for format in packed {
            assert_eq!(
                plan(STEREO, dst, format),
                want(ch, 1, 2, false, is_4point1),
                "dst {dst} format {format}"
            );
        }
        for format in planar_formats {
            assert_eq!(
                plan(STEREO, dst, format),
                want(ch, ch, 2, false, is_4point1),
                "dst {dst} format {format}"
            );
        }
        // a format outside the enum is packed
        for format in ODD_FORMATS {
            assert_eq!(
                plan(STEREO, dst, format),
                want(ch, 1, 2, false, is_4point1),
                "dst {dst} format {format}"
            );
        }
    }

    // packed with no output channels still has one plane (characterized, not
    // endorsed)
    assert_eq!(plan(STEREO, UNKNOWN, FLOAT), want(0, 1, 2, false, false));
}

#[test]
fn steps_input_channels() {
    let cases = [
        // the newer libswresample asks the default layout of 0 channels for
        // an unknown input (characterized, not endorsed)
        (UNKNOWN, 0),
        (MONO, 1),
        (STEREO, 2),
        (TWO_POINT_ONE, 3),
        (FOUR_POINT_ZERO, 4),
        (FOUR_POINT_ONE, 5),
        (FIVE_POINT_ONE, 6),
        (SEVEN_POINT_ONE, 8),
    ];
    for (layout, ch) in cases {
        assert_eq!(
            plan(layout, STEREO, FLOAT).input_channels,
            ch,
            "layout {layout}"
        );
    }
    for layout in ODD_LAYOUTS {
        assert_eq!(
            plan(layout, STEREO, FLOAT).input_channels,
            0,
            "layout {layout}"
        );
    }
}

#[test]
fn steps_four_point_one() {
    let p = plan(FOUR_POINT_ONE, STEREO, FLOAT);
    assert!(p.input_4point1);
    assert!(!p.output_4point1);

    let p = plan(STEREO, FOUR_POINT_ONE, FLOAT);
    assert!(!p.input_4point1);
    assert!(p.output_4point1);

    assert_eq!(
        plan(FOUR_POINT_ONE, FOUR_POINT_ONE, FLOAT_PLANAR),
        want(5, 5, 5, true, true)
    );

    // only 4.1: not 4.0, not five channels by another layout
    for layout in -1..=9 {
        if layout == FOUR_POINT_ONE {
            continue;
        }
        let p = plan(layout, layout, FLOAT);
        assert!(!p.input_4point1, "layout {layout}");
        assert!(!p.output_4point1, "layout {layout}");
    }
}

#[test]
fn steps_mono_into_each_layout() {
    // the input is mono on both libswresample paths: its mask is mono, and
    // the default layout of its one channel is mono
    assert_eq!(layout_mask(MONO), ch_layout::MONO);

    for dst in -1..=9 {
        let p = plan(MONO, dst, FLOAT_PLANAR);
        let ch = channels(dst);

        assert_eq!(
            p,
            want(ch, ch, 1, false, dst == FOUR_POINT_ONE),
            "dst {dst}"
        );
        // upmixed to two channels or more; not to mono, unknown or a layout
        // outside the enum
        assert_eq!(upmixed(true, p.output_ch), ch > 1, "dst {dst}");
    }
}

#[test]
fn steps_upmix_gate() {
    for ch in [0, 1, 2, 3, 5, 8, 9, 64, u32::MAX] {
        // asked only for a mono input
        assert_eq!(upmixes_mono(ch), ch > 1, "{ch} channels");
    }
}

#[test]
fn steps_every_input() {
    for src in -2..=10 {
        for dst in -2..=10 {
            for format in -2..=10 {
                let output_ch = channels(dst);
                let output_planes = if planar(format) { output_ch } else { 1 };
                let input_ch = c_int::try_from(channels(src)).unwrap();

                assert_eq!(
                    plan(src, dst, format),
                    want(
                        output_ch,
                        output_planes,
                        input_ch,
                        src == FOUR_POINT_ONE,
                        dst == FOUR_POINT_ONE
                    ),
                    "src {src} dst {dst} format {format}"
                );
            }
        }
    }
}

/// The layouts, formats and channel counts at the ends of their types and
/// around the enums.
const EDGES: [c_int; 16] = [
    c_int::MIN,
    c_int::MIN + 1,
    -256,
    -9,
    -2,
    -1,
    0,
    5,
    7,
    9,
    10,
    16,
    255,
    256,
    c_int::MAX - 1,
    c_int::MAX,
];

#[test]
fn edge_layout_masks() {
    for layout in EDGES.into_iter().chain(-1..=10) {
        assert_eq!(
            layout_mask(layout),
            model::layout_mask(layout),
            "layout {layout}"
        );
    }
}

#[test]
fn edge_plans() {
    let values: Vec<c_int> = EDGES.into_iter().chain(-1..=10).collect();
    for &src in &values {
        for &dst in &values {
            for &format in &values {
                assert_eq!(
                    plan(src, dst, format),
                    model::plan(src, dst, format),
                    "src {src} dst {dst} format {format}"
                );
            }
        }
    }
}

#[test]
fn edge_upmix_gate() {
    for ch in [
        0,
        1,
        2,
        7,
        8,
        9,
        0x7fff_ffff,
        0x8000_0000,
        u32::MAX - 1,
        u32::MAX,
    ] {
        for mono in [false, true] {
            assert_eq!(
                upmixed(mono, ch),
                model::upmixes_mono(mono, ch),
                "{mono} {ch}"
            );
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(256)))]

    #[test]
    fn prop_layout_mask_any(layout in any::<c_int>()) {
        prop_assert_eq!(layout_mask(layout), model::layout_mask(layout));
    }

    #[test]
    fn prop_plan_any(
        src in prop_oneof![any::<c_int>(), -2..=10],
        dst in prop_oneof![any::<c_int>(), -2..=10],
        format in prop_oneof![any::<c_int>(), -2..=10],
    ) {
        prop_assert_eq!(plan(src, dst, format), model::plan(src, dst, format));
    }

    #[test]
    fn prop_upmix_gate_any(mono in any::<bool>(), ch in any::<u32>()) {
        prop_assert_eq!(upmixed(mono, ch), model::upmixes_mono(mono, ch));
    }
}
