//! Tier 3: `libobs/media-io/audio-resampler-plan.c.h` against its port.
//!
//! The oracle (`rust/obs-c-oracle/oracle/audio_resampler_plan.c`) is the
//! unchanged C file with its six functions renamed `oracle_*`. Each case
//! runs the oracle, the C ABI shims (`ffi::audio_resampler_drift`, what the
//! hybrid libobs links) and the safe core on the same inputs and compares
//! everything they return:
//!
//! - the channel masks;
//! - the mask, the output channels, the input channels and the 4.1 test of
//!   every speaker layout from -300 to 300 and the ends of `int`, and, by
//!   proptest, of any `int`;
//! - whether the output is planar for every format from -300 to 300 and the
//!   ends of `int`, and, by proptest, any `int`;
//! - the upmix gate over 0 to 300 channels and the ends of `uint32_t`, and, by
//!   proptest, any channel count.
//!
//! No intentional difference, and no input left out: every `int` is a
//! defined input to all six functions.
//!
//! Reminder: the mutation check (break the core, see this file fail) is done
//! once per port, not on every change.

use core::ffi::c_int;

use obs_c_oracle::audio_resampler_plan as c;
use obs_media_io::audio_resampler_drift::{self as core_, ch_layout};
use obs_media_io::ffi::audio_resampler_drift as shim;
use proptest::prelude::*;

fn oracle_int(which: usize) -> i64 {
    // SAFETY: plain value in, plain value out.
    unsafe { c::oracle_audio_resampler_plan_int(which) }
}

/// Everything the C answers about the speaker layout `layout`: its mask,
/// its channels as the output and as the input, and whether it is 4.1.
type LayoutAnswers = (u64, u32, c_int, bool);

fn check_layout(layout: c_int) {
    // SAFETY: plain values in, plain values out.
    let want: LayoutAnswers = unsafe {
        (
            c::oracle_audio_resampler_layout_mask(layout),
            c::oracle_audio_resampler_output_channels(layout),
            c::oracle_audio_resampler_input_channels(layout),
            c::oracle_audio_resampler_is_4point1(layout),
        )
    };
    let shims: LayoutAnswers = (
        shim::audio_resampler_layout_mask(layout),
        shim::audio_resampler_output_channels(layout),
        shim::audio_resampler_input_channels(layout),
        shim::audio_resampler_is_4point1(layout),
    );
    let core: LayoutAnswers = (
        core_::layout_mask(layout),
        core_::output_channels(layout),
        core_::input_channels(layout),
        core_::is_4point1(layout),
    );
    assert_eq!(shims, want, "shim, layout {layout}");
    assert_eq!(core, want, "core, layout {layout}");
}

fn check_format(format: c_int) {
    // SAFETY: plain value in, plain value out.
    let want = unsafe { c::oracle_audio_resampler_planar_output(format) };
    assert_eq!(
        shim::audio_resampler_planar_output(format),
        want,
        "shim, format {format}"
    );
    assert_eq!(core_::planar_output(format), want, "core, format {format}");
}

fn check_gate(output_ch: u32) {
    // SAFETY: plain value in, plain value out.
    let want = unsafe { c::oracle_audio_resampler_upmixes_mono(output_ch) };
    assert_eq!(
        shim::audio_resampler_upmixes_mono(output_ch),
        want,
        "shim, {output_ch}"
    );
    assert_eq!(core_::upmixes_mono(output_ch), want, "core, {output_ch}");
}

/// Layouts and formats at the ends of `int`.
const INT_EDGES: [c_int; 4] = [c_int::MIN, c_int::MIN + 1, c_int::MAX - 1, c_int::MAX];

#[test]
fn constants_match() {
    let masks = [
        ch_layout::MONO,
        ch_layout::STEREO,
        ch_layout::SURROUND,
        ch_layout::FOUR_POINT_ZERO,
        ch_layout::FOUR_POINT_ONE,
        ch_layout::FIVE_POINT_ONE_BACK,
        ch_layout::SEVEN_POINT_ONE,
    ];
    for (i, mask) in masks.into_iter().enumerate() {
        assert_eq!(
            i64::try_from(mask).unwrap(),
            oracle_int(i),
            "{}",
            c::INT_NAMES[i]
        );
    }
    assert_eq!(oracle_int(c::INT_NAMES.len()), -1);
}

#[test]
fn layouts_match() {
    for layout in (-300..=300).chain(INT_EDGES) {
        check_layout(layout);
    }
}

#[test]
fn formats_match() {
    for format in (-300..=300).chain(INT_EDGES) {
        check_format(format);
    }
}

#[test]
fn upmix_gate_matches() {
    for ch in (0..=300).chain([0x7fff_ffff, 0x8000_0000, u32::MAX - 1, u32::MAX]) {
        check_gate(ch);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(2_000)))]

    #[test]
    fn layout_any(layout in any::<c_int>()) {
        check_layout(layout);
    }

    #[test]
    fn format_any(format in any::<c_int>()) {
        check_format(format);
    }

    #[test]
    fn upmix_gate_any(output_ch in any::<u32>()) {
        check_gate(output_ch);
    }
}
