//! Tier 3 parity: the shims against the original
//! `libobs/obs-nix-x11-hotkeys.c.h` (oracle/nix_x11_hotkeys.c).
//!
//! `test_nix_x11_hotkeys_steps` compiles the C into itself, so it never sees
//! the Rust swap (a variant linked against `libobs_rust-static` does not
//! link on its own, as for `test_nix_x11_input_steps`). This file is what
//! compares the two.
//!
//! - The values: `OBS_KEY_LAST_VALUE`, `XINPUT_MOUSE_LEN` and the three
//!   event codes.
//! - The route, exhaustive over `-2 .. OBS_KEY_LAST_VALUE + 2` and the `int`
//!   extremes: the route, the number and the default name (as strings),
//!   and which outputs each side left alone.
//! - The decode, exhaustive over every response type, event types 0 to 31
//!   and 0xffff, and details 0 to 255 and a few large ones: the result and
//!   the slot (or that it was left alone). Where the C reads nothing more,
//!   both sides get null pointers for the event type and the detail.
//!
//! Intentional differences, excluded here and checked on their own, exactly
//! those inputs and no more (`rust/README.md` lists them too):
//! - a value outside `0 .. OBS_KEY_LAST_VALUE - 1`: the C routes it to the
//!   keycodes (which `obs_nix_x11_key_to_str()` then indexes with it); the
//!   port gives no route;
//! - response types 51, 99, 115 and 163 (0xa3): the C clears decimal 80
//!   (0x50) to find a generic event, so it takes the first three and not
//!   35 with the send-event bit; the port clears the send-event bit (0x80);
//! - button 0 in a raw press or release: the C passes it at slot `0 - 1`
//!   (`UINT32_MAX`); the port calls it unsupported.
//!
//! Reminder: the mutation check (break the core, see this file fail) is done
//! once per port, not on every change.

use std::ffi::{CStr, c_char, c_int};
use std::ptr;

use obs_c_oracle::nix_x11_hotkeys::{
    VALUE_NAMES, oracle_obs_nix_x11_hotkeys_raw_button, oracle_obs_nix_x11_hotkeys_route,
    oracle_x11_hotkeys_value,
};
use obs_hotkey::ffi::nix_x11_hotkeys::{obs_nix_x11_hotkeys_raw_button, obs_nix_x11_hotkeys_route};
use obs_hotkey::name_map::OBS_KEY_LAST_VALUE;
use obs_hotkey::nix_x11_hotkeys::{
    GE_GENERIC, MOUSE_LEN, RAW_BUTTON_PRESS, RAW_BUTTON_RELEASE, SEND_EVENT,
};

const LAST: c_int = OBS_KEY_LAST_VALUE as c_int;

/// `enum obs_nix_x11_hotkeys_route` and `enum obs_nix_x11_hotkeys_button`.
const ROUTE_NONE: c_int = 0;
const ROUTE_KEYCODES: c_int = 5;
const BUTTON_PRESS: c_int = 1;
const BUTTON_RELEASE: c_int = 2;
const BUTTON_UNSUPPORTED: c_int = 3;

/// Not values the functions write: show what they left alone.
const UNSET_NUMBER: c_int = -12345;
const UNSET_SLOT: u32 = 0xdead_beef;

/// A route as one side gave it: the result, the number (or `UNSET_NUMBER`)
/// and the default name (`None` if left alone).
type Named = (c_int, c_int, Option<Vec<u8>>);

fn named(
    f: unsafe extern "C" fn(c_int, *mut c_int, *mut *const c_char) -> c_int,
    key: c_int,
) -> Named {
    let mut number = UNSET_NUMBER;
    let mut name: *const c_char = ptr::null();
    // SAFETY: both out-parameters are writable locals.
    let route = unsafe { f(key, &raw mut number, &raw mut name) };
    let name = (!name.is_null()).then(|| {
        // SAFETY: a set name is a static NUL-terminated string.
        unsafe { CStr::from_ptr(name) }.to_bytes().to_vec()
    });
    (route, number, name)
}

/// A decode as one side gave it: the result and the slot (or `UNSET_SLOT`).
fn decoded(
    f: unsafe extern "C" fn(u8, *const u16, *const u32, *mut u32) -> c_int,
    response_type: u8,
    event_type: Option<u16>,
    detail: Option<u32>,
) -> (c_int, u32) {
    let mut slot = UNSET_SLOT;
    let event_type = event_type.as_ref().map_or(ptr::null(), ptr::from_ref);
    let detail = detail.as_ref().map_or(ptr::null(), ptr::from_ref);
    // SAFETY: each pointer is null or points to a live local.
    let button = unsafe { f(response_type, event_type, detail, &raw mut slot) };
    (button, slot)
}

/// Whether the C takes `response_type` for a generic event.
fn c_generic(response_type: u8) -> bool {
    response_type & !80 == GE_GENERIC
}

/// Whether the port does.
fn rust_generic(response_type: u8) -> bool {
    response_type & !SEND_EVENT == GE_GENERIC
}

#[test]
fn values_match_the_c() {
    let rust: [i64; 5] = [
        i64::try_from(OBS_KEY_LAST_VALUE).unwrap(),
        i64::try_from(MOUSE_LEN).unwrap(),
        i64::from(GE_GENERIC),
        i64::from(RAW_BUTTON_PRESS),
        i64::from(RAW_BUTTON_RELEASE),
    ];
    for (which, (name, want)) in VALUE_NAMES.iter().zip(rust).enumerate() {
        // SAFETY: a plain getter.
        let c = unsafe { oracle_x11_hotkeys_value(which) };
        assert_eq!(c, want, "{name}");
    }
}

#[test]
fn every_route_matches() {
    let mut compared = 0;
    for key in (-2..LAST + 2).chain([c_int::MIN, c_int::MAX]) {
        let rust = named(obs_nix_x11_hotkeys_route, key);
        let c = named(oracle_obs_nix_x11_hotkeys_route, key);
        if (0..LAST).contains(&key) {
            assert_eq!(rust, c, "route of {key}");
            compared += 1;
        } else {
            // Intentional difference: no route outside the key table.
            assert_eq!(c, (ROUTE_KEYCODES, UNSET_NUMBER, None), "C route of {key}");
            assert_eq!(rust, (ROUTE_NONE, UNSET_NUMBER, None), "route of {key}");
        }
    }
    assert_eq!(compared, LAST);
}

#[test]
fn every_decode_matches() {
    let event_types = (0..32).chain([0xffff]);
    let details: Vec<u32> = (0..256).chain([0x7fff_ffff, 0xffff_ffff]).collect();
    let (mut compared, mut excluded) = (0u64, 0u64);
    for response_type in 0..=u8::MAX {
        let generic = (c_generic(response_type), rust_generic(response_type));
        if generic != (true, true) {
            // Neither side reads further: null pointers on both.
            let rust = decoded(obs_nix_x11_hotkeys_raw_button, response_type, None, None);
            if generic == (false, false) {
                let c = decoded(
                    oracle_obs_nix_x11_hotkeys_raw_button,
                    response_type,
                    None,
                    None,
                );
                assert_eq!(rust, c, "response type {response_type}");
                compared += 1;
                continue;
            }
            // Intentional difference: one side takes it for a generic event.
            assert!(
                matches!(response_type, 51 | 99 | 115 | 163),
                "{response_type}"
            );
            excluded += 1;
            if generic.1 {
                // 0xa3: the port decodes it like 35.
                for event_type in [RAW_BUTTON_PRESS, RAW_BUTTON_RELEASE, 14] {
                    let full = decoded(
                        obs_nix_x11_hotkeys_raw_button,
                        response_type,
                        Some(event_type),
                        Some(3),
                    );
                    let as_35 = decoded(
                        oracle_obs_nix_x11_hotkeys_raw_button,
                        35,
                        Some(event_type),
                        Some(3),
                    );
                    assert_eq!(full, as_35, "0xa3 with type {event_type}");
                }
            } else {
                assert_eq!(rust, (0, UNSET_SLOT), "response type {response_type}");
            }
            continue;
        }
        for event_type in event_types.clone() {
            if event_type != RAW_BUTTON_PRESS && event_type != RAW_BUTTON_RELEASE {
                // The detail is not read: a null pointer on both sides.
                let rust = decoded(
                    obs_nix_x11_hotkeys_raw_button,
                    response_type,
                    Some(event_type),
                    None,
                );
                let c = decoded(
                    oracle_obs_nix_x11_hotkeys_raw_button,
                    response_type,
                    Some(event_type),
                    None,
                );
                assert_eq!(rust, c, "response type {response_type}, type {event_type}");
                compared += 1;
                continue;
            }
            for &detail in &details {
                let rust = decoded(
                    obs_nix_x11_hotkeys_raw_button,
                    response_type,
                    Some(event_type),
                    Some(detail),
                );
                let c = decoded(
                    oracle_obs_nix_x11_hotkeys_raw_button,
                    response_type,
                    Some(event_type),
                    Some(detail),
                );
                if detail == 0 {
                    // Intentional difference: button 0 is unsupported.
                    let want = if event_type == RAW_BUTTON_PRESS {
                        BUTTON_PRESS
                    } else {
                        BUTTON_RELEASE
                    };
                    assert_eq!(c, (want, u32::MAX), "C, button 0, type {event_type}");
                    assert_eq!(rust, (BUTTON_UNSUPPORTED, UNSET_SLOT), "button 0");
                    excluded += 1;
                } else {
                    assert_eq!(
                        rust, c,
                        "{response_type}, type {event_type}, button {detail}"
                    );
                    compared += 1;
                }
            }
        }
    }
    // Response types: 35 generic on both sides, four on one side only, 251
    // on neither. For 35: 31 other event types, and a press and a release
    // of 258 buttons each, button 0 excluded.
    assert_eq!(excluded, 4 + 2);
    assert_eq!(compared, 251 + 31 + 2 * 257);
}
