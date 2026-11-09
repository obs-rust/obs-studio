//! Exact parity of the unchanged C callbacks and Rust shims, including refcounts,
//! writes and operation order. Excludes LONG_MAX (C signed-overflow UB).
#![allow(
    clippy::unnecessary_cast,
    reason = "C long is i32 on Windows and i64 on 64-bit Unix"
)]
use core::ffi::{c_char, c_long, c_void};
use obs_scene::ffi::item_hotkeys::{
    Compare, Descriptions, Owners, Release, SetVisible, Visible, obs_sceneitem_hotkey_rename,
    obs_sceneitem_hotkey_trigger,
};
use proptest::prelude::*;
use std::ffi::CString;

type Callback = unsafe extern "C" fn(
    *mut c_void,
    bool,
    bool,
    Owners,
    Compare,
    Visible,
    SetVisible,
    Release,
) -> bool;
type Rename =
    unsafe extern "C" fn(usize, *const c_char, *const c_char, *const c_char, Descriptions);
unsafe extern "C" {
    fn oracle_scene_hotkey_probe(
        owners: c_long,
        visible: bool,
        show: bool,
        pressed: bool,
        race: bool,
        observed: c_long,
        callback: Callback,
    ) -> u32;
    fn oracle_scene_hotkey_rename_probe(
        show: *const c_char,
        hide: *const c_char,
        name: *const c_char,
        pair: usize,
        callback: Rename,
    ) -> u32;
}

fn check(owners: c_long, visible: bool, show: bool, pressed: bool) {
    check_race(owners, visible, show, pressed, None);
}

fn check_race(owners: c_long, visible: bool, show: bool, pressed: bool, race: Option<c_long>) {
    // SAFETY: the oracle owns real stack items throughout each synchronous callback.
    assert_eq!(
        unsafe {
            oracle_scene_hotkey_probe(
                owners,
                visible,
                show,
                pressed,
                race.is_some(),
                race.unwrap_or(0),
                obs_sceneitem_hotkey_trigger,
            )
        },
        0,
        "owners={owners} visible={visible} show={show} pressed={pressed}"
    );
}
fn rename(show: Option<&[u8]>, hide: Option<&[u8]>, name: Option<&[u8]>, pair: usize) {
    let strings = [show, hide, name].map(|v| v.map(|v| CString::new(v).unwrap()));
    let pointers = strings
        .each_ref()
        .map(|v| v.as_ref().map_or(core::ptr::null(), |v| v.as_ptr()));
    // SAFETY: strings and the oracle's complete private fixtures outlive dispatch.
    assert_eq!(
        unsafe {
            oracle_scene_hotkey_rename_probe(
                pointers[0],
                pointers[1],
                pointers[2],
                pair,
                obs_sceneitem_hotkey_rename,
            )
        },
        0
    );
}

#[test]
fn expired_refs_and_press_edges() {
    for observed in [-1, 0, 3] {
        for show in [false, true] {
            for pressed in [false, true] {
                check_race(1, !show, show, pressed, Some(observed));
            }
        }
    }
    for owners in [c_long::MIN, -1, 0, 1, 2, c_long::MAX - 1] {
        for visible in [false, true] {
            for show in [false, true] {
                for pressed in [false, true] {
                    check(owners, visible, show, pressed);
                }
            }
        }
    }
}
#[test]
fn rename_edges() {
    for template in [
        None,
        Some(&b""[..]),
        Some(&b"%1"[..]),
        Some(&b"[%1] show %1"[..]),
        Some(&b"literal\xff"[..]),
    ] {
        for name in [
            None,
            Some(&b""[..]),
            Some(&b"original"[..]),
            Some(&b"%1\xfe"[..]),
        ] {
            for pair in [0, 123, usize::MAX] {
                rename(template, template, name, pair);
            }
        }
    }
}
proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn callbacks_match_c(owners in -10i32..100_000, visible in any::<bool>(), show in any::<bool>(), pressed in any::<bool>(), race in prop::option::of(-3i32..16)) {
        check_race(owners as c_long, visible, show, pressed, race.map(|value| value as c_long));
    }
    #[test]
    fn descriptions_match_c(show in prop::collection::vec(1u8..=255, 0..96), hide in prop::collection::vec(1u8..=255, 0..96), name in prop::collection::vec(1u8..=255, 0..64), pair in any::<usize>()) {
        let mut template = show;
        template.extend_from_slice(b"%1%1");
        rename(Some(&template), Some(&hide), Some(&name), pair);
    }
}
