//! Tier 1: `obs_output::output_reconnect`, the reconnect policy of
//! `obs-output.c`.
//!
//! The `steps_*` cases port `test/cmocka/test_output_reconnect_steps.c`
//! 1:1: its 10 cases under the same names, in the same order, with the same
//! values and expectations, the "characterized, not endorsed" one included.
//! The C calls the hidden `obs_output_reconnect_*` functions; here the safe
//! core is called directly.
//!
//! The `edge_*` cases are new: proptests of the waits against a model of the
//! C's expressions where the C is defined, their bounds and growth, any
//! input without a panic, and the intentional differences (the wait clamped
//! at every step, a negative count off).

#![allow(
    clippy::as_conversions,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]

use obs_output::output_reconnect::{
    MAX_MSEC, OBS_OUTPUT_DISCONNECTED, OBS_OUTPUT_SUCCESS, active, can, first_delay, give_up,
    grows, next_delay,
};
use proptest::prelude::*;

const DEFAULT_RETRY_SEC: i32 = 2;
const DEFAULT_RETRY_MAX: usize = 20;

/// Every stop code of `obs-defs.h`, and two that are not.
const ALL_CODES: [i32; 12] = [
    OBS_OUTPUT_SUCCESS,
    -1, // OBS_OUTPUT_BAD_PATH
    -2, // OBS_OUTPUT_CONNECT_FAILED
    -3, // OBS_OUTPUT_INVALID_STREAM
    -4, // OBS_OUTPUT_ERROR
    OBS_OUTPUT_DISCONNECTED,
    -6, // OBS_OUTPUT_UNSUPPORTED
    -7, // OBS_OUTPUT_NO_SPACE
    -8, // OBS_OUTPUT_ENCODE_ERROR
    -9, // OBS_OUTPUT_HDR_DISABLED
    1,
    -100,
];

/// The waits `output_reconnect()` sets for attempts 1 to `N` of one
/// reconnect, as it calls the policy.
fn walk<const N: usize>(retry_sec: i32, exp: f32) -> [u32; N] {
    let mut delays = [0; N];
    let mut cur = 0;
    for (retries, delay) in delays.iter_mut().enumerate() {
        let retries = retries as i32;
        if retries == 0 {
            cur = first_delay(retry_sec);
        }
        if grows(retries) {
            cur = next_delay(cur, exp);
        }
        *delay = cur;
    }
    delays
}

// ---------------------------------------------------------------------------
// the delays

#[test]
fn steps_first_delay_from_the_setting() {
    assert_eq!(first_delay(0), 0, "0 s");
    assert_eq!(first_delay(1), 1000, "1 s");
    assert_eq!(first_delay(DEFAULT_RETRY_SEC), 2000, "2 s (default)");
    assert_eq!(first_delay(10), 10000, "10 s");
    assert_eq!(first_delay(60), 60000, "60 s");
    assert_eq!(first_delay(900), 900_000, "900 s, the limit");
}

#[test]
fn steps_default_delays_grow_to_the_limit() {
    // The exponent is 1.5 plus up to 0.05 of jitter, drawn at create.
    let at_1_5: [u32; DEFAULT_RETRY_MAX] = [
        2000, 3000, 4500, 6750, 10125, 15187, 22780, 34170, 51255, 76882, 115_323, 172_984,
        259_476, 389_214, 583_821, 875_731, 900_000, 900_000, 900_000, 900_000,
    ];
    let at_1_55: [u32; DEFAULT_RETRY_MAX] = [
        2000, 3100, 4805, 7447, 11542, 17890, 27729, 42979, 66617, 103_256, 160_046, 248_071,
        384_510, 595_990, 900_000, 900_000, 900_000, 900_000, 900_000, 900_000,
    ];
    assert_eq!(walk(DEFAULT_RETRY_SEC, 1.5), at_1_5, "2 s at 1.5");
    assert_eq!(walk(DEFAULT_RETRY_SEC, 1.55), at_1_55, "2 s at 1.55");
}

#[test]
fn steps_other_settings_and_exponents() {
    let ten_at_1_5: [u32; DEFAULT_RETRY_MAX] = [
        10000, 15000, 22500, 33750, 50625, 75937, 113_905, 170_857, 256_285, 384_427, 576_640,
        864_960, 900_000, 900_000, 900_000, 900_000, 900_000, 900_000, 900_000, 900_000,
    ];
    let one_at_1_55: [u32; DEFAULT_RETRY_MAX] = [
        1000, 1550, 2402, 3723, 5770, 8943, 13861, 21484, 33300, 51615, 80003, 124_004, 192_206,
        297_919, 461_774, 715_749, 900_000, 900_000, 900_000, 900_000,
    ];
    let two_at_2: [u32; DEFAULT_RETRY_MAX] = [
        2000, 4000, 8000, 16000, 32000, 64000, 128_000, 256_000, 512_000, 900_000, 900_000,
        900_000, 900_000, 900_000, 900_000, 900_000, 900_000, 900_000, 900_000, 900_000,
    ];
    assert_eq!(walk(10, 1.5), ten_at_1_5, "10 s at 1.5");
    assert_eq!(walk(1, 1.55), one_at_1_55, "1 s at 1.55");
    assert_eq!(walk(DEFAULT_RETRY_SEC, 2.0), two_at_2, "2 s at 2");
    assert_eq!(
        walk::<DEFAULT_RETRY_MAX>(DEFAULT_RETRY_SEC, 1.0),
        [2000; DEFAULT_RETRY_MAX],
        "2 s at 1"
    );
}

#[test]
fn steps_a_delay_of_zero_stays_zero() {
    // Characterized, not endorsed: every attempt starts at once.
    assert_eq!(
        walk::<DEFAULT_RETRY_MAX>(0, 1.5),
        [0; DEFAULT_RETRY_MAX],
        "0 s at 1.5"
    );
    assert_eq!(
        walk::<DEFAULT_RETRY_MAX>(0, 1.55),
        [0; DEFAULT_RETRY_MAX],
        "0 s at 1.55"
    );
}

#[test]
fn steps_next_delay_at_the_limit() {
    assert_eq!(next_delay(599_999, 1.5), 899_998, "599999 * 1.5");
    assert_eq!(next_delay(600_000, 1.5), MAX_MSEC, "600000 * 1.5");
    assert_eq!(next_delay(600_001, 1.5), MAX_MSEC, "600001 * 1.5");
    assert_eq!(next_delay(MAX_MSEC, 1.5), MAX_MSEC, "the limit * 1.5");
    assert_eq!(next_delay(2_000_000, 1.5), MAX_MSEC, "2000000 * 1.5");
    assert_eq!(next_delay(MAX_MSEC, 1.0), MAX_MSEC, "the limit * 1");
    // Truncated, not rounded.
    assert_eq!(next_delay(1, 1.5), 1, "1 * 1.5");
    assert_eq!(next_delay(3, 1.5), 4, "3 * 1.5");
    assert_eq!(next_delay(0, 1.55), 0, "0 * 1.55");
}

#[test]
fn steps_only_later_attempts_grow() {
    let max = DEFAULT_RETRY_MAX as i32;
    assert!(!grows(0));
    assert!(grows(1));
    assert!(grows(2));
    assert!(grows(max - 1));
    assert!(grows(max));
}

// ---------------------------------------------------------------------------
// the retries

#[test]
fn steps_gives_up_after_the_count() {
    let max = DEFAULT_RETRY_MAX as i32;
    for retries in 0..max {
        assert!(!give_up(retries, max), "{retries} of {max}");
    }
    assert!(give_up(max, max));
    assert!(give_up(max + 1, max));

    assert!(!give_up(0, 1));
    assert!(give_up(1, 1));

    // A count lowered to 0 while reconnecting: the next attempt gives up.
    assert!(give_up(0, 0));
    assert!(give_up(5, 0));
}

// ---------------------------------------------------------------------------
// the route

#[test]
fn steps_reconnecting_is_on_for_a_count() {
    // With a count of 0, the callback is not asked and a disconnect stops
    // the output.
    assert!(!active(0));
    assert!(active(1));
    assert!(active(DEFAULT_RETRY_MAX as i32));
    assert!(active(10000));
}

#[test]
fn steps_an_active_output_reconnects_only_on_a_disconnect() {
    for code in ALL_CODES {
        let want = code == OBS_OUTPUT_DISCONNECTED;
        assert_eq!(can(false, true, code), want, "active, code {code}");
        // Reconnecting off: never.
        assert!(!can(false, false, code), "off, code {code}");
    }
}

#[test]
fn steps_a_failed_attempt_retries_on_any_error() {
    for code in ALL_CODES {
        let want = code != OBS_OUTPUT_SUCCESS;
        // Whether reconnecting is still on does not matter: a count lowered
        // to 0 while reconnecting retries too, and gives up.
        assert_eq!(can(true, true, code), want, "reconnecting, code {code}");
        assert_eq!(can(true, false, code), want, "reconnecting, code {code}");
    }
}

// ---------------------------------------------------------------------------
// edges

/// The C's `(uint32_t)((float)cur_msec * exp)` and its clamp, where the
/// product fits in `uint32_t` (the C is defined).
fn c_next_delay(cur_msec: u32, exp: f32) -> Option<u32> {
    let product = cur_msec as f32 * exp;
    if !(0.0..4_294_967_296.0).contains(&product) {
        return None;
    }
    Some((product as u32).min(MAX_MSEC))
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 4096,
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    #[test]
    fn edge_first_delay_is_the_setting_clamped(retry_sec in any::<i32>()) {
        let want = (i64::from(retry_sec) * 1000).clamp(0, i64::from(MAX_MSEC)) as u32;
        prop_assert_eq!(first_delay(retry_sec), want);
    }

    #[test]
    fn edge_first_delay_matches_c_where_c_keeps_it(retry_sec in 0..=900_i32) {
        // In the C's range (no overflow, no wrap, under the limit).
        prop_assert_eq!(first_delay(retry_sec), (retry_sec * 1000) as u32);
    }

    #[test]
    fn edge_next_delay_matches_c_where_c_is_defined(
        cur_msec in any::<u32>(),
        // The exponent at create is 1.5 to 1.55; any from 0 to 2 here.
        exp in (0_u32..=2000).prop_map(|n| n as f32 / 1000.0),
    ) {
        if let Some(want) = c_next_delay(cur_msec, exp) {
            prop_assert_eq!(next_delay(cur_msec, exp), want);
        }
    }

    #[test]
    fn edge_next_delay_grows_and_stays_under_the_limit(
        cur_msec in 0..=MAX_MSEC,
        exp in (1000_u32..=2000).prop_map(|n| n as f32 / 1000.0),
    ) {
        let next = next_delay(cur_msec, exp);
        prop_assert!(next >= cur_msec, "{} -> {}", cur_msec, next);
        prop_assert!(next <= MAX_MSEC);
    }

    #[test]
    fn edge_any_input_without_a_panic(
        retry_sec in any::<i32>(),
        retries in any::<i32>(),
        retry_max in any::<i32>(),
        cur_msec in any::<u32>(),
        exp_bits in any::<u32>(),
        reconnecting in any::<bool>(),
        on in any::<bool>(),
        code in any::<i32>(),
    ) {
        let exp = f32::from_bits(exp_bits);
        prop_assert!(first_delay(retry_sec) <= MAX_MSEC);
        prop_assert!(next_delay(cur_msec, exp) <= MAX_MSEC);
        let _ = give_up(retries, retry_max);
        let _ = grows(retries);
        prop_assert_eq!(active(retry_max), retry_max > 0);
        let _ = can(reconnecting, on, code);
    }

    #[test]
    fn edge_a_whole_reconnect_waits_at_most_the_limit(
        retry_sec in any::<i32>(),
        exp in (1000_u32..=2000).prop_map(|n| n as f32 / 1000.0),
    ) {
        let delays = walk::<25>(retry_sec, exp);
        for pair in delays.windows(2) {
            prop_assert!(pair[0] <= pair[1], "{:?}", delays);
        }
        prop_assert!(delays.iter().all(|&d| d <= MAX_MSEC), "{:?}", delays);
    }
}

#[test]
fn edge_intentional_differences() {
    // The C overflows int above 2147483 s (undefined); clamped here.
    assert_eq!(first_delay(2_147_484), MAX_MSEC);
    assert_eq!(first_delay(i32::MAX), MAX_MSEC);
    // The C wraps a negative setting to about 49 days; 0 here.
    assert_eq!(first_delay(-1), 0);
    assert_eq!(first_delay(i32::MIN), 0);
    // The C keeps a first wait above 15 minutes; clamped here.
    assert_eq!(first_delay(901), MAX_MSEC);
    assert_eq!(first_delay(3600), MAX_MSEC);
    // The C converts a product above UINT32_MAX (undefined); clamped first
    // here.
    assert_eq!(next_delay(u32::MAX, 1.5), MAX_MSEC);
    assert_eq!(next_delay(4_294_966_296, 1.55), MAX_MSEC);
    // Undefined in C as well: a negative product gives 0, one that is not a
    // number the limit.
    assert_eq!(next_delay(1000, -1.5), 0);
    assert_eq!(next_delay(1000, f32::NAN), MAX_MSEC);
    assert_eq!(next_delay(1000, f32::INFINITY), MAX_MSEC);
    assert_eq!(next_delay(0, f32::INFINITY), MAX_MSEC);
    // The C turns reconnecting on for a negative count; off here, as for 0.
    assert!(!active(-1));
    assert!(!active(-20));
    assert!(!active(i32::MIN));
    assert!(active(i32::MAX));
}
