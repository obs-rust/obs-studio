//! Tier 3: the safe cores of the `static inline` helpers in
//! `libobs/media-io/audio-io.h`, and the C-value helpers in
//! `obs_media_io::ffi::audio_io`, behave exactly like the header functions,
//! compiled as non-inline oracle wrappers.
//!
//! No intentional differences from C. The enum inputs are small, so besides
//! proptest over arbitrary `c_int`s the tests sweep every enumerator plus
//! values just outside each enum (7, the gap in `enum speaker_layout`,
//! included). The sweep covers every input of
//! `test/cmocka/test_audio_io_inlines.c`.
//!
//! `audio_frames_to_ns` and `ns_to_audio_frames` wrap `util_mul_div64`,
//! which picks its math per platform, and so does the port
//! (`obs_util::util_uint64::mul_div64`), so this runs on each CI platform
//! against that platform's C. Excluded inputs (C UB or fault, the port
//! panics instead):
//! - `audio_frames_to_ns` with `sample_rate == 0` (division by zero).
//! - On MSVC x64, results that do not fit in 64 bits (`_udiv128` faults).
//!   Off MSVC x64 they wrap, and are compared.
//!
//! Reminder: the mutation check (break the core, see this file fail) is done
//! once per port, not on every change.

use core::ffi::c_int;

use obs_c_oracle::audio_io_inlines as c;
use obs_media_io::audio_io::{
    self, AudioFormat, SpeakerLayout, audio_format_to_c, speaker_layout_to_c,
};
use obs_media_io::ffi::audio_io as rs;
use proptest::prelude::*;

/// `-2..=last + 2`, and the extremes.
fn sweep(last: c_int) -> impl Iterator<Item = c_int> {
    (-2..=last + 2).chain([c_int::MIN, c_int::MAX, 1000])
}

fn formats() -> impl Iterator<Item = c_int> {
    sweep(audio_format_to_c(AudioFormat::FloatPlanar))
}

/// Includes 7, which `enum speaker_layout` skips.
fn layouts() -> impl Iterator<Item = c_int> {
    sweep(speaker_layout_to_c(SpeakerLayout::SevenPointOne))
}

/// Frame counts around the `u32` edges and the sizes the C uses.
const FRAMES: [u32; 9] = [0, 1, 2, 480, 1024, 48000, 1 << 31, u32::MAX - 1, u32::MAX];

fn check_format(format: c_int) {
    // SAFETY: the oracle wrappers take any int and only compute.
    unsafe {
        assert_eq!(
            rs::get_audio_bytes_per_channel(format),
            c::oracle_get_audio_bytes_per_channel(format),
            "get_audio_bytes_per_channel({format})"
        );
        assert_eq!(
            rs::is_audio_planar(format),
            c::oracle_is_audio_planar(format),
            "is_audio_planar({format})"
        );
    }
}

fn check_layout(speakers: c_int) {
    // SAFETY: as in check_format.
    unsafe {
        assert_eq!(
            rs::get_audio_channels(speakers),
            c::oracle_get_audio_channels(speakers),
            "get_audio_channels({speakers})"
        );
    }
}

fn check_sizes(format: c_int, speakers: c_int, frames: u32) {
    // SAFETY: as in check_format.
    unsafe {
        assert_eq!(
            rs::get_audio_planes(format, speakers),
            c::oracle_get_audio_planes(format, speakers),
            "get_audio_planes({format}, {speakers})"
        );
        assert_eq!(
            rs::get_audio_size(format, speakers, frames),
            c::oracle_get_audio_size(format, speakers, frames),
            "get_audio_size({format}, {speakers}, {frames})"
        );
        assert_eq!(
            rs::get_total_audio_size(format, speakers, frames),
            c::oracle_get_total_audio_size(format, speakers, frames),
            "get_total_audio_size({format}, {speakers}, {frames})"
        );
    }
}

#[test]
fn c_value_helpers_match_c_over_sweep() {
    for format in formats() {
        check_format(format);
        for speakers in layouts() {
            for frames in FRAMES {
                check_sizes(format, speakers, frames);
            }
        }
    }
    for speakers in layouts() {
        check_layout(speakers);
    }
}

#[test]
fn safe_core_matches_c_for_every_enumerator() {
    // SAFETY: as in check_format.
    unsafe {
        for format in AudioFormat::ALL {
            let f = audio_format_to_c(format);
            assert_eq!(
                audio_io::audio_bytes_per_channel(format),
                c::oracle_get_audio_bytes_per_channel(f),
                "{format:?}"
            );
            assert_eq!(
                audio_io::is_audio_planar(format),
                c::oracle_is_audio_planar(f),
                "{format:?}"
            );
            for speakers in SpeakerLayout::ALL {
                let s = speaker_layout_to_c(speakers);
                assert_eq!(
                    audio_io::audio_planes(format, speakers),
                    c::oracle_get_audio_planes(f, s),
                    "{format:?} {speakers:?}"
                );
                for frames in FRAMES {
                    assert_eq!(
                        audio_io::audio_size(format, speakers, frames),
                        c::oracle_get_audio_size(f, s, frames),
                        "{format:?} {speakers:?} {frames}"
                    );
                    assert_eq!(
                        audio_io::total_audio_size(format, speakers, frames),
                        c::oracle_get_total_audio_size(f, s, frames),
                        "{format:?} {speakers:?} {frames}"
                    );
                }
            }
        }
        for speakers in SpeakerLayout::ALL {
            assert_eq!(
                audio_io::audio_channels(speakers),
                c::oracle_get_audio_channels(speaker_layout_to_c(speakers)),
                "{speakers:?}"
            );
        }
    }
}

const EXACT: bool = cfg!(all(target_env = "msvc", target_arch = "x86_64"));

/// The `util_mul_div64` inputs the C cannot run: see the module docs.
fn excluded(num: u64, mul: u64, div: u64) -> bool {
    div == 0
        || (EXACT && u128::from(num) * u128::from(mul) / u128::from(div) > u128::from(u64::MAX))
}

const NS_PER_SEC: u64 = 1_000_000_000;

fn check_frames_to_ns(sample_rate: usize, frames: u64) {
    let rate = u64::try_from(sample_rate).unwrap();
    if excluded(frames, NS_PER_SEC, rate) {
        return;
    }
    // SAFETY: plain integer arguments; the inputs the C cannot run are
    // excluded above.
    let theirs = unsafe { c::oracle_audio_frames_to_ns(sample_rate, frames) };
    assert_eq!(
        audio_io::audio_frames_to_ns(sample_rate, frames),
        theirs,
        "audio_frames_to_ns({sample_rate}, {frames:#x})"
    );
}

fn check_ns_to_frames(sample_rate: usize, ns: u64) {
    let rate = u64::try_from(sample_rate).unwrap();
    if excluded(ns, rate, NS_PER_SEC) {
        return;
    }
    // SAFETY: as in check_frames_to_ns.
    let theirs = unsafe { c::oracle_ns_to_audio_frames(sample_rate, ns) };
    assert_eq!(
        audio_io::ns_to_audio_frames(sample_rate, ns),
        theirs,
        "ns_to_audio_frames({sample_rate}, {ns:#x})"
    );
}

/// Sample rates OBS uses, and the edges.
const RATES: [usize; 14] = [
    0,
    1,
    2,
    8000,
    11025,
    22050,
    44100,
    48000,
    96000,
    192000,
    1_000_000_000,
    1_000_000_001,
    u32::MAX as usize,
    usize::MAX,
];

/// Values around the 32- and 64-bit edges, where the products overflow,
/// and the cmocka test's large inputs.
const EDGES: [u64; 15] = [
    0,
    1,
    1023,
    1024,
    48000,
    NS_PER_SEC,
    885_443_715_504_000,
    (1 << 32) - 1,
    1 << 32,
    (1 << 63) - 1,
    1 << 63,
    18_446_744_073_000_000_000,
    u64::MAX - 1,
    u64::MAX,
    u64::MAX / NS_PER_SEC * 48000 + 47999,
];

#[test]
fn conversions_match_c_at_edges() {
    for rate in RATES {
        for value in EDGES {
            check_frames_to_ns(rate, value);
            check_ns_to_frames(rate, value);
        }
    }
}

fn rate() -> impl Strategy<Value = usize> {
    prop_oneof![
        prop::sample::select(&RATES[..]),
        any::<u32>().prop_map(|r| r as usize),
        any::<usize>(),
    ]
}

fn edgy() -> impl Strategy<Value = u64> {
    prop_oneof![
        prop::sample::select(&EDGES[..]),
        any::<u64>(),
        any::<u32>().prop_map(u64::from),
        (0u32..64).prop_flat_map(|bits| any::<u64>().prop_map(move |v| v >> bits)),
    ]
}

proptest! {
    #[test]
    fn format_helpers_match_c(format in any::<c_int>()) {
        check_format(format);
    }

    #[test]
    fn channels_match_c(speakers in any::<c_int>()) {
        check_layout(speakers);
    }

    #[test]
    fn sizes_match_c(format in any::<c_int>(), speakers in any::<c_int>(), frames in any::<u32>()) {
        check_sizes(format, speakers, frames);
    }

    #[test]
    fn sizes_match_c_over_enumerators(
        format in -1..=9,
        speakers in -1..=9,
        frames in any::<u32>(),
    ) {
        check_sizes(format, speakers, frames);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(20_000)))]

    #[test]
    fn frames_to_ns_matches_c(sample_rate in rate(), frames in edgy()) {
        check_frames_to_ns(sample_rate, frames);
    }

    #[test]
    fn ns_to_frames_matches_c(sample_rate in rate(), ns in edgy()) {
        check_ns_to_frames(sample_rate, ns);
    }

    #[test]
    fn conversions_match_c_over_u64(sample_rate in any::<usize>(), value in any::<u64>()) {
        check_frames_to_ns(sample_rate, value);
        check_ns_to_frames(sample_rate, value);
    }
}
