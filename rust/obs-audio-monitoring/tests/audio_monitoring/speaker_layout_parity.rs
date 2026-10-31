//! Tier 3 parity: the `speaker_layout` core and its shim against the
//! original `monitor-speaker-layout.c.h` (oracle/speaker_layout.c).
//!
//! The unchanged C tests `test_monitor_speaker_layout` and
//! `test_monitor_wasapi_steps` compile the C into themselves; their `_rust`
//! runs use the shim, and this file compares all three sides (oracle, shim,
//! core) exactly: the masks and the enum's size, every mask the monitor can
//! tell apart against a grid of channel counts, every mask of the low 16 bits
//! with 0 to 9 channels, a sweep of channel counts under the listed masks and
//! none, and a proptest over any mask and count.
//!
//! No intentional difference, and no input the C leaves undefined: the
//! function reads two integers and calls only `get_audio_channels()`.

use std::ffi::c_int;

use obs_audio_monitoring::ffi::speaker_layout as shim;
use obs_audio_monitoring::speaker_layout::{
    self as core, MASK_2POINT1, MASK_4POINT1, MASK_5POINT1, MASK_7POINT1, MASK_SURROUND,
};
use obs_c_oracle::speaker_layout::{
    VALUE_NAMES, oracle_audio_monitor_speaker_layout, oracle_monitor_speaker_layout_value,
};
use proptest::prelude::*;

const LISTED: [u32; 5] = [
    MASK_2POINT1,
    MASK_SURROUND,
    MASK_4POINT1,
    MASK_5POINT1,
    MASK_7POINT1,
];

fn oracle(mask: u32, channels: u32) -> c_int {
    // SAFETY: plain integers in and out.
    unsafe { oracle_audio_monitor_speaker_layout(mask, channels) }
}

/// The three sides agree; returns the layout.
fn check(mask: u32, channels: u32) -> c_int {
    let want = oracle(mask, channels);
    assert_eq!(
        shim::audio_monitor_speaker_layout(mask, channels),
        want,
        "shim: mask {mask:#x}, {channels} channels"
    );
    assert_eq!(
        core::speaker_layout(mask, channels),
        want,
        "core: mask {mask:#x}, {channels} channels"
    );
    want
}

#[test]
fn constants_match_the_c() {
    let rust: [i64; 6] = [
        i64::from(MASK_2POINT1),
        i64::from(MASK_SURROUND),
        i64::from(MASK_4POINT1),
        i64::from(MASK_5POINT1),
        i64::from(MASK_7POINT1),
        i64::try_from(size_of::<c_int>()).unwrap_or(-1),
    ];
    for (which, (name, want)) in VALUE_NAMES.iter().zip(rust).enumerate() {
        // SAFETY: an index; the oracle returns -1 past the end.
        let c = unsafe { oracle_monitor_speaker_layout_value(which) };
        assert_eq!(c, want, "{name}");
    }
    // SAFETY: as above.
    assert_eq!(
        unsafe { oracle_monitor_speaker_layout_value(VALUE_NAMES.len()) },
        -1
    );
    assert_eq!(core::UNKNOWN, 0);
}

/// Masks the monitor can tell apart: the listed ones, each with one bit
/// flipped, each single bit, none, every bit and the common unlisted ones.
fn masks() -> Vec<u32> {
    let mut masks = vec![0, u32::MAX, 0x3, 0x4, 0x33, 0x60F, 0x63F, 0x70F, 0x7FF];
    for listed in LISTED {
        masks.push(listed);
        masks.extend((0..32).map(|bit| listed ^ (1 << bit)));
    }
    masks.extend((0..32).map(|bit| 1u32 << bit));
    masks
}

/// Channel counts: 0 to 64, and +-3 around 255, 2^31 and `UINT32_MAX`.
fn counts() -> Vec<u32> {
    let mut counts: Vec<u32> = (0..=64).collect();
    for around in [255u32, 0x8000_0000, u32::MAX - 3] {
        counts.extend((0..=6).map(|d| around.wrapping_sub(3).wrapping_add(d)));
    }
    counts
}

#[test]
fn every_mask_against_a_grid_of_counts() {
    let counts = counts();
    let mut found = [0u32; 9];
    for mask in masks() {
        for &channels in &counts {
            let layout = check(mask, channels);
            if let Some(n) = usize::try_from(layout).ok().and_then(|i| found.get_mut(i)) {
                *n += 1;
            }
        }
    }
    // every layout comes up, and 7 never does
    assert_eq!(found[7], 0);
    for (layout, n) in found.iter().enumerate() {
        assert!(layout == 7 || *n > 0, "layout {layout} never returned");
    }
}

#[test]
fn every_low_mask_with_up_to_nine_channels() {
    for mask in obs_c_oracle::parity::sweep(0..1 << 16) {
        let mask = u32::try_from(mask).unwrap_or(u32::MAX);
        for channels in 0..10 {
            check(mask, channels);
        }
    }
}

#[test]
fn sweep_of_counts_under_listed_masks_and_none() {
    for channels in obs_c_oracle::parity::sweep(0..1 << 20) {
        let channels = u32::try_from(channels).unwrap_or(u32::MAX);
        check(0, channels);
        for mask in LISTED {
            check(mask, channels);
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(20_000)))]

    #[test]
    fn any_mask_and_count(
        mask in prop_oneof![any::<u32>(), proptest::sample::select(LISTED.to_vec())],
        channels in prop_oneof![0u32..12, any::<u32>()],
    ) {
        check(mask, channels);
    }
}
