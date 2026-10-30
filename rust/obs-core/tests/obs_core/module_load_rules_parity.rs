//! Tier 3: the module load rule shims against the original C.
//!
//! The oracle is `libobs/obs-module-load-rules.c.h`, unmodified, with its
//! entry points renamed to `oracle_*` (`obs-c-oracle/oracle/
//! module_load_rules.c`). The same file gives the constants of
//! `obs-module-load-rules.h`, `obs-defs.h` and `obs.h`, and
//! `LIBOBS_API_VER`. Compared, as the C returns them (the list enum and
//! the route bits as `u32`, the state through the pointer):
//!
//! - the constants, against the Rust ones, the C headers' and the shims'
//!   codes;
//! - every function over every bool it takes;
//! - `_open_route()` over every code from -64 to 64 plus the `i32` ends,
//!   and `_is_core()`/`_is_legacy()` over the module types 0 to 15 and eight
//!   wide ones;
//! - `_version_word()` and `_version_ok()` at the edges of the libobs
//!   version and the mask, and `_safe_list()`, `_disabled_list()` and
//!   `_core_loaded()` at the `usize` edges;
//! - `load_all_callback()`'s whole decision, each side's functions in the
//!   C's order, for all 512 settings of what it reads by every type and
//!   code above;
//! - proptests over any `u32` version word and libobs version, any `usize`
//!   list length and failure count, any type and any code.
//!
//! Intentional difference: where the C would write the state through a
//! NULL pointer, the shims do not (`obs-module.c` always passes a local).
//! It is not compared with the C; `route_null_state` pins it.

use core::ffi::c_int;
use core::ptr;

use obs_c_oracle::module_load_rules::{
    CONSTANT_NAMES, oracle_module_load_rules_constant, oracle_module_load_rules_constant_count,
    oracle_obs_module_load_rules_allow_disable, oracle_obs_module_load_rules_arg_missing,
    oracle_obs_module_load_rules_core_loaded, oracle_obs_module_load_rules_count_failure,
    oracle_obs_module_load_rules_disabled_list, oracle_obs_module_load_rules_export_missing,
    oracle_obs_module_load_rules_init_route, oracle_obs_module_load_rules_is_core,
    oracle_obs_module_load_rules_is_legacy, oracle_obs_module_load_rules_open_route,
    oracle_obs_module_load_rules_safe_list, oracle_obs_module_load_rules_version_ok,
    oracle_obs_module_load_rules_version_word,
};
use obs_core::ffi::module_load_rules::{
    obs_module_load_rules_allow_disable, obs_module_load_rules_arg_missing,
    obs_module_load_rules_core_loaded, obs_module_load_rules_count_failure,
    obs_module_load_rules_disabled_list, obs_module_load_rules_export_missing,
    obs_module_load_rules_init_route, obs_module_load_rules_is_core,
    obs_module_load_rules_is_legacy, obs_module_load_rules_open_route,
    obs_module_load_rules_safe_list, obs_module_load_rules_version_ok,
    obs_module_load_rules_version_word,
};
use obs_core::module_load_rules::{
    List, MODULE_ERROR, MODULE_FAILED_TO_OPEN, MODULE_HARDCODED_SKIP, MODULE_INCOMPATIBLE_VER,
    MODULE_MISSING_EXPORTS, MODULE_SUCCESS, ROUTE_CORE_CONTINUES, ROUTE_DISABLE, ROUTE_FAIL,
    ROUTE_FREE, ROUTE_KEEP_TYPE, ROUTE_SET_TYPE, ROUTE_STOP, STATE_DISABLED, STATE_DISABLED_SAFE,
    STATE_FAILED_TO_INITIALIZE, STATE_FAILED_TO_OPEN, STATE_INVALID, TYPE_CORE, TYPE_LEGACY_PLUGIN,
    TYPE_PLUGIN, TYPE_UNDEFINED,
};
use proptest::prelude::*;

/// The constants as the C has them, by name.
fn c_constant(name: &str) -> i64 {
    let i = CONSTANT_NAMES
        .iter()
        .position(|n| *n == name)
        .unwrap_or_else(|| panic!("no oracle constant {name}"));
    // SAFETY: plain values.
    unsafe { oracle_module_load_rules_constant(i) }
}

fn api_ver() -> u32 {
    u32::try_from(c_constant("LIBOBS_API_VER")).expect("LIBOBS_API_VER fits in u32")
}

#[test]
fn constants_match_the_c() {
    // SAFETY: plain values.
    assert_eq!(
        unsafe { oracle_module_load_rules_constant_count() },
        CONSTANT_NAMES.len()
    );
    let pairs: [(&str, i64); 34] = [
        ("OBS_MODULE_LOAD_RULES_SUCCESS", MODULE_SUCCESS.into()),
        ("OBS_MODULE_LOAD_RULES_ERROR", MODULE_ERROR.into()),
        (
            "OBS_MODULE_LOAD_RULES_FAILED_TO_OPEN",
            MODULE_FAILED_TO_OPEN.into(),
        ),
        (
            "OBS_MODULE_LOAD_RULES_MISSING_EXPORTS",
            MODULE_MISSING_EXPORTS.into(),
        ),
        (
            "OBS_MODULE_LOAD_RULES_INCOMPATIBLE_VER",
            MODULE_INCOMPATIBLE_VER.into(),
        ),
        (
            "OBS_MODULE_LOAD_RULES_HARDCODED_SKIP",
            MODULE_HARDCODED_SKIP.into(),
        ),
        ("MODULE_SUCCESS", MODULE_SUCCESS.into()),
        ("MODULE_ERROR", MODULE_ERROR.into()),
        ("MODULE_FAILED_TO_OPEN", MODULE_FAILED_TO_OPEN.into()),
        ("MODULE_MISSING_EXPORTS", MODULE_MISSING_EXPORTS.into()),
        ("MODULE_INCOMPATIBLE_VER", MODULE_INCOMPATIBLE_VER.into()),
        ("MODULE_HARDCODED_SKIP", MODULE_HARDCODED_SKIP.into()),
        ("OBS_MODULE_LOAD_RULES_TYPE_CORE", TYPE_CORE.into()),
        (
            "OBS_MODULE_LOAD_RULES_TYPE_LEGACY_PLUGIN",
            TYPE_LEGACY_PLUGIN.into(),
        ),
        ("OBS_MODULE_TYPE_UNDEFINED", TYPE_UNDEFINED.into()),
        ("OBS_MODULE_TYPE_CORE", TYPE_CORE.into()),
        ("OBS_MODULE_TYPE_PLUGIN", TYPE_PLUGIN.into()),
        ("OBS_MODULE_TYPE_LEGACY_PLUGIN", TYPE_LEGACY_PLUGIN.into()),
        ("OBS_MODULE_LOAD_RULES_STATE_INVALID", STATE_INVALID.into()),
        (
            "OBS_MODULE_LOAD_RULES_STATE_FAILED_TO_OPEN",
            STATE_FAILED_TO_OPEN.into(),
        ),
        (
            "OBS_MODULE_LOAD_RULES_STATE_FAILED_TO_INITIALIZE",
            STATE_FAILED_TO_INITIALIZE.into(),
        ),
        ("OBS_MODULE_INVALID", STATE_INVALID.into()),
        ("OBS_MODULE_DISABLED", STATE_DISABLED.into()),
        ("OBS_MODULE_DISABLED_SAFE", STATE_DISABLED_SAFE.into()),
        ("OBS_MODULE_FAILED_TO_OPEN", STATE_FAILED_TO_OPEN.into()),
        (
            "OBS_MODULE_FAILED_TO_INITIALIZE",
            STATE_FAILED_TO_INITIALIZE.into(),
        ),
        ("OBS_MODULE_LOAD_RULES_LIST_NONE", List::None.code().into()),
        ("OBS_MODULE_LOAD_RULES_LIST_ALL", List::All.code().into()),
        (
            "OBS_MODULE_LOAD_RULES_LIST_MATCH",
            List::Match.code().into(),
        ),
        ("OBS_MODULE_LOAD_RULES_DISABLE", ROUTE_DISABLE.into()),
        ("OBS_MODULE_LOAD_RULES_KEEP_TYPE", ROUTE_KEEP_TYPE.into()),
        ("OBS_MODULE_LOAD_RULES_SET_TYPE", ROUTE_SET_TYPE.into()),
        ("OBS_MODULE_LOAD_RULES_FREE", ROUTE_FREE.into()),
        ("OBS_MODULE_LOAD_RULES_FAIL", ROUTE_FAIL.into()),
    ];
    for (name, value) in pairs {
        assert_eq!(c_constant(name), value, "{name}");
    }
    assert_eq!(c_constant("OBS_MODULE_LOAD_RULES_STOP"), ROUTE_STOP.into());
    assert_eq!(
        c_constant("OBS_MODULE_LOAD_RULES_CORE_CONTINUES"),
        ROUTE_CORE_CONTINUES.into()
    );
    assert_eq!(c_constant("sizeof(enum obs_module_load_rules_list)"), 4);
}

#[test]
fn bools_match_the_c() {
    for v in [false, true] {
        // SAFETY: plain values, for each call below.
        unsafe {
            assert_eq!(
                obs_module_load_rules_arg_missing(v),
                oracle_obs_module_load_rules_arg_missing(v)
            );
            assert_eq!(
                obs_module_load_rules_export_missing(v),
                oracle_obs_module_load_rules_export_missing(v)
            );
            assert_eq!(
                obs_module_load_rules_allow_disable(v),
                oracle_obs_module_load_rules_allow_disable(v)
            );
            assert_eq!(
                obs_module_load_rules_count_failure(v),
                oracle_obs_module_load_rules_count_failure(v)
            );
        }
        let (mut a, mut b) = (0xDEAD_u32, 0xBEEF_u32);
        // SAFETY: valid state pointers.
        let (ra, rb) = unsafe {
            (
                obs_module_load_rules_init_route(v, &raw mut a),
                oracle_obs_module_load_rules_init_route(v, &raw mut b),
            )
        };
        assert_eq!((ra, a), (rb, b), "init {v}");
    }
}

fn types() -> Vec<u32> {
    let mut t: Vec<u32> = (0..16).collect();
    t.extend([
        0x100,
        0x101,
        0x103,
        0x7FFF_FFFF,
        0x8000_0000,
        0x8000_0001,
        0xFFFF_FFFE,
        0xFFFF_FFFF,
    ]);
    t
}

fn codes() -> Vec<c_int> {
    let mut c: Vec<c_int> = (-64..=64).collect();
    c.extend([i32::MIN, i32::MIN + 1, i32::MAX - 1, i32::MAX]);
    c
}

fn route(code: c_int) -> ((u32, u32), (u32, u32)) {
    let (mut a, mut b) = (0xDEAD_u32, 0xBEEF_u32);
    // SAFETY: valid state pointers.
    unsafe {
        (
            (obs_module_load_rules_open_route(code, &raw mut a), a),
            (oracle_obs_module_load_rules_open_route(code, &raw mut b), b),
        )
    }
}

#[test]
fn types_match_the_c() {
    for t in types() {
        // SAFETY: plain values, for each call below.
        unsafe {
            assert_eq!(
                obs_module_load_rules_is_core(t),
                oracle_obs_module_load_rules_is_core(t),
                "{t}"
            );
            assert_eq!(
                obs_module_load_rules_is_legacy(t),
                oracle_obs_module_load_rules_is_legacy(t),
                "{t}"
            );
        }
    }
}

#[test]
fn open_route_matches_the_c() {
    for code in codes() {
        let (rust, c) = route(code);
        assert_eq!(rust, c, "code {code}");
    }
}

#[test]
fn route_null_state() {
    // Intentional difference: the C would write through NULL.
    for code in codes() {
        // SAFETY: a NULL state is allowed by the shim.
        let bits = unsafe { obs_module_load_rules_open_route(code, ptr::null_mut()) };
        assert_eq!(bits, route(code).1.0, "{code}");
    }
    for ok in [false, true] {
        let mut s = 0;
        // SAFETY: a NULL state is allowed by the shim; then a valid one.
        unsafe {
            assert_eq!(
                obs_module_load_rules_init_route(ok, ptr::null_mut()),
                oracle_obs_module_load_rules_init_route(ok, &raw mut s)
            );
        }
    }
}

#[test]
fn versions_match_the_c() {
    let api = api_ver();
    let mut words = vec![
        0,
        1,
        0xFFFF,
        0x1_0000,
        0xFFFF_0000,
        0xFFFF_FFFF,
        0x8000_0000,
        0x7FFF_FFFF,
    ];
    for d in [0_u32, 1, 0xFFFF, 0x1_0000, 0x0100_0000] {
        words.extend([api.wrapping_add(d), api.wrapping_sub(d)]);
    }
    for &w in &words {
        // SAFETY: plain values, for each call below.
        unsafe {
            assert_eq!(
                obs_module_load_rules_version_word(w),
                oracle_obs_module_load_rules_version_word(w),
                "{w:x}"
            );
            for a in [api, 0, 0xFFFF_FFFF, api & 0xFFFF_0000] {
                assert_eq!(
                    obs_module_load_rules_version_ok(w, a),
                    oracle_obs_module_load_rules_version_ok(w, a),
                    "{w:x} {a:x}"
                );
            }
        }
    }
}

#[test]
fn lengths_match_the_c() {
    for n in [0, 1, 2, 3, 1000, usize::MAX - 1, usize::MAX] {
        // SAFETY: plain values, for each call below.
        unsafe {
            assert_eq!(
                obs_module_load_rules_safe_list(n),
                oracle_obs_module_load_rules_safe_list(n)
            );
            assert_eq!(
                obs_module_load_rules_disabled_list(n),
                oracle_obs_module_load_rules_disabled_list(n)
            );
            assert_eq!(
                obs_module_load_rules_core_loaded(n),
                oracle_obs_module_load_rules_core_loaded(n)
            );
        }
    }
}

/// The functions of one side, as `obs-module.c` calls them.
struct Side {
    safe_list: unsafe extern "C" fn(usize) -> u32,
    disabled_list: unsafe extern "C" fn(usize) -> u32,
    is_core: unsafe extern "C" fn(u32) -> bool,
    is_legacy: unsafe extern "C" fn(u32) -> bool,
    open_route: unsafe extern "C" fn(c_int, *mut u32) -> u32,
    init_route: unsafe extern "C" fn(bool, *mut u32) -> u32,
    count_failure: unsafe extern "C" fn(bool) -> bool,
}

const RUST: Side = Side {
    safe_list: obs_module_load_rules_safe_list,
    disabled_list: obs_module_load_rules_disabled_list,
    is_core: obs_module_load_rules_is_core,
    is_legacy: obs_module_load_rules_is_legacy,
    open_route: obs_module_load_rules_open_route,
    init_route: obs_module_load_rules_init_route,
    count_failure: obs_module_load_rules_count_failure,
};

const C: Side = Side {
    safe_list: oracle_obs_module_load_rules_safe_list,
    disabled_list: oracle_obs_module_load_rules_disabled_list,
    is_core: oracle_obs_module_load_rules_is_core,
    is_legacy: oracle_obs_module_load_rules_is_legacy,
    open_route: oracle_obs_module_load_rules_open_route,
    init_route: oracle_obs_module_load_rules_init_route,
    count_failure: oracle_obs_module_load_rules_count_failure,
};

/// `load_all_callback()`: every step it takes, in order, as events.
fn walk(s: &Side, b: u32, t: u32, code: c_int) -> Vec<(&'static str, u32)> {
    let mut ev = Vec::new();
    let is_plugin = b & 1 != 0;
    let safe_num = if b & 2 != 0 { 2 } else { 0 };
    let disabled_num = usize::from(b & 8 != 0);
    let answer = |a: u32, listed: bool, ev: &mut Vec<(&'static str, u32)>, what| match a {
        1 => true,
        0 => false,
        _ => {
            ev.push((what, 0));
            listed
        }
    };
    if !is_plugin {
        ev.push(("skip not plugin", 0));
        return ev;
    }
    // SAFETY: plain values and valid state pointers throughout.
    unsafe {
        if !answer((s.safe_list)(safe_num), b & 4 != 0, &mut ev, "ask safe") {
            ev.push(("disable", STATE_DISABLED_SAFE));
            return ev;
        }
        if !(s.is_core)(t)
            && answer(
                (s.disabled_list)(disabled_num),
                b & 16 != 0,
                &mut ev,
                "ask disabled",
            )
        {
            ev.push(("disable", STATE_DISABLED));
            return ev;
        }
        if (s.is_legacy)(t) {
            ev.push(("ask core", 0));
            if b & 32 != 0 {
                ev.push(("skip legacy core", 0));
                return ev;
            }
        }
        ev.push(("ask existing", 0));
        if b & 64 != 0 {
            let which = if (s.is_core)(t) {
                1
            } else if (s.is_legacy)(t) {
                2
            } else {
                3
            };
            ev.push(("skip loaded", which));
            return ev;
        }
        let mut st = 0xAAAA;
        let r = (s.open_route)(code, &raw mut st);
        ev.push(("open route", r));
        if r & ROUTE_DISABLE != 0 {
            ev.push(("disable", st));
        }
        if r & ROUTE_CORE_CONTINUES != 0 {
            ev.push(("ask type", 0));
            if (s.is_core)(t) {
                ev.push(("core continues", 0));
                return ev;
            }
        }
        if r & ROUTE_FAIL != 0 {
            ev.push(("counted", u32::from((s.count_failure)(b & 256 != 0))));
            return ev;
        }
        if r & ROUTE_STOP != 0 {
            return ev;
        }
        let r = (s.init_route)(b & 128 != 0, &raw mut st);
        ev.push(("init route", r));
        if r & ROUTE_DISABLE != 0 {
            ev.push(("disable", st));
        }
    }
    ev
}

#[test]
fn load_all_callback_matches_the_c() {
    for b in 0..512 {
        for t in types() {
            for code in [
                MODULE_SUCCESS,
                MODULE_ERROR,
                MODULE_FAILED_TO_OPEN,
                MODULE_MISSING_EXPORTS,
                MODULE_INCOMPATIBLE_VER,
                MODULE_HARDCODED_SKIP,
                1,
                -6,
                i32::MIN,
            ] {
                assert_eq!(
                    walk(&RUST, b, t, code),
                    walk(&C, b, t, code),
                    "{b} {t} {code}"
                );
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(4096)))]

    #[test]
    fn version_any(ver in any::<u32>(), api in any::<u32>()) {
        // SAFETY: plain values.
        unsafe {
            prop_assert_eq!(
                obs_module_load_rules_version_word(ver),
                oracle_obs_module_load_rules_version_word(ver)
            );
            let w = obs_module_load_rules_version_word(ver);
            prop_assert_eq!(
                obs_module_load_rules_version_ok(w, api),
                oracle_obs_module_load_rules_version_ok(w, api)
            );
            prop_assert_eq!(
                obs_module_load_rules_version_ok(ver, api),
                oracle_obs_module_load_rules_version_ok(ver, api)
            );
        }
    }

    #[test]
    fn lengths_any(n in any::<usize>()) {
        // SAFETY: plain values.
        unsafe {
            prop_assert_eq!(obs_module_load_rules_safe_list(n), oracle_obs_module_load_rules_safe_list(n));
            prop_assert_eq!(
                obs_module_load_rules_disabled_list(n),
                oracle_obs_module_load_rules_disabled_list(n)
            );
            prop_assert_eq!(obs_module_load_rules_core_loaded(n), oracle_obs_module_load_rules_core_loaded(n));
        }
    }

    #[test]
    fn route_any(code in any::<i32>(), t in any::<u32>()) {
        let (rust, c) = route(code);
        prop_assert_eq!(rust, c);
        // SAFETY: plain values.
        unsafe {
            prop_assert_eq!(obs_module_load_rules_is_core(t), oracle_obs_module_load_rules_is_core(t));
            prop_assert_eq!(obs_module_load_rules_is_legacy(t), oracle_obs_module_load_rules_is_legacy(t));
        }
    }

    #[test]
    fn walk_any(b in 0..512_u32, t in any::<u32>(), code in any::<i32>()) {
        prop_assert_eq!(walk(&RUST, b, t, code), walk(&C, b, t, code));
    }
}
