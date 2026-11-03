//! Tier 1: the rules of `libobs/obs-hotkey-save-load.c.h`.
//!
//! The C tests `test_hotkey_context_data`, `test_hotkey_bindings_codec`,
//! `test_hotkey_volume_keys` and `test_hotkey_load` pin these rules through the
//! public API, and `save_load_parity` compares the shims with the C.

use std::cell::Cell;

use obs_hotkey::name_map::{OBS_KEY_VK_VOLUME_DOWN, OBS_KEY_VK_VOLUME_UP};
use obs_hotkey::save_load::{
    BINDINGS_CHANGED, VOLUME_KEYS_FIXED, is_volume_up_down, load_signals, loaded_key,
    save_marks_fixed,
};

const UP: i32 = OBS_KEY_VK_VOLUME_UP;
const DOWN: i32 = OBS_KEY_VK_VOLUME_DOWN;
const OTHER: i32 = OBS_KEY_VK_VOLUME_UP + 1000;

#[test]
fn names_are_the_c_strings() {
    assert_eq!(BINDINGS_CHANGED.to_str(), Ok("hotkey_bindings_changed"));
    assert_eq!(VOLUME_KEYS_FIXED.to_str(), Ok("volume_keys_fixed"));
}

#[test]
fn only_the_two_volume_keys_are_volume_keys() {
    assert!(is_volume_up_down(UP));
    assert!(is_volume_up_down(DOWN));
    assert!(!is_volume_up_down(OTHER));
    assert!(!is_volume_up_down(0));
    assert_ne!(UP, DOWN);
}

#[test]
fn saved_volume_key_swaps_only_when_unfixed_on_a_swapped_host() {
    for (key, swapped_to) in [(UP, DOWN), (DOWN, UP)] {
        assert_eq!(loaded_key(key, true, || false, || true), swapped_to);
        assert_eq!(loaded_key(key, true, || true, || true), key);
        assert_eq!(loaded_key(key, true, || false, || false), key);
        assert_eq!(loaded_key(key, false, || false, || true), key);
    }
    assert_eq!(loaded_key(OTHER, true, || false, || true), OTHER);
}

/// The C's `&&`: a later read does not happen once the result is known.
#[test]
fn loaded_key_reads_in_the_c_order() {
    let reads = |key, saved, fixed: bool| {
        let (fixed_reads, swapped_reads) = (Cell::new(0), Cell::new(0));
        loaded_key(
            key,
            saved,
            || {
                fixed_reads.set(fixed_reads.get() + 1);
                fixed
            },
            || {
                swapped_reads.set(swapped_reads.get() + 1);
                true
            },
        );
        (fixed_reads.get(), swapped_reads.get())
    };
    assert_eq!(reads(UP, false, false), (0, 0));
    assert_eq!(reads(OTHER, true, false), (0, 0));
    assert_eq!(reads(UP, true, true), (1, 0));
    assert_eq!(reads(UP, true, false), (1, 1));
}

#[test]
fn a_load_signals_for_any_item_or_a_removal() {
    assert!(!load_signals(0, false));
    assert!(load_signals(0, true));
    assert!(load_signals(1, false));
    assert!(load_signals(1, true));
    assert!(load_signals(usize::MAX, false));
}

#[test]
fn only_scene_collection_data_marks_a_volume_key() {
    assert!(save_marks_fixed(true, UP));
    assert!(save_marks_fixed(true, DOWN));
    assert!(!save_marks_fixed(false, UP));
    assert!(!save_marks_fixed(false, DOWN));
    assert!(!save_marks_fixed(true, OTHER));
}
