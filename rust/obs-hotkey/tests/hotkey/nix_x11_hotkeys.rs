//! Tier 1: the safe core of `libobs/obs-nix-x11-hotkeys.c.h`.
//!
//! The first twelve tests port `test/cmocka/test_nix_x11_hotkeys_steps.c`
//! one to one, with the same names, inputs and expectations, in the same
//! order, except where the port fixes the C on purpose (each says so):
//! - a value outside the key table has no route (C sends it to the
//!   keycodes, which it indexes with it);
//! - button 0 is unsupported (C stores it at slot `0 - 1`, wrapped);
//! - the generic event test clears the send-event bit, 0x80 (C clears
//!   decimal 80, 0x50).
//!
//! The cases after those pin the edges of each function, and the proptests
//! check each one against a model written from the C, with those three
//! fixes.

use std::cell::Cell;
use std::ffi::CStr;

use obs_hotkey::name_map::{self as k, OBS_KEY_LAST_VALUE};
use obs_hotkey::nix_x11_hotkeys::{
    self as xh, GE_GENERIC, MOUSE_LEN, RAW_BUTTON_PRESS, RAW_BUTTON_RELEASE, RawButton, Route,
    SEND_EVENT,
};
use proptest::prelude::*;

const LAST: i32 = OBS_KEY_LAST_VALUE as i32;

/// The 30 fixed keys and their default names, in the C's order.
const FIXED: [(i32, &CStr); 30] = [
    (k::OBS_KEY_INSERT, c"Insert"),
    (k::OBS_KEY_DELETE, c"Delete"),
    (k::OBS_KEY_HOME, c"Home"),
    (k::OBS_KEY_END, c"End"),
    (k::OBS_KEY_PAGEUP, c"Page Up"),
    (k::OBS_KEY_PAGEDOWN, c"Page Down"),
    (k::OBS_KEY_NUMLOCK, c"Num Lock"),
    (k::OBS_KEY_SCROLLLOCK, c"Scroll Lock"),
    (k::OBS_KEY_CAPSLOCK, c"Caps Lock"),
    (k::OBS_KEY_BACKSPACE, c"Backspace"),
    (k::OBS_KEY_TAB, c"Tab"),
    (k::OBS_KEY_PRINT, c"Print"),
    (k::OBS_KEY_PAUSE, c"Pause"),
    (k::OBS_KEY_LEFT, c"Left"),
    (k::OBS_KEY_RIGHT, c"Right"),
    (k::OBS_KEY_UP, c"Up"),
    (k::OBS_KEY_DOWN, c"Down"),
    (k::OBS_KEY_SHIFT, c"Shift"),
    (k::OBS_KEY_ALT, c"Alt"),
    (k::OBS_KEY_CONTROL, c"Control"),
    (k::OBS_KEY_META, c"Super"),
    (k::OBS_KEY_MENU, c"Menu"),
    (k::OBS_KEY_NUMASTERISK, c"Numpad *"),
    (k::OBS_KEY_NUMPLUS, c"Numpad +"),
    (k::OBS_KEY_NUMMINUS, c"Numpad -"),
    (k::OBS_KEY_NUMCOMMA, c"Numpad ,"),
    (k::OBS_KEY_NUMPERIOD, c"Numpad ."),
    (k::OBS_KEY_NUMSLASH, c"Numpad /"),
    (k::OBS_KEY_SPACE, c"Space"),
    (k::OBS_KEY_ESCAPE, c"Escape"),
];

/// The decode of an event with all three fields, as the C reads them.
fn decode(response_type: u8, event_type: u16, detail: u32) -> RawButton {
    xh::raw_button(response_type, || event_type, || detail)
}

/// The decode of an event whose event type and detail must not be read.
fn decode_unread(response_type: u8) -> RawButton {
    xh::raw_button(
        response_type,
        || panic!("event type read"),
        || panic!("detail read"),
    )
}

/// The decode of a generic event whose detail must not be read.
fn decode_type_only(event_type: u16) -> RawButton {
    xh::raw_button(GE_GENERIC, || event_type, || panic!("detail read"))
}

/// Mirrors `route_mouse`: Mouse 1 to 29, numbered from 1.
#[test]
fn route_mouse() {
    for n in 1..=29 {
        assert_eq!(xh::route(k::OBS_KEY_MOUSE1 + n - 1), Route::Mouse(n));
    }
    assert_eq!(xh::route(k::OBS_KEY_MOUSE29), Route::Mouse(29));
}

/// Mirrors `route_numpad`: Numpad 0 to 9, numbered from 0.
#[test]
fn route_numpad() {
    for n in 0..=9 {
        assert_eq!(xh::route(k::OBS_KEY_NUM0 + n), Route::Numpad(n));
    }
    assert_eq!(xh::route(k::OBS_KEY_NUM9), Route::Numpad(9));
}

/// Mirrors `route_fixed`: the 30 keys with a default name, Meta named
/// "Super".
#[test]
fn route_fixed() {
    for (key, name) in FIXED {
        assert_eq!(xh::route(key), Route::Fixed(name), "key {key}");
    }
}

/// Mirrors `route_function`: F1 to F35 by number, with no translation
/// lookup (characterized, not endorsed: every other named key has one).
#[test]
fn route_function() {
    assert_eq!(xh::route(k::OBS_KEY_F1), Route::Function(1));
    assert_eq!(xh::route(k::OBS_KEY_F12), Route::Function(12));
    assert_eq!(xh::route(k::OBS_KEY_F24), Route::Function(24));
    assert_eq!(xh::route(k::OBS_KEY_F35), Route::Function(35));
}

/// Mirrors `route_keycodes`: every other key goes to its keycodes,
/// `OBS_KEY_NONE` too.
#[test]
fn route_keycodes() {
    for key in [
        k::OBS_KEY_NONE,
        k::OBS_KEY_A,
        k::OBS_KEY_1,
        k::OBS_KEY_RETURN,
        k::OBS_KEY_ENTER,
        k::OBS_KEY_NUMEQUAL,
        k::OBS_KEY_VK_VOLUME_UP,
        LAST - 1,
    ] {
        assert_eq!(xh::route(key), Route::Keycodes, "key {key}");
    }
}

/// Mirrors `route_outside`. Intentional difference: a value outside the
/// key table has no route, so the name is left as it is (C sends it to the
/// keycodes, which it indexes with the value).
#[test]
fn route_outside() {
    assert_eq!(xh::route(-1), Route::None);
    assert_eq!(xh::route(LAST), Route::None);
    assert_eq!(xh::route(LAST + 1), Route::None);
}

/// Mirrors `raw_press_release`: a raw press or release of buttons 1 to 32
/// sets slots 0 to 31.
#[test]
fn raw_press_release() {
    for d in 1..u32::try_from(MOUSE_LEN).unwrap() {
        let slot = usize::try_from(d - 1).unwrap();
        assert_eq!(
            decode(GE_GENERIC, RAW_BUTTON_PRESS, d),
            RawButton::Press(slot)
        );
        assert_eq!(
            decode(GE_GENERIC, RAW_BUTTON_RELEASE, d),
            RawButton::Release(slot)
        );
    }
    assert_eq!(
        decode(GE_GENERIC, RAW_BUTTON_PRESS, 32),
        RawButton::Press(31)
    );
}

/// Mirrors `raw_unsupported`: button 33 and up are unsupported.
#[test]
fn raw_unsupported() {
    for d in [33, 34, 255, 256, 0x7fff_ffff, 0xffff_ffff] {
        assert_eq!(
            decode(GE_GENERIC, RAW_BUTTON_PRESS, d),
            RawButton::Unsupported
        );
        assert_eq!(
            decode(GE_GENERIC, RAW_BUTTON_RELEASE, d),
            RawButton::Unsupported
        );
    }
}

/// Mirrors `raw_button_zero`. Intentional difference: button 0 is
/// unsupported (C stores it at slot `0 - 1`, wrapped to `UINT32_MAX`).
#[test]
fn raw_button_zero() {
    assert_eq!(
        decode(GE_GENERIC, RAW_BUTTON_PRESS, 0),
        RawButton::Unsupported
    );
    assert_eq!(
        decode(GE_GENERIC, RAW_BUTTON_RELEASE, 0),
        RawButton::Unsupported
    );
}

/// Mirrors `raw_other_types`: another XInput event type is ignored, and its
/// detail not read.
#[test]
fn raw_other_types() {
    for t in [0, 1, 13, 14, 17, 18, 31, 0x10f, 0xffff] {
        assert_eq!(decode_type_only(t), RawButton::Ignore, "type {t}");
    }
}

/// Mirrors `raw_generic_mask`. Intentional difference: the generic event
/// test clears only the send-event bit, so 35 with it (0xa3) is generic,
/// and 51, 99 and 115 are not (C clears decimal 80, 0x50, so it takes those
/// three and not 0xa3).
#[test]
fn raw_generic_mask() {
    assert_eq!(decode(35, RAW_BUTTON_PRESS, 3), RawButton::Press(2));
    assert_eq!(
        decode(35 | SEND_EVENT, RAW_BUTTON_PRESS, 3),
        RawButton::Press(2)
    );
    for t in [51, 99, 115] {
        assert_eq!(decode_unread(t), RawButton::Ignore, "type {t}");
    }

    // Not generic: the event type is not read.
    for t in [0, 2, 3, 4, 5, 34, 36, 0xff, 0x80 | 51] {
        assert_eq!(decode_unread(t), RawButton::Ignore, "type {t}");
    }
}

/// Mirrors `event_codes`: the event codes are X's.
#[test]
fn event_codes() {
    assert_eq!(GE_GENERIC, 35);
    assert_eq!(RAW_BUTTON_PRESS, 15);
    assert_eq!(RAW_BUTTON_RELEASE, 16);
    assert_eq!(MOUSE_LEN, 33);
    assert_eq!(SEND_EVENT, 0x80);
}

/// Every key has one route, and the routes cover exactly the C's ranges:
/// 29 mouse buttons, 10 numpad digits, 30 fixed keys, 35 function keys.
#[test]
fn route_counts() {
    let (mut mouse, mut numpad, mut fixed, mut function, mut keycodes) = (0, 0, 0, 0, 0);
    for key in 0..LAST {
        match xh::route(key) {
            Route::Mouse(_) => mouse += 1,
            Route::Numpad(_) => numpad += 1,
            Route::Fixed(_) => fixed += 1,
            Route::Function(_) => function += 1,
            Route::Keycodes => keycodes += 1,
            Route::None => panic!("key {key} has no route"),
        }
    }
    assert_eq!((mouse, numpad, fixed, function), (29, 10, 30, 35));
    assert_eq!(keycodes, LAST - 104);
}

/// The keys next to each range route elsewhere.
#[test]
fn route_range_edges() {
    assert_eq!(xh::route(k::OBS_KEY_MOUSE1 - 1), Route::Keycodes);
    assert_eq!(xh::route(k::OBS_KEY_NUM0 - 1), Route::Fixed(c"Numpad /"));
    assert_eq!(xh::route(k::OBS_KEY_NUM9 + 1), Route::Keycodes);
    assert_eq!(xh::route(k::OBS_KEY_F1 - 1), Route::Fixed(c"Scroll Lock"));
    assert_eq!(xh::route(k::OBS_KEY_F35 + 1), Route::Fixed(c"Menu"));
    assert_eq!(xh::route(i32::MIN), Route::None);
    assert_eq!(xh::route(i32::MAX), Route::None);
}

/// Each argument is read at most once, the event type only for a generic
/// event and the detail only for a raw press or release.
#[test]
fn raw_reads() {
    let types = Cell::new(0);
    let details = Cell::new(0);
    let run = |response_type: u8, event_type: u16| {
        types.set(0);
        details.set(0);
        let button = xh::raw_button(
            response_type,
            || {
                types.set(types.get() + 1);
                event_type
            },
            || {
                details.set(details.get() + 1);
                5
            },
        );
        (button, types.get(), details.get())
    };
    assert_eq!(run(2, RAW_BUTTON_PRESS), (RawButton::Ignore, 0, 0));
    assert_eq!(run(GE_GENERIC, 14), (RawButton::Ignore, 1, 0));
    assert_eq!(
        run(GE_GENERIC, RAW_BUTTON_PRESS),
        (RawButton::Press(4), 1, 1)
    );
    assert_eq!(
        run(GE_GENERIC | SEND_EVENT, RAW_BUTTON_RELEASE),
        (RawButton::Release(4), 1, 1)
    );
}

/// The model of `route`, written from the C, with the fix for keys outside
/// the table.
fn route_model(key: i32) -> Route {
    if !(0..LAST).contains(&key) {
        return Route::None;
    }
    if (k::OBS_KEY_MOUSE1..=k::OBS_KEY_MOUSE29).contains(&key) {
        return Route::Mouse(key - k::OBS_KEY_MOUSE1 + 1);
    }
    if (k::OBS_KEY_NUM0..=k::OBS_KEY_NUM9).contains(&key) {
        return Route::Numpad(key - k::OBS_KEY_NUM0);
    }
    if let Some(&(_, name)) = FIXED.iter().find(|(f, _)| *f == key) {
        return Route::Fixed(name);
    }
    if (k::OBS_KEY_F1..=k::OBS_KEY_F35).contains(&key) {
        return Route::Function(key - k::OBS_KEY_F1 + 1);
    }
    Route::Keycodes
}

/// The model of `raw_button`, written from the C, with the fixes for the
/// send-event bit and button 0.
fn raw_model(response_type: u8, event_type: u16, detail: u32) -> RawButton {
    if response_type & !SEND_EVENT != GE_GENERIC {
        return RawButton::Ignore;
    }
    let pressed = match event_type {
        RAW_BUTTON_PRESS => true,
        RAW_BUTTON_RELEASE => false,
        _ => return RawButton::Ignore,
    };
    match usize::try_from(detail) {
        Ok(d @ 1..MOUSE_LEN) if pressed => RawButton::Press(d - 1),
        Ok(d @ 1..MOUSE_LEN) => RawButton::Release(d - 1),
        _ => RawButton::Unsupported,
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(256)))]

    #[test]
    fn route_matches_model(key in prop_oneof![-4..LAST + 4, any::<i32>()]) {
        prop_assert_eq!(xh::route(key), route_model(key));
    }

    #[test]
    fn raw_button_matches_model(
        response_type in any::<u8>(),
        event_type in prop_oneof![14u16..18, any::<u16>()],
        detail in prop_oneof![0u32..40, any::<u32>()],
    ) {
        prop_assert_eq!(
            decode(response_type, event_type, detail),
            raw_model(response_type, event_type, detail)
        );
    }
}
