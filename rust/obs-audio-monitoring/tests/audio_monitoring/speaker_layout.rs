//! Tier 1: `obs_audio_monitoring::speaker_layout`, the WASAPI monitor's
//! speaker layout for the device's channel mask and count.
//!
//! The `layout_*` cases port `test/cmocka/test_monitor_speaker_layout.c`
//! and the `steps_*` cases the speaker layout half of
//! `test/cmocka/test_monitor_wasapi_steps.c`, 1:1: the same cases under the
//! same names, in the same order, with the same values and expectations.
//! The C calls the hidden `audio_monitor_speaker_layout()`; here the safe
//! core is called directly.
//!
//! The `edge_*` cases are new: the core against a model of the origin/master
//! C (its switch and check inline) over every mask the monitor can tell
//! apart and a range of channel counts, and a proptest over full-range
//! inputs.

use std::ffi::c_int;

use obs_audio_monitoring::speaker_layout::{
    MASK_2POINT1, MASK_4POINT1, MASK_5POINT1, MASK_7POINT1, MASK_SURROUND, UNKNOWN, speaker_layout,
};
use proptest::prelude::*;

// `enum speaker_layout` values (`media-io/audio-io.h`).
const MONO: c_int = 1;
const STEREO: c_int = 2;
const TWO_POINT_ONE: c_int = 3;
const FOUR_POINT_ZERO: c_int = 4;
const FOUR_POINT_ONE: c_int = 5;
const FIVE_POINT_ONE: c_int = 6;
const SEVEN_POINT_ONE: c_int = 8;

// ksmedia.h's SPEAKER_* bits
const FL: u32 = 0x1;
const FR: u32 = 0x2;
const FC: u32 = 0x4;
const LFE: u32 = 0x8;
const BL: u32 = 0x10;
const BR: u32 = 0x20;
const FLC: u32 = 0x40;
const FRC: u32 = 0x80;
const BC: u32 = 0x100;
const SL: u32 = 0x200;
const SR: u32 = 0x400;
const TC: u32 = 0x800;

/// `get_audio_channels()` of a raw layout value, 0 outside the enum.
fn channels_of(layout: c_int) -> u32 {
    match layout {
        1 => 1,
        2 => 2,
        3 => 3,
        4 => 4,
        5 => 5,
        6 => 6,
        8 => 8,
        _ => 0,
    }
}

/// The C test's `is_layout()`.
fn is_layout(layout: c_int) -> bool {
    matches!(layout, 0..=6 | 8)
}

struct Device {
    name: &'static str,
    mask: u32,
    channels: u32,
}

/// The C test's `devices[]`: masks devices report. The first five are the
/// ones the monitor lists.
const DEVICES: [Device; 16] = [
    Device {
        name: "2.1",
        mask: FL | FR | LFE,
        channels: 3,
    },
    Device {
        name: "surround",
        mask: FL | FR | FC | BC,
        channels: 4,
    },
    Device {
        name: "4.1",
        mask: FL | FR | FC | LFE | BC,
        channels: 5,
    },
    Device {
        name: "5.1 (back)",
        mask: FL | FR | FC | LFE | BL | BR,
        channels: 6,
    },
    Device {
        name: "7.1 (wide)",
        mask: FL | FR | FC | LFE | BL | BR | FLC | FRC,
        channels: 8,
    },
    Device {
        name: "mono",
        mask: FC,
        channels: 1,
    },
    Device {
        name: "stereo",
        mask: FL | FR,
        channels: 2,
    },
    Device {
        name: "quad",
        mask: FL | FR | BL | BR,
        channels: 4,
    },
    Device {
        name: "5.1 (side)",
        mask: FL | FR | FC | LFE | SL | SR,
        channels: 6,
    },
    Device {
        name: "6.1",
        mask: FL | FR | FC | LFE | BC | SL | SR,
        channels: 7,
    },
    Device {
        name: "7.1 (surround)",
        mask: FL | FR | FC | LFE | BL | BR | SL | SR,
        channels: 8,
    },
    Device {
        name: "10 outputs",
        mask: FL | FR | FC | LFE | BL | BR | FLC | FRC | SL | SR,
        channels: 10,
    },
    Device {
        name: "12 outputs",
        mask: FL | FR | FC | LFE | BL | BR | FLC | FRC | BC | SL | SR | TC,
        channels: 12,
    },
    Device {
        name: "no mask, 7",
        mask: 0,
        channels: 7,
    },
    Device {
        name: "no mask, 10",
        mask: 0,
        channels: 10,
    },
    Device {
        name: "no mask, 0",
        mask: 0,
        channels: 0,
    },
];

// ---- test_monitor_speaker_layout.c ----

/// Every device gets a real layout with its own channel count, or none.
#[test]
fn layout_has_device_channel_count() {
    for d in &DEVICES {
        let layout = speaker_layout(d.mask, d.channels);
        assert!(
            is_layout(layout),
            "{} (mask {:#05x}, {} channels): layout {layout} does not exist",
            d.name,
            d.mask,
            d.channels
        );
        assert!(
            layout == UNKNOWN || channels_of(layout) == d.channels,
            "{} (mask {:#05x}, {} channels): layout {layout} has {} channels",
            d.name,
            d.mask,
            d.channels,
            channels_of(layout)
        );
    }
}

/// A mask whose channel count disagrees with the device's is not used: the
/// monitor copies nChannels floats per frame.
#[test]
fn layout_mask_with_other_count_is_not_used() {
    let mut prev = None;
    for d in &DEVICES {
        if prev == Some(d.mask) {
            continue;
        }
        prev = Some(d.mask);
        for channels in 0..=16 {
            let layout = speaker_layout(d.mask, channels);
            if layout == UNKNOWN {
                continue;
            }
            assert!(
                is_layout(layout) && channels_of(layout) == channels,
                "mask {:#05x} with {channels} channels: layout {layout}",
                d.mask
            );
        }
    }
}

/// What the map gave before #2559 and still gives: the listed masks map to
/// their layouts, and any other mask to the layout with the device's
/// channel count where there is one.
#[test]
fn layout_known_devices_keep_their_layout() {
    assert_eq!(speaker_layout(FL | FR | LFE, 3), TWO_POINT_ONE);
    assert_eq!(speaker_layout(FL | FR | FC | BC, 4), FOUR_POINT_ZERO);
    assert_eq!(speaker_layout(FL | FR | FC | LFE | BC, 5), FOUR_POINT_ONE);
    assert_eq!(
        speaker_layout(FL | FR | FC | LFE | BL | BR, 6),
        FIVE_POINT_ONE
    );
    assert_eq!(
        speaker_layout(FL | FR | FC | LFE | BL | BR | FLC | FRC, 8),
        SEVEN_POINT_ONE
    );

    assert_eq!(speaker_layout(FC, 1), MONO);
    assert_eq!(speaker_layout(FL | FR, 2), STEREO);
    assert_eq!(speaker_layout(FL | FR | BL | BR, 4), FOUR_POINT_ZERO);
    assert_eq!(
        speaker_layout(FL | FR | FC | LFE | SL | SR, 6),
        FIVE_POINT_ONE
    );
    assert_eq!(
        speaker_layout(FL | FR | FC | LFE | BL | BR | SL | SR, 8),
        SEVEN_POINT_ONE
    );
    assert_eq!(speaker_layout(0, 2), STEREO);
}

// ---- test_monitor_wasapi_steps.c, speaker layout ----

struct MaskRow {
    name: &'static str,
    mask: u32,
    /// The layout for 0 to 9 channels.
    layout: [c_int; 10],
}

/// The C test's `rows[]`.
const ROWS: [MaskRow; 13] = [
    // the five masks the monitor lists: their own layout for their own
    // channel count, none for any other count
    MaskRow {
        name: "2.1",
        mask: MASK_2POINT1,
        layout: [0, 0, 0, 3, 0, 0, 0, 0, 0, 0],
    },
    MaskRow {
        name: "surround",
        mask: MASK_SURROUND,
        layout: [0, 0, 0, 0, 4, 0, 0, 0, 0, 0],
    },
    MaskRow {
        name: "4.1",
        mask: MASK_4POINT1,
        layout: [0, 0, 0, 0, 0, 5, 0, 0, 0, 0],
    },
    MaskRow {
        name: "5.1",
        mask: MASK_5POINT1,
        layout: [0, 0, 0, 0, 0, 0, 6, 0, 0, 0],
    },
    MaskRow {
        name: "7.1",
        mask: MASK_7POINT1,
        layout: [0, 0, 0, 0, 0, 0, 0, 0, 8, 0],
    },
    // any other mask: the layout numbered by the channel count, none for 0,
    // 7 (no layout is numbered 7) or 9
    MaskRow {
        name: "no mask",
        mask: 0x0,
        layout: [0, 1, 2, 3, 4, 5, 6, 0, 8, 0],
    },
    MaskRow {
        name: "mono",
        mask: 0x4,
        layout: [0, 1, 2, 3, 4, 5, 6, 0, 8, 0],
    },
    MaskRow {
        name: "stereo",
        mask: 0x3,
        layout: [0, 1, 2, 3, 4, 5, 6, 0, 8, 0],
    },
    MaskRow {
        name: "quad",
        mask: 0x33,
        layout: [0, 1, 2, 3, 4, 5, 6, 0, 8, 0],
    },
    MaskRow {
        name: "5.1 (side)",
        mask: 0x60F,
        layout: [0, 1, 2, 3, 4, 5, 6, 0, 8, 0],
    },
    MaskRow {
        name: "6.1",
        mask: 0x70F,
        layout: [0, 1, 2, 3, 4, 5, 6, 0, 8, 0],
    },
    MaskRow {
        name: "7.1 (surround)",
        mask: 0x63F,
        layout: [0, 1, 2, 3, 4, 5, 6, 0, 8, 0],
    },
    MaskRow {
        name: "every bit",
        mask: 0xFFFF_FFFF,
        layout: [0, 1, 2, 3, 4, 5, 6, 0, 8, 0],
    },
];

#[test]
fn steps_constants() {
    assert_eq!(MASK_2POINT1, 0x0B);
    assert_eq!(MASK_SURROUND, 0x107);
    assert_eq!(MASK_4POINT1, 0x10F);
    assert_eq!(MASK_5POINT1, 0x3F);
    assert_eq!(MASK_7POINT1, 0xFF);
    assert_eq!(UNKNOWN, 0);
    assert_eq!(MONO, 1);
    assert_eq!(STEREO, 2);
    assert_eq!(TWO_POINT_ONE, 3);
    assert_eq!(FOUR_POINT_ZERO, 4);
    assert_eq!(FOUR_POINT_ONE, 5);
    assert_eq!(FIVE_POINT_ONE, 6);
    assert_eq!(SEVEN_POINT_ONE, 8);
}

/// Every mask with 0 to 9 channels. A listed mask with another channel
/// count gives no layout, even where the count has one (5.1 with 2
/// channels), where an unlisted mask would give it.
#[test]
fn steps_every_mask_and_count() {
    for row in &ROWS {
        for (channels, &want) in (0u32..).zip(row.layout.iter()) {
            let got = speaker_layout(row.mask, channels);
            assert_eq!(
                got, want,
                "{} (mask {:#x}) with {channels} channels",
                row.name, row.mask
            );
        }
    }
}

/// 7 channels give layout value 7 in the default, which no layout has
/// (`get_audio_channels()` of it is 0): none, under every mask.
#[test]
fn steps_seven_channels_have_no_layout() {
    for row in &ROWS {
        assert_eq!(speaker_layout(row.mask, 7), UNKNOWN, "{}", row.name);
    }
    assert_eq!(speaker_layout(0x7FF, 7), UNKNOWN);
}

/// Counts past 8 give none, whatever the mask; nothing is truncated.
#[test]
fn steps_large_counts_have_no_layout() {
    const COUNTS: [u32; 7] = [9, 10, 16, 258, 0x8000_0002, u32::MAX - 1, u32::MAX];
    for row in &ROWS {
        for count in COUNTS {
            assert_eq!(
                speaker_layout(row.mask, count),
                UNKNOWN,
                "{} with {count}",
                row.name
            );
        }
    }
}

// ---- new: against a model of the origin/master C ----

/// origin/master's `audio_monitor_speaker_layout()`, its switch and check
/// inline.
fn model(mask: u32, channels: u32) -> c_int {
    let layout: i64 = match mask {
        0x0000_000B => 3,
        0x0000_0107 => 4,
        0x0000_010F => 5,
        0x0000_003F => 6,
        0x0000_00FF => 8,
        _ if channels <= 8 => i64::from(channels),
        _ => 0,
    };
    let count = match layout {
        1..=6 | 8 => layout,
        _ => 0,
    };
    if count == i64::from(channels) {
        c_int::try_from(layout).unwrap_or(c_int::MAX)
    } else {
        0
    }
}

/// Masks to try: the listed ones, each one bit off, each single bit, none
/// and every bit.
fn edge_masks() -> Vec<u32> {
    let mut masks = vec![0, u32::MAX, 0x7FF, 0x33, 0x60F, 0x63F, 0x70F];
    for listed in [
        MASK_2POINT1,
        MASK_SURROUND,
        MASK_4POINT1,
        MASK_5POINT1,
        MASK_7POINT1,
    ] {
        masks.push(listed);
        for bit in 0..32 {
            masks.push(listed ^ (1 << bit));
        }
    }
    masks.extend((0..32).map(|bit| 1u32 << bit));
    masks
}

#[test]
fn edge_every_mask_against_the_model() {
    let counts = (0..=40).chain([
        255,
        256,
        258,
        0x8000_0000,
        0x8000_0008,
        u32::MAX - 1,
        u32::MAX,
    ]);
    let counts: Vec<u32> = counts.collect();
    for mask in edge_masks() {
        for &channels in &counts {
            assert_eq!(
                speaker_layout(mask, channels),
                model(mask, channels),
                "mask {mask:#x}, {channels} channels"
            );
        }
    }
}

/// A layout is returned only with the device's own channel count.
#[test]
fn edge_layout_always_has_the_device_count() {
    for mask in edge_masks() {
        for channels in 0..=16 {
            let layout = speaker_layout(mask, channels);
            assert!(
                layout == UNKNOWN || channels_of(layout) == channels,
                "mask {mask:#x}, {channels}: {layout}"
            );
            assert!(is_layout(layout) && layout != 7);
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(4096)))]

    #[test]
    fn edge_any_mask_and_count(mask in any::<u32>(), channels in prop_oneof![0u32..12, any::<u32>()]) {
        prop_assert_eq!(speaker_layout(mask, channels), model(mask, channels));
    }
}
