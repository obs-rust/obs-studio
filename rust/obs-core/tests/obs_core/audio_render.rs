//! Tier 1: the safe core of `libobs/obs-audio-render.c.h`.
//!
//! `tree_choice`, `silence_check`, `clear_mixes_choice` and
//! `backwards_choice` mirror the `obs-audio-render.c` cases of
//! `test/cmocka/test_audio_buffering_subsample.c` one to one, with the same
//! names, inputs and expected values. The cmocka cases
//! `tree_wrapper_tags_once` and `silence_wrappers_use_the_source_fields`
//! check the wrappers in `obs-audio.c` and have no counterpart here.
//!
//! The cases after those pin the edges of the safe API, and proptests check
//! that no input panics and the invariants.

use obs_core::audio_render::{
    BackwardsChoice, MAX_AUDIO_MIXES, MonitorDedup, SOURCE_AUDIO, SOURCE_COMPOSITE, TYPE_INPUT,
    TreeChoice, backwards, clear_mixes, silence, tree,
};
use proptest::prelude::*;

const START: u64 = 1_000_000_000;

/// `OBS_SOURCE_TYPE_*` and `OBS_SOURCE_*` from `obs-source.h`.
const TYPE_FILTER: u32 = 1;
const TYPE_TRANSITION: u32 = 2;
const TYPE_SCENE: u32 = 3;
const SOURCE_VIDEO: u32 = 1 << 0;
const SOURCE_ASYNC: u32 = 1 << 2;

#[test]
fn tree_choice() {
    let audio_in = SOURCE_AUDIO;
    let av_in = SOURCE_AUDIO | SOURCE_VIDEO | SOURCE_ASYNC;

    // Removed: skipped before anything else.
    assert_eq!(
        tree(true, false, TYPE_INPUT, audio_in, false),
        TreeChoice::Skip
    );
    assert_eq!(
        tree(true, true, TYPE_INPUT, audio_in, false),
        TreeChoice::Skip
    );

    // Not in the render order yet: added, whatever the type.
    assert_eq!(
        tree(false, false, TYPE_INPUT, audio_in, true),
        TreeChoice::Add
    );
    assert_eq!(
        tree(false, false, TYPE_SCENE, SOURCE_COMPOSITE, false),
        TreeChoice::Add
    );

    // Present, an individual audio source, not tagged yet: tagged.
    assert_eq!(
        tree(false, true, TYPE_INPUT, audio_in, false),
        TreeChoice::Duplicate
    );
    assert_eq!(
        tree(false, true, TYPE_INPUT, av_in, false),
        TreeChoice::Duplicate
    );

    // Present and tagged already: nothing.
    assert_eq!(
        tree(false, true, TYPE_INPUT, audio_in, true),
        TreeChoice::None
    );

    // Present but not individual: a scene, a transition, a composite input
    // with audio, an input without audio, a filter.
    let composite_audio = SOURCE_COMPOSITE | SOURCE_AUDIO;
    assert_eq!(
        tree(false, true, TYPE_SCENE, composite_audio, false),
        TreeChoice::None
    );
    assert_eq!(
        tree(false, true, TYPE_TRANSITION, composite_audio, false),
        TreeChoice::None
    );
    assert_eq!(
        tree(false, true, TYPE_INPUT, composite_audio, false),
        TreeChoice::None
    );
    assert_eq!(
        tree(false, true, TYPE_INPUT, SOURCE_VIDEO, false),
        TreeChoice::None
    );
    assert_eq!(
        tree(false, true, TYPE_FILTER, SOURCE_AUDIO, false),
        TreeChoice::None
    );
}

#[test]
fn silence_check() {
    let on = MonitorDedup {
        set: true,
        active: true,
        volume: 1.0,
        ..MonitorDedup::default()
    };

    // An unmuted, active capture source silences a monitored source.
    assert!(silence(&on, true, false));

    // Not the capture source itself, and not an unmonitored source.
    assert!(!silence(&on, true, true));
    assert!(!silence(&on, false, false));

    // No capture source, or inactive, or muted.
    assert!(!silence(&MonitorDedup { set: false, ..on }, true, false));
    assert!(!silence(
        &MonitorDedup {
            active: false,
            ..on
        },
        true,
        false
    ));
    assert!(!silence(&MonitorDedup { muted: true, ..on }, true, false));

    // The fader: within 0.0001 of 0 counts as muted.
    assert!(!silence(&MonitorDedup { volume: 0.0, ..on }, true, false));
    assert!(!silence(
        &MonitorDedup {
            volume: 0.00005,
            ..on
        },
        true,
        false
    ));
    assert!(silence(
        &MonitorDedup {
            volume: 0.001,
            ..on
        },
        true,
        false
    ));
}

#[test]
fn clear_mixes_choice() {
    assert_eq!(clear_mixes(0x01, 0x02), 0);
    assert_eq!(clear_mixes(0x03, 0x06), 0x02);
    assert_eq!(clear_mixes(0x3f, 0x3f), 0x3f);
    // Bits at and above MAX_AUDIO_MIXES are not mixes.
    assert_eq!(clear_mixes(0xffff_ffff, 0xffff_ffff), 0x3f);
    assert_eq!(clear_mixes(0x40, 0x40), 0);
}

#[test]
fn backwards_choice() {
    // Only at the maximum, with a timestamp, strictly before the tick.
    assert_eq!(
        backwards(false, START - 10, START, false),
        BackwardsChoice::None
    );
    assert_eq!(backwards(true, 0, START, false), BackwardsChoice::None);
    assert_eq!(backwards(true, START, START, false), BackwardsChoice::None);
    assert_eq!(
        backwards(true, START + 1, START, true),
        BackwardsChoice::None
    );

    // A render source is marked pending; another one is dropped.
    assert_eq!(
        backwards(true, START - 1, START, true),
        BackwardsChoice::Pending
    );
    assert_eq!(
        backwards(true, START - 1, START, false),
        BackwardsChoice::Ignore
    );
}

#[test]
fn edge_type_outside_the_enum_is_not_an_input() {
    assert_eq!(tree(false, true, 4, SOURCE_AUDIO, false), TreeChoice::None);
    assert_eq!(
        tree(false, true, u32::MAX, SOURCE_AUDIO, false),
        TreeChoice::None
    );
}

#[test]
fn edge_fader_boundaries_and_nan() {
    let on = MonitorDedup {
        set: true,
        active: true,
        volume: 1.0,
        ..MonitorDedup::default()
    };
    // Exactly the precision counts as muted; negative volumes too.
    assert!(!silence(
        &MonitorDedup {
            volume: 0.0001,
            ..on
        },
        true,
        false
    ));
    assert!(!silence(
        &MonitorDedup {
            volume: -0.00005,
            ..on
        },
        true,
        false
    ));
    // NaN is never close: not muted, so the source is silenced (as fabsf).
    assert!(silence(
        &MonitorDedup {
            volume: f32::NAN,
            ..on
        },
        true,
        false
    ));
}

#[test]
fn edge_backwards_at_the_ends() {
    assert_eq!(backwards(true, 1, u64::MAX, false), BackwardsChoice::Ignore);
    assert_eq!(
        backwards(true, u64::MAX, u64::MAX, true),
        BackwardsChoice::None
    );
    assert_eq!(backwards(true, 1, 0, true), BackwardsChoice::None);
}

proptest! {
    #[test]
    fn tree_rules(removed: bool, present: bool, kind: u32, flags: u32, duplicated: bool) {
        let choice = tree(removed, present, kind, flags, duplicated);
        let individual = kind == TYPE_INPUT && flags & SOURCE_AUDIO != 0 && flags & SOURCE_COMPOSITE == 0;
        let expected = if removed {
            TreeChoice::Skip
        } else if !present {
            TreeChoice::Add
        } else if individual && !duplicated {
            TreeChoice::Duplicate
        } else {
            TreeChoice::None
        };
        prop_assert_eq!(choice, expected);
    }

    #[test]
    fn silence_needs_every_condition(
        set: bool, active: bool, muted: bool, volume: f32, mixers: u32,
        monitoring: bool, is_dedup: bool,
    ) {
        let dedup = MonitorDedup { set, active, muted, volume, mixers };
        let silenced = silence(&dedup, monitoring, is_dedup);
        if silenced {
            prop_assert!(set && active && !muted && monitoring && !is_dedup);
            // Not within the precision of 0 (NaN is never close).
            prop_assert!(volume.is_nan() || volume.abs() > 0.0001);
        }
        // The mixers do not change the answer.
        prop_assert_eq!(silenced, silence(&MonitorDedup { mixers: 0, ..dedup }, monitoring, is_dedup));
    }

    #[test]
    fn clear_mixes_is_the_common_mixes(a: u32, b: u32) {
        let mixes = clear_mixes(a, b);
        prop_assert_eq!(mixes & !(a & b), 0);
        prop_assert!(mixes < 1 << MAX_AUDIO_MIXES);
        prop_assert_eq!(mixes, a & b & ((1 << MAX_AUDIO_MIXES) - 1));
    }

    #[test]
    fn backwards_only_behind_at_the_maximum(maxed: bool, ts: u64, start: u64, render: bool) {
        let choice = backwards(maxed, ts, start, render);
        let behind = maxed && ts != 0 && ts < start;
        prop_assert_eq!(choice != BackwardsChoice::None, behind);
        if behind {
            prop_assert_eq!(choice == BackwardsChoice::Pending, render);
        }
    }
}
