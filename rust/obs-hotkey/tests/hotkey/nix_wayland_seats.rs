//! Tier 1: the safe core of `libobs/obs-nix-wayland-seats.c.h`.
//!
//! The first ten tests port `test/cmocka/test_nix_wayland_seats_steps.c`
//! one to one, with the same names, inputs and expectations, in the same
//! order. A C seat array becomes its capabilities or names, and `SIZE_MAX`
//! becomes `None`.
//!
//! The cases after those pin the edges of each function, and the proptests
//! check each one against a model written from the C.

use std::cell::Cell;

use obs_hotkey::nix_wayland_seats::{
    self as seats, CAPABILITY_KEYBOARD as KEYBOARD, MAX_VERSION, MIN_VERSION, RELEASE_SINCE_VERSION,
};
use proptest::prelude::*;

/// `WL_SEAT_CAPABILITY_POINTER` and `WL_SEAT_CAPABILITY_TOUCH`.
const POINTER: u32 = 1;
const TOUCH: u32 = 4;

/// The C bind version: 0 for too old.
fn bind(offered: u32) -> u32 {
    seats::bind_version(offered).unwrap_or(0)
}

/// One C seat: its registry name and capabilities (the `wl_seat` pointer is
/// never read).
#[derive(Debug, Clone, Copy)]
struct Seat {
    name: u32,
    caps: u32,
}

const fn seat(name: u32, caps: u32) -> Seat {
    Seat { name, caps }
}

fn pick(s: &[Seat]) -> Option<usize> {
    seats::pick(s.iter().map(|s| s.caps))
}

fn find(s: &[Seat], name: u32) -> Option<usize> {
    seats::find_name(s.iter().map(|s| s.name), name)
}

#[test]
fn bind_version() {
    // Below version 4 the compositor is too old: nothing is bound.
    assert_eq!(bind(0), 0);
    assert_eq!(bind(1), 0);
    assert_eq!(bind(2), 0);
    assert_eq!(bind(3), 0);

    // From 4 to 7 the offered version is bound.
    assert_eq!(bind(4), 4);
    assert_eq!(bind(5), 5);
    assert_eq!(bind(6), 6);
    assert_eq!(bind(7), 7);

    // Newer versions are capped at 7.
    assert_eq!(bind(8), 7);
    assert_eq!(bind(10), 7);
    assert_eq!(bind(0x7fff_ffff), 7);
    assert_eq!(bind(0x8000_0000), 7);
    assert_eq!(bind(u32::MAX), 7);
}

#[test]
fn pick_no_seat() {
    // An empty seat array is NULL (a DARRAY that never grew).
    assert_eq!(pick(&[]), None);

    let s = [seat(1, KEYBOARD)];
    assert_eq!(pick(&s[..0]), None);
}

#[test]
fn pick_no_keyboard_seat() {
    let s = [
        seat(1, 0),
        seat(2, POINTER),
        seat(3, TOUCH),
        seat(4, POINTER | TOUCH),
        seat(5, !KEYBOARD),
    ];
    assert_eq!(pick(&s), None);
}

#[test]
fn pick_first_keyboard_seat() {
    // Two keyboard seats: the first wins, whatever its name.
    let s = [
        seat(9, POINTER),
        seat(7, KEYBOARD),
        seat(3, POINTER | KEYBOARD),
    ];
    assert_eq!(pick(&s), Some(1));

    // Only the first num seats count.
    assert_eq!(pick(&s[..1]), None);
    assert_eq!(pick(&s[..2]), Some(1));
    assert_eq!(pick(&s[2..3]), Some(0));

    // Any capabilities with the keyboard bit, the first seat included.
    let all = [seat(1, u32::MAX), seat(2, KEYBOARD)];
    assert_eq!(pick(&all), Some(0));

    let touch_keyboard = [seat(1, TOUCH), seat(2, TOUCH), seat(3, TOUCH | KEYBOARD)];
    assert_eq!(pick(&touch_keyboard), Some(2));
}

#[test]
fn lacks_keyboard() {
    // No keyboard bit: the platform then compares the seat with the
    // keyboard's, and releases the keyboard if it is that seat.
    assert!(seats::lacks_keyboard(0));
    assert!(seats::lacks_keyboard(POINTER));
    assert!(seats::lacks_keyboard(POINTER | TOUCH));
    assert!(seats::lacks_keyboard(!KEYBOARD));

    // A keyboard: the platform keeps it, without comparing the seat.
    assert!(!seats::lacks_keyboard(KEYBOARD));
    assert!(!seats::lacks_keyboard(POINTER | KEYBOARD));
    assert!(!seats::lacks_keyboard(u32::MAX));
}

#[test]
fn capabilities_of_another_seat() {
    // The platform's release is lacks_keyboard() && seat == keyboard_seat:
    // a seat other than the keyboard's (or any seat while there is no
    // keyboard) never releases it, keyboard or not.
    let caps = [0, POINTER, KEYBOARD, !KEYBOARD, u32::MAX];
    let lacks = [true, true, false, true, false];
    for (c, l) in caps.into_iter().zip(lacks) {
        let is_keyboard_seat = false;
        assert_eq!(seats::lacks_keyboard(c), l);
        assert!(!(seats::lacks_keyboard(c) && is_keyboard_seat));
    }
}

#[test]
fn find_name() {
    let s = [
        seat(12, KEYBOARD),
        seat(0, 0),
        seat(30, POINTER),
        seat(u32::MAX, KEYBOARD),
    ];
    assert_eq!(find(&s, 12), Some(0));
    assert_eq!(find(&s, 0), Some(1));
    assert_eq!(find(&s, 30), Some(2));
    assert_eq!(find(&s, u32::MAX), Some(3));

    // Only the first num seats count.
    assert_eq!(find(&s[..3], u32::MAX), None);

    // The name alone decides, not the capabilities.
    let same = [seat(5, 0), seat(5, KEYBOARD)];
    assert_eq!(find(&same, 5), Some(0));
    assert_eq!(find(&same[1..], 5), Some(0));
}

#[test]
fn find_unknown_name() {
    // A global the platform never bound (another interface, or a seat that
    // was too old) is removed: nothing is dropped.
    assert_eq!(find(&[], 0), None);
    assert_eq!(find(&[], 7), None);

    let s = [seat(1, KEYBOARD), seat(2, KEYBOARD)];
    assert_eq!(find(&s, 3), None);
    assert_eq!(find(&s, 0), None);
    assert_eq!(find(&s, u32::MAX), None);
}

#[test]
fn uses_release() {
    // Before wl_seat version 5 there is no release request.
    assert!(!seats::uses_release(0));
    assert!(!seats::uses_release(1));
    assert!(!seats::uses_release(4));

    assert!(seats::uses_release(5));
    assert!(seats::uses_release(6));
    assert!(seats::uses_release(7));
    assert!(seats::uses_release(8));
    assert!(seats::uses_release(u32::MAX));

    // The versions bound are 4 to 7: only 4 is destroyed.
    for offered in 4..12 {
        let bound = bind(offered);
        assert_eq!(seats::uses_release(bound), bound >= 5);
    }
}

#[test]
fn handover() {
    // The platform's steps for two keyboard seats, the keyboard's seat
    // losing its keyboard and then being removed, as obs-nix-wayland.c
    // runs them: store the capabilities, release, take; or release,
    // release the seat, erase, take.
    let mut s = vec![seat(21, KEYBOARD), seat(22, POINTER | KEYBOARD)];
    let mut keyboard = pick(&s);
    assert_eq!(keyboard, Some(0));

    // Seat 21 keeps its keyboard: nothing changes.
    assert!(!(seats::lacks_keyboard(KEYBOARD) && keyboard == Some(0)));

    // Seat 22 loses its keyboard: not the keyboard's seat.
    s[1].caps = POINTER;
    assert!(!(seats::lacks_keyboard(s[1].caps) && keyboard == Some(1)));
    s[1].caps = POINTER | KEYBOARD;

    // Seat 21 loses it: released, and seat 22 takes over.
    s[0].caps = 0;
    assert!(seats::lacks_keyboard(s[0].caps) && keyboard == Some(0));
    keyboard = pick(&s);
    assert_eq!(keyboard, Some(1));

    // Seat 22 is removed: it had the keyboard, and no seat is left with
    // one.
    let gone = find(&s, 22);
    assert_eq!(gone, Some(1));
    assert_eq!(gone, keyboard);
    s.pop();
    assert_eq!(pick(&s), None);

    // Seat 21 gets a keyboard again and takes it.
    s[0].caps = KEYBOARD | TOUCH;
    assert!(!seats::lacks_keyboard(s[0].caps));
    assert_eq!(pick(&s), Some(0));
}

// Edges past the C test.

#[test]
fn constants_match_wayland() {
    // <wayland-client-protocol.h>: WL_SEAT_CAPABILITY_KEYBOARD and
    // WL_SEAT_RELEASE_SINCE_VERSION; obs-nix-wayland.c checks the C side.
    assert_eq!(KEYBOARD, 2);
    assert_eq!(RELEASE_SINCE_VERSION, 5);
    assert_eq!((MIN_VERSION, MAX_VERSION), (4, 7));
    assert_eq!(seats::bind_version(MIN_VERSION - 1), None);
    assert_eq!(seats::bind_version(MIN_VERSION), Some(MIN_VERSION));
    assert_eq!(seats::bind_version(MAX_VERSION + 1), Some(MAX_VERSION));
}

#[test]
fn every_bound_version_is_in_range() {
    for offered in (0..64).chain([u32::MAX - 1, u32::MAX]) {
        if let Some(v) = seats::bind_version(offered) {
            assert!((MIN_VERSION..=MAX_VERSION).contains(&v), "{offered} -> {v}");
            assert!(v <= offered);
        }
    }
}

#[test]
fn searches_stop_at_the_seat_they_find() {
    // The C loops read each seat up to the one they return, and none
    // after it: so do the iterators.
    let reads = Cell::new(0usize);
    let caps = [POINTER, TOUCH, KEYBOARD, KEYBOARD, 0];
    let found = seats::pick(caps.iter().map(|&c| {
        reads.set(reads.get() + 1);
        c
    }));
    assert_eq!((found, reads.get()), (Some(2), 3));

    reads.set(0);
    let names = [4, 9, 9, 1];
    let found = seats::find_name(
        names.iter().map(|&n| {
            reads.set(reads.get() + 1);
            n
        }),
        9,
    );
    assert_eq!((found, reads.get()), (Some(1), 2));

    // A miss reads every seat once.
    reads.set(0);
    let found = seats::find_name(
        names.iter().map(|&n| {
            reads.set(reads.get() + 1);
            n
        }),
        5,
    );
    assert_eq!((found, reads.get()), (None, 4));
}

fn model_bind(offered: u32) -> u32 {
    if offered < 4 {
        0
    } else if offered <= 7 {
        offered
    } else {
        7
    }
}

fn model_pick(caps: &[u32]) -> Option<usize> {
    (0..caps.len()).find(|&i| caps[i] & 2 != 0)
}

fn model_find(names: &[u32], name: u32) -> Option<usize> {
    (0..names.len()).find(|&i| names[i] == name)
}

fn caps() -> impl Strategy<Value = u32> {
    prop_oneof![0u32..8, any::<u32>()]
}

fn small_names() -> impl Strategy<Value = Vec<u32>> {
    prop::collection::vec(prop_oneof![0u32..6, any::<u32>()], 0..12)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(256)))]

    #[test]
    fn prop_bind_version(offered in prop_oneof![0u32..16, any::<u32>()]) {
        prop_assert_eq!(bind(offered), model_bind(offered));
    }

    #[test]
    fn prop_pick(c in prop::collection::vec(caps(), 0..12)) {
        prop_assert_eq!(seats::pick(c.iter().copied()), model_pick(&c));
    }

    #[test]
    fn prop_lacks_keyboard(c in caps()) {
        prop_assert_eq!(seats::lacks_keyboard(c), c & 2 == 0);
    }

    #[test]
    fn prop_find_name(names in small_names(), name in prop_oneof![0u32..6, any::<u32>()]) {
        prop_assert_eq!(seats::find_name(names.iter().copied(), name), model_find(&names, name));
    }

    #[test]
    fn prop_uses_release(version in prop_oneof![0u32..16, any::<u32>()]) {
        prop_assert_eq!(seats::uses_release(version), version >= 5);
    }
}
