//! Tier 3 parity: the shim against the original
//! `libobs/obs-nix-wayland-key-names.c.h` (oracle/nix_wayland_key_names.c).
//!
//! `test_nix_wayland_key_names_steps` compiles the C into itself, so it never
//! sees the Rust swap (a variant linked against `libobs_rust-static` does not
//! link on its own, as for `test_nix_x11_hotkeys_steps`). This file is what
//! compares the two.
//!
//! - The values: `OBS_KEY_LAST_VALUE` and the route enum (the C test's
//!   `route_values`), against the codes the shim returns.
//! - The route, exhaustive over the key table, `0 .. OBS_KEY_LAST_VALUE`:
//!   the route, the number and the default name (as strings), and which
//!   outputs each side left alone.
//!
//! - A null `number` or `name`: for every key the shim returns the C's
//!   route and writes nothing.
//!
//! Intentional differences, checked on their own and excluded from the
//! comparison (`rust/README.md` lists them too):
//! - a null out-parameter is not written; the C dereferences it, so the C
//!   side is never called with one;
//! - a value outside `0 .. OBS_KEY_LAST_VALUE - 1` gets `NONE` from the
//!   shim, where the C gives `KEYSYM` (its caller then indexed the keycode
//!   table with it): `values_outside_the_table` pins both sides.
//!
//! Reminder: the mutation check (break the core, see this file fail) is done
//! once per port, not on every change.

use std::ffi::{CStr, c_char, c_int};
use std::ptr;

use obs_c_oracle::nix_wayland_key_names::{
    VALUE_NAMES, oracle_obs_nix_wayland_key_names_route, oracle_wayland_key_names_value,
};
use obs_hotkey::ffi::nix_wayland_key_names::obs_nix_wayland_key_names_route;
use obs_hotkey::name_map::{self as k, OBS_KEY_LAST_VALUE};

const LAST: c_int = OBS_KEY_LAST_VALUE as c_int;

/// Not a value the function writes: shows what it left alone.
const UNSET_NUMBER: c_int = -12345;

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

#[test]
fn values_match_the_c() {
    // The shim's code for a key of each route; NONE for a value outside the table.
    let code = |key| named(obs_nix_wayland_key_names_route, key).0;
    let rust: [i64; 7] = [
        i64::try_from(OBS_KEY_LAST_VALUE).unwrap(),
        i64::from(code(-1)),
        i64::from(code(k::OBS_KEY_MOUSE1)),
        i64::from(code(k::OBS_KEY_NUM0)),
        i64::from(code(k::OBS_KEY_INSERT)),
        i64::from(code(k::OBS_KEY_F1)),
        i64::from(code(k::OBS_KEY_A)),
    ];
    for (which, (name, want)) in VALUE_NAMES.iter().zip(rust).enumerate() {
        // SAFETY: a plain getter.
        let c = unsafe { oracle_wayland_key_names_value(which) };
        assert_eq!(c, want, "{name}");
    }
}

#[test]
fn every_route_matches() {
    let mut compared = 0;
    for key in 0..LAST {
        let rust = named(obs_nix_wayland_key_names_route, key);
        let c = named(oracle_obs_nix_wayland_key_names_route, key);
        assert_eq!(rust, c, "route of {key}");
        compared += 1;
    }
    assert_eq!(compared, LAST);
}

/// The intentional difference: outside the table the C routes to the keysym
/// and writes nothing; the shim gives `NONE` and writes nothing either.
#[test]
fn values_outside_the_table() {
    let keysym = named(oracle_obs_nix_wayland_key_names_route, k::OBS_KEY_A).0;
    for key in [c_int::MIN, -2, -1, LAST, LAST + 1, c_int::MAX] {
        assert_eq!(
            named(oracle_obs_nix_wayland_key_names_route, key),
            (keysym, UNSET_NUMBER, None),
            "C route of {key}"
        );
        assert_eq!(
            named(obs_nix_wayland_key_names_route, key),
            (0, UNSET_NUMBER, None),
            "Rust route of {key}"
        );
    }
}

#[test]
fn null_outputs_are_not_written() {
    for key in 0..LAST {
        let (c, _, _) = named(oracle_obs_nix_wayland_key_names_route, key);
        // SAFETY: the shim takes null out-parameters (the intentional
        // difference); the C side gets writable locals above.
        let rust =
            unsafe { obs_nix_wayland_key_names_route(key, ptr::null_mut(), ptr::null_mut()) };
        assert_eq!(rust, c, "route of {key}");
    }
}
