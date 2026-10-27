//! Tier 1: `obs_core::canvas_rules`, the canvas flag rules and setter
//! checks.
//!
//! The `steps_*` cases port `test/cmocka/test_canvas_rules_steps.c` 1:1: its
//! 11 cases under the same names, in the same order, with the same values
//! and expectations, the "characterized, not endorsed" ones included. The C
//! calls the hidden `obs_canvas_rules_*` functions; here the safe core is
//! called directly. The C's checks of the copied values against `obs.h`
//! are checks of the literal values here (Tier 3 compares them with the C
//! headers).
//!
//! The `edge_*` cases are new: proptests over every `u32` flags value, every
//! saved `i64`, every channel, size and format, and any name's first byte,
//! against models written from the C's expressions.

use core::ffi::c_int;

use obs_core::canvas_rules::{
    ACTIVATE, DEVICE, EPHEMERAL, FORMAT_NONE, MAIN, MAX_CHANNELS, MIX_AUDIO, PREVIEW, PROGRAM,
    PUBLIC, REMOVED, SCENE_REF, channel_ok, format_set, global_signals, holds_scenes, is_public,
    main_flags, main_view, mark_removed, mix_audio, name_empty, public_flags, removable, removed,
    renames, resets, saves, size_set,
};
use proptest::prelude::*;

const UNKNOWN: u32 = 1 << 5;

/// The five public flags, `MAIN`, `REMOVED` and an unknown bit.
const BITS: [u32; 7] = [
    MAIN, ACTIVATE, MIX_AUDIO, SCENE_REF, EPHEMERAL, REMOVED, UNKNOWN,
];

/// `VIDEO_FORMAT_I420`, `VIDEO_FORMAT_NV12` and `VIDEO_FORMAT_P010`
/// (`media-io/video-io.h`).
const VIDEO_FORMAT_I420: c_int = 1;
const VIDEO_FORMAT_NV12: c_int = 2;
const VIDEO_FORMAT_P010: c_int = 17;

fn combo(n: u32) -> u32 {
    BITS.iter()
        .enumerate()
        .filter(|(i, _)| n & (1 << i) != 0)
        .fold(0, |flags, (_, bit)| flags | bit)
}

fn check_flag_rule(rule: &str, flags: u32, actual: bool, expected: bool) {
    assert_eq!(actual, expected, "{rule}({flags:#x})");
}

/// `obs_load_canvas()`: `(uint32_t)obs_data_get_int(data, "flags")`, then
/// the public mask.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn load_flags(saved: i64) -> u32 {
    public_flags(saved as u32)
}

// ---------------------------------------------------------------------------
// the C's cases

#[test]
fn steps_constants() {
    assert_eq!(MAIN, 1 << 0);
    assert_eq!(ACTIVATE, 1 << 1);
    assert_eq!(MIX_AUDIO, 1 << 2);
    assert_eq!(SCENE_REF, 1 << 3);
    assert_eq!(EPHEMERAL, 1 << 4);
    assert_eq!(REMOVED, 0x8000_0000);
    assert_eq!(PUBLIC, ACTIVATE | MIX_AUDIO | SCENE_REF | EPHEMERAL);
    assert_eq!(PUBLIC, 0x1e);
    assert_eq!(FORMAT_NONE, 0);
    assert_eq!(MAX_CHANNELS, 64);
    assert_eq!(PROGRAM, ACTIVATE | MIX_AUDIO | SCENE_REF);
    assert_eq!(PREVIEW, EPHEMERAL);
    assert_eq!(DEVICE, ACTIVATE | EPHEMERAL);
}

#[test]
fn steps_constructor_flags() {
    // obs_create_main_canvas()
    assert_eq!(main_flags(), MAIN | PROGRAM);
    assert_eq!(main_flags(), 0x0f);

    // obs_canvas_create() and _create_private(): only the public flags
    assert_eq!(public_flags(0), 0);
    assert_eq!(public_flags(PROGRAM), PROGRAM);
    assert_eq!(public_flags(PREVIEW), PREVIEW);
    assert_eq!(public_flags(DEVICE), DEVICE);
    assert_eq!(public_flags(MAIN), 0);
    assert_eq!(public_flags(MAIN | PROGRAM), PROGRAM);
    assert_eq!(public_flags(REMOVED), 0);
    assert_eq!(public_flags(REMOVED | EPHEMERAL), EPHEMERAL);
    assert_eq!(public_flags(u32::MAX), 0x1e);
    for bit in 0..32 {
        let flags = 1_u32 << bit;
        let kept = (1..=4).contains(&bit);
        assert_eq!(public_flags(flags), if kept { flags } else { 0 });
    }
    for n in 0..(1 << BITS.len()) {
        let flags = combo(n);
        assert_eq!(public_flags(flags), flags & 0x1e);
    }
}

#[test]
fn steps_load_flags_from_int64() {
    assert_eq!(load_flags(0), 0);
    assert_eq!(load_flags(i64::from(PROGRAM)), PROGRAM);
    assert_eq!(load_flags(i64::from(EPHEMERAL)), EPHEMERAL);
    // a saved MAIN or REMOVED is masked off
    assert_eq!(load_flags(i64::from(MAIN | PROGRAM)), PROGRAM);
    assert_eq!(load_flags(0x8000_0000), 0);
    assert_eq!(load_flags(0x8000_001f), 0x1e);
    assert_eq!(load_flags(0xffff_ffff), 0x1e);
    // characterized, not endorsed: the int64 is truncated before the mask,
    // so bits above 31 are dropped silently and a negative value keeps the
    // public flags of its low word
    assert_eq!(load_flags(0x1_0000_0000), 0);
    assert_eq!(load_flags(0x1_0000_001e), 0x1e);
    assert_eq!(load_flags(0x1_0000_0001), 0);
    assert_eq!(load_flags(-1), 0x1e);
    assert_eq!(load_flags(-2), 0x1e);
    assert_eq!(load_flags(-32), 0);
    assert_eq!(load_flags(i64::MIN), 0);
    assert_eq!(load_flags(i64::MAX), 0x1e);
}

#[test]
fn steps_flag_rules_every_combination() {
    for n in 0..(1 << BITS.len()) {
        let f = combo(n);
        let is_main = f & MAIN != 0;
        let is_removed = f & REMOVED != 0;
        check_flag_rule("main_view", f, main_view(f), f & ACTIVATE != 0);
        check_flag_rule("mix_audio", f, mix_audio(f), f & MIX_AUDIO != 0);
        check_flag_rule("holds_scenes", f, holds_scenes(f), f & SCENE_REF != 0);
        check_flag_rule("saves", f, saves(f), f & EPHEMERAL == 0 && !is_removed);
        check_flag_rule("resets", f, resets(f), !is_main);
        check_flag_rule("global_signals", f, global_signals(f), is_main);
        check_flag_rule("renames", f, renames(f), !is_main);
        check_flag_rule("removable", f, removable(f), !is_main && !is_removed);
        check_flag_rule("removed", f, removed(f), is_removed);
        assert_eq!(mark_removed(f), f | REMOVED);
    }
}

#[test]
fn steps_flag_rules_every_bit() {
    for bit in 0..32 {
        let f = 1_u32 << bit;
        check_flag_rule("main_view", f, main_view(f), bit == 1);
        check_flag_rule("mix_audio", f, mix_audio(f), bit == 2);
        check_flag_rule("holds_scenes", f, holds_scenes(f), bit == 3);
        check_flag_rule("saves", f, saves(f), bit != 4 && bit != 31);
        check_flag_rule("resets", f, resets(f), bit != 0);
        check_flag_rule("global_signals", f, global_signals(f), bit == 0);
        check_flag_rule("renames", f, renames(f), bit != 0);
        check_flag_rule("removable", f, removable(f), bit != 0 && bit != 31);
        check_flag_rule("removed", f, removed(f), bit == 31);
    }
    // every flag set
    check_flag_rule("saves", u32::MAX, saves(u32::MAX), false);
    check_flag_rule("removable", u32::MAX, removable(u32::MAX), false);
    assert_eq!(mark_removed(u32::MAX), u32::MAX);
    assert_eq!(mark_removed(0), REMOVED);
}

#[test]
fn steps_canvas_kinds() {
    // the main canvas: a MAIN_VIEW that mixes audio, holds its scenes, is
    // saved, signals globally and cannot be reset, renamed or removed
    let f = main_flags();
    assert!(main_view(f));
    assert!(mix_audio(f));
    assert!(holds_scenes(f));
    assert!(saves(f));
    assert!(!resets(f));
    assert!(global_signals(f));
    assert!(!renames(f));
    assert!(!removable(f));

    // a program canvas from obs_canvas_create(PROGRAM)
    let f = public_flags(PROGRAM);
    assert!(main_view(f));
    assert!(mix_audio(f));
    assert!(holds_scenes(f));
    assert!(saves(f));
    assert!(resets(f));
    assert!(!global_signals(f));
    assert!(renames(f));
    assert!(removable(f));

    // a preview canvas: an AUX_VIEW that is not saved
    let f = public_flags(PREVIEW);
    assert!(!main_view(f));
    assert!(!mix_audio(f));
    assert!(!holds_scenes(f));
    assert!(!saves(f));

    // a device canvas: a MAIN_VIEW that is not saved
    let f = public_flags(DEVICE);
    assert!(main_view(f));
    assert!(!mix_audio(f));
    assert!(!saves(f));

    // a caller asking for MAIN gets a plain canvas
    let f = public_flags(MAIN | PROGRAM);
    assert!(!global_signals(f));
    assert!(renames(f));
    assert!(removable(f));
}

#[test]
fn steps_public_rule() {
    assert!(is_public(false));
    assert!(!is_public(true));
}

#[test]
fn steps_video_info_checks() {
    assert!(!size_set(0));
    assert!(size_set(1));
    assert!(size_set(1920));
    assert!(size_set(u32::MAX));

    assert!(!format_set(FORMAT_NONE));
    assert!(format_set(VIDEO_FORMAT_I420));
    assert!(format_set(VIDEO_FORMAT_NV12));
    assert!(format_set(VIDEO_FORMAT_P010));
    // characterized, not endorsed: a format outside the enum counts as set
    assert!(format_set(-1));
    assert!(format_set(9999));
    assert!(format_set(c_int::MIN));
    assert!(format_set(c_int::MAX));
}

#[test]
fn steps_channel_bound() {
    assert!(channel_ok(0));
    assert!(channel_ok(1));
    assert!(channel_ok(MAX_CHANNELS - 1));
    assert!(!channel_ok(MAX_CHANNELS));
    assert!(!channel_ok(MAX_CHANNELS + 1));
    assert!(!channel_ok(0x8000_0000));
    assert!(!channel_ok(u32::MAX));
    for ch in 0..1024 {
        assert_eq!(channel_ok(ch), ch < 64);
    }
}

#[test]
fn steps_name_checks() {
    // the C passes the name; the core gets its first byte, None for NULL
    assert!(name_empty(None));
    assert!(name_empty(Some(b'\0')));
    assert!(!name_empty(Some(b' ')));
    assert!(!name_empty(Some(b'a')));
    assert!(!name_empty(Some(b'M')));
    // only the first byte is read: "\0b" is empty
    assert!(name_empty(Some(0)));
    assert!(!name_empty(Some(0xff)));
}

#[test]
fn steps_remove_twice() {
    let mut f = public_flags(PROGRAM);
    assert!(removable(f));
    assert!(!removed(f));
    f = mark_removed(f);
    assert_eq!(f, PROGRAM | REMOVED);
    assert!(removed(f));
    // the second obs_canvas_remove() returns before anything
    assert!(!removable(f));
    // a removed canvas is not saved
    assert!(!saves(f));
    // characterized, not endorsed: a removed canvas can still be reset and
    // renamed, and keeps its other rules
    assert!(resets(f));
    assert!(renames(f));
    assert!(holds_scenes(f));
    assert!(mix_audio(f));
    // a reloaded removed canvas comes back as not removed
    assert_eq!(public_flags(f), PROGRAM);
}

// ---------------------------------------------------------------------------
// edge cases: models of the C's expressions

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4096))]

    #[test]
    fn edge_flag_rules_any_flags(f in any::<u32>()) {
        prop_assert_eq!(public_flags(f), f & 0x1e);
        prop_assert_eq!(main_view(f), f & 0x2 != 0);
        prop_assert_eq!(mix_audio(f), f & 0x4 != 0);
        prop_assert_eq!(holds_scenes(f), f & 0x8 != 0);
        prop_assert_eq!(saves(f), f & 0x8000_0010 == 0);
        prop_assert_eq!(resets(f), f & 0x1 == 0);
        prop_assert_eq!(global_signals(f), f & 0x1 != 0);
        prop_assert_eq!(renames(f), f & 0x1 == 0);
        prop_assert_eq!(removable(f), f & 0x8000_0001 == 0);
        prop_assert_eq!(mark_removed(f), f | 0x8000_0000);
        prop_assert_eq!(removed(f), f & 0x8000_0000 != 0);
    }

    /// Any saved value, truncated as the C's cast, never keeps `MAIN`,
    /// `REMOVED` or an unknown bit.
    #[test]
    fn edge_load_any_int64(saved in any::<i64>()) {
        let flags = load_flags(saved);
        prop_assert_eq!(flags, u32::from(saved.to_le_bytes()[0]) & 0x1e);
        prop_assert!(!global_signals(flags));
        prop_assert!(!removed(flags));
        prop_assert!(removable(flags));
    }

    #[test]
    fn edge_values_any_input(size in any::<u32>(), format in any::<c_int>(), channel in any::<u32>(), byte in any::<Option<u8>>(), private: bool) {
        prop_assert_eq!(size_set(size), size != 0);
        prop_assert_eq!(format_set(format), format != 0);
        prop_assert_eq!(channel_ok(channel), channel < 64);
        prop_assert_eq!(name_empty(byte), byte.is_none() || byte == Some(0));
        prop_assert_eq!(is_public(private), !private);
    }
}
