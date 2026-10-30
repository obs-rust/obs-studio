//! Tier 3 parity: the shims of `obs_output::ffi::output_reconnect` (and the
//! safe core under them) against the original
//! `libobs/obs-output-reconnect.c.h`, compiled as
//! `oracle_obs_output_reconnect_*` (obs-c-oracle,
//! oracle/output_reconnect.c).
//!
//! Every call runs the shim, the C and the safe core, and compares the
//! results exactly where the C is defined and the port keeps its behaviour.
//!
//! Runs: exhaustive over the stop codes (every code of `obs-defs.h`, the
//! range -10 to 5 and the `int` edges) with every pair of the
//! `reconnecting` and `active` flags; the retry counts from -2 to 25 and the
//! `int` edges, for whether reconnecting is on, the give-up (against every
//! attempt from -2 to 25) and the growth; every delay setting the C can
//! multiply without overflow (-2147483 to 2147483 s); the next wait for
//! 1,001 exponents from 1.0 to 2.0 over waits around the limit and the
//! defaults' sequence; whole reconnects of up to 25 attempts for the
//! settings the frontend offers and the defaults, the C and the shim side by
//! side; and proptests over the whole `int` range of the delay setting and
//! exponents from 1.0 to 2.0.
//!
//! Intentional differences, checked on the shim and the core against the C
//! where the C is defined, and on the shim and the core alone where it is
//! not:
//! - every wait stays between 0 and 15 minutes: a delay setting above 900 s
//!   gives the limit where the C keeps it for the first attempt, a negative
//!   one gives 0 where the C wraps it to about 49 days, one whose `* 1000`
//!   overflows `int` gives 0 or the limit (undefined in C), and a later
//!   product above `UINT32_MAX` gives the limit (undefined in C). Over a
//!   whole reconnect only the first wait differs: the C's second wait is
//!   already clamped;
//! - a retry count of 0 or less is off, where the C turns reconnecting on
//!   for any count but 0.
//!
//! `test_output_reconnect_steps` runs the C test against the C; the
//! real-libobs output tests run against the hybrid libobs, where these shims
//! replace the C.

#![allow(
    clippy::as_conversions,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]

use obs_c_oracle::output_reconnect::{
    oracle_obs_output_reconnect_active, oracle_obs_output_reconnect_can,
    oracle_obs_output_reconnect_first_delay, oracle_obs_output_reconnect_give_up,
    oracle_obs_output_reconnect_grows, oracle_obs_output_reconnect_next_delay,
};
use obs_output::ffi::output_reconnect::{
    obs_output_reconnect_active, obs_output_reconnect_can, obs_output_reconnect_first_delay,
    obs_output_reconnect_give_up, obs_output_reconnect_grows, obs_output_reconnect_next_delay,
};
use obs_output::output_reconnect::{self as core, MAX_MSEC, OBS_OUTPUT_DISCONNECTED};
use proptest::prelude::*;

/// The largest delay setting the C multiplies by 1000 without overflow.
const C_SEC_LIMIT: i32 = i32::MAX / 1000;

/// The stop codes: every one of `obs-defs.h` and the range around them,
/// and the `int` edges.
fn codes() -> impl Iterator<Item = i32> {
    (-10..=5).chain([-100, i32::MIN, i32::MIN + 1, i32::MAX])
}

/// Retry counts and attempts: -2 to 25, and the `int` edges.
fn counts() -> impl Iterator<Item = i32> {
    (-2..=25).chain([i32::MIN, i32::MIN + 1, i32::MAX - 1, i32::MAX])
}

/// 1.0 to 2.0 in steps of 0.001, the range around the exponent at create
/// (1.5 to 1.55).
fn exponents() -> impl Iterator<Item = f32> {
    (1000_u32..=2000).map(|n| n as f32 / 1000.0)
}

fn c_first_delay(retry_sec: i32) -> u32 {
    assert!((-C_SEC_LIMIT..=C_SEC_LIMIT).contains(&retry_sec));
    // SAFETY: plain values; `retry_sec * 1000` fits in `int`.
    unsafe { oracle_obs_output_reconnect_first_delay(retry_sec) }
}

/// The C's next wait, where the C is defined (the product fits in
/// `uint32_t`).
fn c_next_delay(cur_msec: u32, exp: f32) -> Option<u32> {
    let product = cur_msec as f32 * exp;
    // SAFETY: plain values; the product converts to `uint32_t` without
    // undefined behaviour.
    (0.0..4_294_967_296.0)
        .contains(&product)
        .then(|| unsafe { oracle_obs_output_reconnect_next_delay(cur_msec, exp) })
}

fn check_first_delay(retry_sec: i32) {
    let shim = obs_output_reconnect_first_delay(retry_sec);
    assert_eq!(shim, core::first_delay(retry_sec), "core, {retry_sec} s");
    assert!(shim <= MAX_MSEC, "{retry_sec} s: {shim}");
    if (-C_SEC_LIMIT..=C_SEC_LIMIT).contains(&retry_sec) {
        let c = c_first_delay(retry_sec);
        if (0..=900).contains(&retry_sec) {
            assert_eq!(shim, c, "{retry_sec} s");
        } else if retry_sec < 0 {
            // Intentional: the C wraps it to about 49 days.
            assert_eq!(shim, 0, "{retry_sec} s");
            assert!(c > u32::MAX - 2_147_483_000, "{retry_sec} s: C {c}");
        } else {
            // Intentional: the C keeps a first wait above the limit.
            assert_eq!(shim, MAX_MSEC, "{retry_sec} s");
            assert_eq!(c, (retry_sec * 1000) as u32, "{retry_sec} s");
        }
    } else {
        // Undefined in C: clamped here.
        let want = if retry_sec < 0 { 0 } else { MAX_MSEC };
        assert_eq!(shim, want, "{retry_sec} s");
    }
}

fn check_next_delay(cur_msec: u32, exp: f32) {
    let shim = obs_output_reconnect_next_delay(cur_msec, exp);
    assert_eq!(
        shim,
        core::next_delay(cur_msec, exp),
        "core, {cur_msec} * {exp}"
    );
    assert!(shim <= MAX_MSEC);
    match c_next_delay(cur_msec, exp) {
        Some(c) => assert_eq!(shim, c, "{cur_msec} * {exp}"),
        // Undefined in C: clamped before the conversion here.
        None => assert_eq!(shim, MAX_MSEC, "{cur_msec} * {exp}"),
    }
}

/// The waits of one reconnect of `n` attempts, as `output_reconnect()`
/// asks for them, through `first`, `grows` and `next`.
fn walk(
    n: i32,
    retry_sec: i32,
    exp: f32,
    first: impl Fn(i32) -> u32,
    grows: impl Fn(i32) -> bool,
    next: impl Fn(u32, f32) -> u32,
) -> Vec<u32> {
    let mut cur = 0;
    (0..n)
        .map(|retries| {
            if retries == 0 {
                cur = first(retry_sec);
            }
            if grows(retries) {
                cur = next(cur, exp);
            }
            cur
        })
        .collect()
}

fn c_walk(n: i32, retry_sec: i32, exp: f32) -> Vec<u32> {
    walk(
        n,
        retry_sec,
        exp,
        c_first_delay,
        // SAFETY: plain values.
        |retries| unsafe { oracle_obs_output_reconnect_grows(retries) },
        |cur, exp| c_next_delay(cur, exp).expect("the C's product fits"),
    )
}

fn shim_walk(n: i32, retry_sec: i32, exp: f32) -> Vec<u32> {
    walk(
        n,
        retry_sec,
        exp,
        |retry_sec| obs_output_reconnect_first_delay(retry_sec),
        |retries| obs_output_reconnect_grows(retries),
        |cur, exp| obs_output_reconnect_next_delay(cur, exp),
    )
}

// ---------------------------------------------------------------------------
// exhaustive

#[test]
fn route_over_every_code_and_flag_pair() {
    for code in codes() {
        for reconnecting in [false, true] {
            for active in [false, true] {
                let shim = obs_output_reconnect_can(reconnecting, active, code);
                // SAFETY: plain values.
                let c = unsafe { oracle_obs_output_reconnect_can(reconnecting, active, code) };
                assert_eq!(
                    shim, c,
                    "code {code}, reconnecting {reconnecting}, active {active}"
                );
                assert_eq!(shim, core::can(reconnecting, active, code));
            }
        }
    }
}

#[test]
fn counts_on_off_give_up_and_growth() {
    for count in counts() {
        let shim = obs_output_reconnect_active(count);
        // SAFETY: plain values.
        let c = unsafe { oracle_obs_output_reconnect_active(count) };
        assert_eq!(shim, core::active(count));
        if count >= 0 {
            assert_eq!(shim, c, "count {count}");
        } else {
            // Intentional: the C turns reconnecting on for a negative count.
            assert!(c && !shim, "count {count}");
        }

        let shim = obs_output_reconnect_grows(count);
        // SAFETY: plain values.
        let c = unsafe { oracle_obs_output_reconnect_grows(count) };
        assert_eq!(shim, c, "retries {count}");
        assert_eq!(shim, core::grows(count));

        for retries in counts() {
            let shim = obs_output_reconnect_give_up(retries, count);
            // SAFETY: plain values.
            let c = unsafe { oracle_obs_output_reconnect_give_up(retries, count) };
            assert_eq!(shim, c, "{retries} of {count}");
            assert_eq!(shim, core::give_up(retries, count));
        }
    }
}

#[test]
fn route_of_a_stop_with_every_count() {
    // can_reconnect() as obs-output.c asks it, the callback agreeing: the
    // C and the port differ only for a negative count on an output that
    // is not reconnecting yet, which the C reconnects on a disconnect.
    for count in counts() {
        for reconnecting in [false, true] {
            for code in codes() {
                let shim = obs_output_reconnect_can(
                    reconnecting,
                    obs_output_reconnect_active(count),
                    code,
                );
                // SAFETY: plain values.
                let c = unsafe {
                    oracle_obs_output_reconnect_can(
                        reconnecting,
                        oracle_obs_output_reconnect_active(count),
                        code,
                    )
                };
                if count < 0 && !reconnecting && code == OBS_OUTPUT_DISCONNECTED {
                    assert!(c && !shim, "count {count}");
                } else {
                    assert_eq!(
                        shim, c,
                        "count {count}, reconnecting {reconnecting}, code {code}"
                    );
                }
            }
        }
    }
}

#[test]
fn every_delay_setting_the_c_can_multiply() {
    for retry_sec in -C_SEC_LIMIT..=C_SEC_LIMIT {
        check_first_delay(retry_sec);
    }
    for retry_sec in [C_SEC_LIMIT + 1, -C_SEC_LIMIT - 1, i32::MAX, i32::MIN] {
        check_first_delay(retry_sec);
    }
}

#[test]
fn next_delay_for_every_exponent() {
    let mut waits: Vec<u32> = vec![
        0,
        1,
        2,
        3,
        999,
        1000,
        2000,
        599_999,
        600_000,
        600_001,
        MAX_MSEC - 1,
        MAX_MSEC,
        MAX_MSEC + 1,
        2_147_483_000,
        2_863_311_530,
        4_294_967_295,
    ];
    waits.extend(shim_walk(25, 2, 1.5));
    waits.extend(shim_walk(25, 30, 1.55));
    for exp in exponents() {
        for &cur in &waits {
            check_next_delay(cur, exp);
        }
    }
}

#[test]
fn whole_reconnects_for_the_frontends_settings() {
    // The settings dialog offers a delay of 1 to 30 s; the defaults are
    // 2 s, and 0 comes from scripts.
    for retry_sec in 0..=30 {
        for exp in exponents() {
            assert_eq!(
                shim_walk(25, retry_sec, exp),
                c_walk(25, retry_sec, exp),
                "{retry_sec} s at {exp}"
            );
        }
    }
}

#[test]
fn whole_reconnects_above_the_limit_differ_only_first() {
    for retry_sec in [901, 1000, 3600, 86400, 2_000_000, C_SEC_LIMIT] {
        for exp in [1.0, 1.5, 1.55, 2.0] {
            let shim = shim_walk(25, retry_sec, exp);
            let c = c_walk(25, retry_sec, exp);
            assert_eq!(shim.first(), Some(&MAX_MSEC), "{retry_sec} s");
            assert_eq!(
                c.first(),
                Some(&((retry_sec * 1000) as u32)),
                "{retry_sec} s"
            );
            assert_eq!(shim.get(1..), c.get(1..), "{retry_sec} s at {exp}");
        }
    }
}

// ---------------------------------------------------------------------------
// proptests

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(4096),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    #[test]
    fn any_delay_setting(retry_sec in any::<i32>()) {
        check_first_delay(retry_sec);
    }

    #[test]
    fn any_wait_and_exponent(
        cur_msec in any::<u32>(),
        exp in (1000_u32..=2000).prop_map(|n| n as f32 / 1000.0),
    ) {
        check_next_delay(cur_msec, exp);
    }

    #[test]
    fn any_whole_reconnect_in_the_cs_range(
        retry_sec in 0..=900_i32,
        exp in (1000_u32..=2000).prop_map(|n| n as f32 / 1000.0),
        n in 1..=40_i32,
    ) {
        prop_assert_eq!(shim_walk(n, retry_sec, exp), c_walk(n, retry_sec, exp));
    }

    #[test]
    fn any_input_on_the_shim(
        retry_sec in any::<i32>(),
        retries in any::<i32>(),
        count in any::<i32>(),
        cur_msec in any::<u32>(),
        exp_bits in any::<u32>(),
        reconnecting in any::<bool>(),
        active in any::<bool>(),
        code in any::<i32>(),
    ) {
        let exp = f32::from_bits(exp_bits);
        prop_assert_eq!(obs_output_reconnect_first_delay(retry_sec), core::first_delay(retry_sec));
        prop_assert_eq!(
            obs_output_reconnect_next_delay(cur_msec, exp),
            core::next_delay(cur_msec, exp)
        );
        prop_assert!(obs_output_reconnect_next_delay(cur_msec, exp) <= MAX_MSEC);
        prop_assert_eq!(obs_output_reconnect_give_up(retries, count), core::give_up(retries, count));
        prop_assert_eq!(obs_output_reconnect_grows(retries), core::grows(retries));
        prop_assert_eq!(obs_output_reconnect_active(count), core::active(count));
        prop_assert_eq!(
            obs_output_reconnect_can(reconnecting, active, code),
            core::can(reconnecting, active, code)
        );
    }
}
