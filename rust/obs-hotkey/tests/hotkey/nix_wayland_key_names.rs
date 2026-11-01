//! Tier 1: the safe core of `libobs/obs-nix-wayland-key-names.c.h`.
//!
//! The first six tests port `test/cmocka/test_nix_wayland_key_names_steps.c`
//! one to one, with the same names, inputs and expectations, in the same
//! order. Its seventh, `route_values`, pins the C enum, which only the shim
//! sees: the parity test checks it. The cases after those check the route against the X11 platform's,
//! whose table it shares, and the proptest checks it against a model
//! written from the C, over the key table.

use std::ffi::CStr;

use obs_hotkey::name_map::{self as k, OBS_KEY_LAST_VALUE};
use obs_hotkey::nix_wayland_key_names::{self as wk, Route};
use obs_hotkey::nix_x11_hotkeys::{self as xh, Route as X11Route};
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

/// The route the C gives `key`, written from it.
fn route_model(key: i32) -> Route {
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
    Route::Keysym
}

/// Mouse 1 to 29, numbered from 1.
#[test]
fn route_mouse() {
    for n in 1..=29 {
        assert_eq!(wk::route(k::OBS_KEY_MOUSE1 + n - 1), Route::Mouse(n));
    }
    assert_eq!(wk::route(k::OBS_KEY_MOUSE29), Route::Mouse(29));
}

/// Numpad 0 to 9, numbered from 0.
#[test]
fn route_numpad() {
    for n in 0..=9 {
        assert_eq!(wk::route(k::OBS_KEY_NUM0 + n), Route::Numpad(n));
    }
    assert_eq!(wk::route(k::OBS_KEY_NUM9), Route::Numpad(9));
}

/// The 30 keys with a default name, Meta named "Super".
#[test]
fn route_fixed() {
    assert_eq!(FIXED.len(), 30);
    for (key, name) in FIXED {
        assert_eq!(wk::route(key), Route::Fixed(name), "key {key}");
    }
}

/// F1 to F35 by number, with no translation lookup (characterized, not
/// endorsed: every other named key has one).
#[test]
fn route_function() {
    for n in 1..=35 {
        assert_eq!(wk::route(k::OBS_KEY_F1 + n - 1), Route::Function(n));
    }
}

/// Every other key goes to its keysym, `OBS_KEY_NONE` too.
#[test]
fn route_keysym() {
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
        assert_eq!(wk::route(key), Route::Keysym, "key {key}");
    }
}

/// Every key of the table: the route of each, by the ranges and the fixed
/// list, and how many take each.
#[test]
fn route_every_key() {
    let (mut mouse, mut numpad, mut fixed, mut function, mut keysym) = (0, 0, 0, 0, 0);
    for key in 0..LAST {
        let route = wk::route(key);
        assert_eq!(route, route_model(key), "key {key}");
        match route {
            Route::Mouse(_) => mouse += 1,
            Route::Numpad(_) => numpad += 1,
            Route::Fixed(_) => fixed += 1,
            Route::Function(_) => function += 1,
            Route::Keysym => keysym += 1,
        }
    }
    assert_eq!((mouse, numpad, fixed, function), (29, 10, 30, 35));
    assert_eq!(keysym, LAST - 29 - 10 - 30 - 35);
}

/// The X11 platform names every key of the table the same way, with its
/// keycodes where Wayland uses the keysym.
#[test]
fn route_matches_x11() {
    for key in 0..LAST {
        let x11 = match xh::route(key) {
            X11Route::Mouse(n) => Route::Mouse(n),
            X11Route::Numpad(n) => Route::Numpad(n),
            X11Route::Fixed(name) => Route::Fixed(name),
            X11Route::Function(n) => Route::Function(n),
            X11Route::Keycodes => Route::Keysym,
            X11Route::None => panic!("no X11 route for key {key}"),
        };
        assert_eq!(wk::route(key), x11, "key {key}");
    }
}

/// The edges of each range route to the next test down the C's order.
#[test]
fn route_range_edges() {
    for key in [
        k::OBS_KEY_MOUSE1 - 1,
        k::OBS_KEY_MOUSE29 + 1,
        k::OBS_KEY_NUM0 - 1,
        k::OBS_KEY_NUM9 + 1,
        k::OBS_KEY_F1 - 1,
        k::OBS_KEY_F35 + 1,
    ] {
        assert_eq!(wk::route(key), route_model(key), "key {key}");
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(256)))]

    #[test]
    fn route_matches_model(key in 0..LAST) {
        prop_assert_eq!(wk::route(key), route_model(key));
    }
}
