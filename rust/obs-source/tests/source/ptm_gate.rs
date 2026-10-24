//! Tier 1: `obs_source::ptm_gate`, the push-to-mute and push-to-talk gate
//! of `source_muted()`.
//!
//! The `steps_*` cases port `test/cmocka/test_source_ptm_gate_steps.c` 1:1,
//! under the same case names, in the same order, with the same inputs and
//! expectations, the stand-in source and the copies of `source_muted()` and
//! of the gate of `get_source_volume()` included.
//!
//! The `edge_*` cases are new: the stop time against `u128` arithmetic at
//! every delay and clock around the product and sum edges and over the full
//! `u64` range; the hold against the comparison; the gates over their four
//! inputs; and `source_muted()` against the rule written another way (muted
//! when disabled, user-muted, push-to-mute active or push-to-talk not
//! active), over every flag and any stop times, delays and clocks.

use obs_source::ptm_gate::{NS_PER_MS, held, mutes, stop_time, talk_mutes};
use proptest::prelude::*;

/// Written out, not the port's constant, so a wrong `NS_PER_MS` fails.
const MS: u64 = 1_000_000;
const SEC: u64 = 1000 * MS;
/// The largest delay whose product does not wrap: 18446744073709 ms.
const MAX_DELAY: u64 = u64::MAX / MS;
/// What that product leaves below `u64::MAX`: 551615 ns.
const MAX_DELAY_SLACK: u64 = u64::MAX - MAX_DELAY * MS;
/// A clock well away from 0 and `u64::MAX`.
const T: u64 = 1000 * SEC;

#[track_caller]
fn check_stop(os_time: u64, delay: u64, expected: u64) {
    assert_eq!(
        stop_time(os_time, delay),
        expected,
        "stop_time({os_time}, {delay})"
    );
}

#[track_caller]
fn check_held(os_time: u64, stop: u64, expected: bool) {
    assert_eq!(held(os_time, stop), expected, "held({os_time}, {stop})");
}

/// The fields of `struct obs_source` the gate reads and writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct GateSource {
    enabled: bool,
    user_muted: bool,
    muted: bool,
    push_to_mute_enabled: bool,
    push_to_mute_pressed: bool,
    user_push_to_mute_pressed: bool,
    push_to_talk_enabled: bool,
    push_to_talk_pressed: bool,
    user_push_to_talk_pressed: bool,
    push_to_mute_delay: u64,
    push_to_mute_stop_time: u64,
    push_to_talk_delay: u64,
    push_to_talk_stop_time: u64,
}

/// `source_muted()` of `obs-source.c`, on the stand-in.
fn source_muted(source: &mut GateSource, os_time: u64) -> bool {
    if source.push_to_mute_enabled && source.user_push_to_mute_pressed {
        source.push_to_mute_stop_time = stop_time(os_time, source.push_to_mute_delay);
    }

    if source.push_to_talk_enabled && source.user_push_to_talk_pressed {
        source.push_to_talk_stop_time = stop_time(os_time, source.push_to_talk_delay);
    }

    let push_to_mute_active =
        source.user_push_to_mute_pressed || held(os_time, source.push_to_mute_stop_time);
    let push_to_talk_active =
        source.user_push_to_talk_pressed || held(os_time, source.push_to_talk_stop_time);

    !source.enabled
        || source.user_muted
        || mutes(source.push_to_mute_enabled, push_to_mute_active)
        || talk_mutes(source.push_to_talk_enabled, push_to_talk_active)
}

/// The gate of `get_source_volume()` in `obs-source.c`, on the stand-in:
/// the mix's pressed flags and `source->muted`. Only its stop time goes
/// through the port.
fn mix_muted(source: &mut GateSource, os_time: u64) -> bool {
    if source.push_to_mute_enabled && source.push_to_mute_pressed {
        source.push_to_mute_stop_time = stop_time(os_time, source.push_to_mute_delay);
    }

    if source.push_to_talk_enabled && source.push_to_talk_pressed {
        source.push_to_talk_stop_time = stop_time(os_time, source.push_to_talk_delay);
    }

    let push_to_mute_active =
        source.push_to_mute_pressed || os_time < source.push_to_mute_stop_time;
    let push_to_talk_active =
        source.push_to_talk_pressed || os_time < source.push_to_talk_stop_time;

    !source.enabled
        || source.muted
        || (source.push_to_mute_enabled && push_to_mute_active)
        || (source.push_to_talk_enabled && !push_to_talk_active)
}

#[track_caller]
fn check_muted(source: &mut GateSource, os_time: u64, expected: bool) {
    let before = *source;
    assert_eq!(
        source_muted(source, os_time),
        expected,
        "source_muted({before:?}, {os_time})"
    );
}

fn enabled_source() -> GateSource {
    GateSource {
        enabled: true,
        ..GateSource::default()
    }
}

// ---------------------------------------------------------------------------
// stop_time()

#[test]
fn steps_stop_time_valid_delays() {
    check_stop(T, 300, T + 300 * MS);
    check_stop(T, 1, T + MS);
    check_stop(0, 300, 300 * MS);
    // the settings dialog's largest delay, INT_MAX ms
    check_stop(T, 2_147_483_647, T + 2_147_483_647 * MS);
}

#[test]
fn steps_stop_time_delay_zero() {
    check_stop(T, 0, T);
    check_stop(0, 0, 0);
    check_stop(1, 0, 1);
    // nothing to add, so nothing saturates
    check_stop(u64::MAX, 0, u64::MAX);
    check_stop(u64::MAX - 1, 0, u64::MAX - 1);
}

#[test]
fn steps_stop_time_product_edge() {
    assert_eq!(MAX_DELAY, 18_446_744_073_709);
    assert_eq!(MAX_DELAY_SLACK, 551_615);

    check_stop(0, MAX_DELAY, MAX_DELAY * MS);
    check_stop(0, MAX_DELAY - 1, (MAX_DELAY - 1) * MS);
    check_stop(0, MAX_DELAY + 1, u64::MAX);
    check_stop(T, MAX_DELAY + 1, u64::MAX);
    check_stop(0, u64::MAX, u64::MAX);
    check_stop(0, u64::MAX / 2, u64::MAX);
    // INT64_MAX, the largest delay a scene collection stores
    check_stop(0, 9_223_372_036_854_775_807, u64::MAX);
    // a negative int64 delay converted, as the setters still accept
    check_stop(T, (-1_i64).cast_unsigned(), u64::MAX);
    check_stop(T, (-300_i64).cast_unsigned(), u64::MAX);
}

#[test]
fn steps_stop_time_sum_edge() {
    check_stop(MAX_DELAY_SLACK - 1, MAX_DELAY, u64::MAX - 1);
    check_stop(MAX_DELAY_SLACK, MAX_DELAY, u64::MAX);
    check_stop(MAX_DELAY_SLACK + 1, MAX_DELAY, u64::MAX);
    check_stop(T, MAX_DELAY, u64::MAX);
    check_stop(u64::MAX, MAX_DELAY, u64::MAX);

    // a clock near the end with a small delay
    check_stop(u64::MAX - MS, 1, u64::MAX);
    check_stop(u64::MAX - MS - 1, 1, u64::MAX - 1);
    check_stop(u64::MAX - MS + 1, 1, u64::MAX);
    check_stop(u64::MAX, 1, u64::MAX);
    check_stop(u64::MAX - 300 * MS, 300, u64::MAX);
    check_stop(u64::MAX - 300 * MS - 1, 300, u64::MAX - 1);
    check_stop(u64::MAX - 300 * MS + 1, 300, u64::MAX);
}

// ---------------------------------------------------------------------------
// held() and the two gates

#[test]
fn steps_held_before_the_stop_time() {
    check_held(T - 1, T, true);
    check_held(T, T, false);
    check_held(T + 1, T, false);
    check_held(0, 0, false);
    check_held(0, 1, true);
    check_held(u64::MAX - 1, u64::MAX, true);
    check_held(u64::MAX, u64::MAX, false);
    // not modular: a stop time of 0 (none stored yet) is before every clock
    check_held(u64::MAX, 0, false);
}

#[test]
fn steps_gates_truth_tables() {
    for enabled in [false, true] {
        for active in [false, true] {
            assert_eq!(mutes(enabled, active), enabled && active, "push-to-mute");
            assert_eq!(
                talk_mutes(enabled, active),
                enabled && !active,
                "push-to-talk"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// source_muted()

#[test]
fn steps_source_muted_every_flag() {
    for bits in 0_u32..64 {
        let mut s = GateSource {
            enabled: bits & 1 != 0,
            user_muted: bits & 2 != 0,
            push_to_mute_enabled: bits & 4 != 0,
            user_push_to_mute_pressed: bits & 8 != 0,
            push_to_talk_enabled: bits & 16 != 0,
            user_push_to_talk_pressed: bits & 32 != 0,
            push_to_mute_delay: 300,
            push_to_talk_delay: 500,
            ..GateSource::default()
        };

        let expected = !s.enabled
            || s.user_muted
            || (s.push_to_mute_enabled && s.user_push_to_mute_pressed)
            || (s.push_to_talk_enabled && !s.user_push_to_talk_pressed);
        assert_eq!(source_muted(&mut s, T), expected, "flags {bits:#04x}");
        let ptm = if s.push_to_mute_enabled && s.user_push_to_mute_pressed {
            T + 300 * MS
        } else {
            0
        };
        let ptt = if s.push_to_talk_enabled && s.user_push_to_talk_pressed {
            T + 500 * MS
        } else {
            0
        };
        assert_eq!(
            s.push_to_mute_stop_time, ptm,
            "flags {bits:#04x}: ptm stop time"
        );
        assert_eq!(
            s.push_to_talk_stop_time, ptt,
            "flags {bits:#04x}: ptt stop time"
        );
    }
}

#[test]
fn steps_source_muted_disabled_and_user_muted() {
    let mut s = enabled_source();
    s.push_to_talk_enabled = true;
    s.user_push_to_talk_pressed = true;
    s.push_to_talk_delay = 300;
    check_muted(&mut s, T, false);

    s.enabled = false;
    check_muted(&mut s, T + SEC, true);
    assert_eq!(s.push_to_talk_stop_time, T + SEC + 300 * MS);

    s.enabled = true;
    s.user_muted = true;
    check_muted(&mut s, T + 2 * SEC, true);
    assert_eq!(s.push_to_talk_stop_time, T + 2 * SEC + 300 * MS);

    s.user_muted = false;
    check_muted(&mut s, T + 3 * SEC, false);
}

#[test]
fn steps_push_to_talk_release_inside_and_after_the_delay() {
    let mut s = enabled_source();
    s.push_to_talk_enabled = true;
    s.push_to_talk_delay = 300;

    // never pressed: closed
    check_muted(&mut s, T - SEC, true);

    s.user_push_to_talk_pressed = true;
    check_muted(&mut s, T, false);
    assert_eq!(s.push_to_talk_stop_time, T + 300 * MS);

    s.user_push_to_talk_pressed = false;
    check_muted(&mut s, T + 1, false);
    check_muted(&mut s, T + 150 * MS, false);
    check_muted(&mut s, T + 300 * MS - 1, false);
    check_muted(&mut s, T + 300 * MS, true);
    check_muted(&mut s, T + 300 * MS + 1, true);
    check_muted(&mut s, T + 10 * SEC, true);
    // a release does not move the stop time
    assert_eq!(s.push_to_talk_stop_time, T + 300 * MS);

    // each packet while held moves it on
    s.user_push_to_talk_pressed = true;
    check_muted(&mut s, T + 20 * SEC, false);
    check_muted(&mut s, T + 21 * SEC, false);
    assert_eq!(s.push_to_talk_stop_time, T + 21 * SEC + 300 * MS);
    s.user_push_to_talk_pressed = false;
    check_muted(&mut s, T + 21 * SEC + 300 * MS - 1, false);
    check_muted(&mut s, T + 21 * SEC + 300 * MS, true);
}

#[test]
fn steps_push_to_mute_release_inside_and_after_the_delay() {
    let mut s = enabled_source();
    s.push_to_mute_enabled = true;
    s.push_to_mute_delay = 300;

    check_muted(&mut s, T - SEC, false);

    s.user_push_to_mute_pressed = true;
    check_muted(&mut s, T, true);
    assert_eq!(s.push_to_mute_stop_time, T + 300 * MS);

    s.user_push_to_mute_pressed = false;
    check_muted(&mut s, T + 1, true);
    check_muted(&mut s, T + 300 * MS - 1, true);
    check_muted(&mut s, T + 300 * MS, false);
    check_muted(&mut s, T + 10 * SEC, false);
}

#[test]
fn steps_delay_zero() {
    let mut s = enabled_source();
    s.push_to_talk_enabled = true;
    s.push_to_mute_enabled = true;

    s.user_push_to_talk_pressed = true;
    check_muted(&mut s, T, false);
    assert_eq!(s.push_to_talk_stop_time, T);
    s.user_push_to_talk_pressed = false;
    check_muted(&mut s, T, true);
    check_muted(&mut s, T - 1, false);

    s.push_to_talk_enabled = false;
    s.user_push_to_mute_pressed = true;
    check_muted(&mut s, T + SEC, true);
    assert_eq!(s.push_to_mute_stop_time, T + SEC);
    s.user_push_to_mute_pressed = false;
    check_muted(&mut s, T + SEC, false);
}

#[test]
fn steps_saturated_delays_hold() {
    let mut s = enabled_source();
    s.push_to_talk_enabled = true;
    s.push_to_talk_delay = MAX_DELAY + 1;
    s.user_push_to_talk_pressed = true;
    check_muted(&mut s, T, false);
    assert_eq!(s.push_to_talk_stop_time, u64::MAX);
    s.user_push_to_talk_pressed = false;
    check_muted(&mut s, T + 1, false);
    check_muted(&mut s, u64::MAX - 1, false);
    // characterized, not endorsed: u64::MAX itself is not before it
    check_muted(&mut s, u64::MAX, true);

    // the sum: MAX_DELAY from a clock past the slack
    s.push_to_talk_delay = MAX_DELAY;
    s.user_push_to_talk_pressed = true;
    check_muted(&mut s, MAX_DELAY_SLACK + 1, false);
    assert_eq!(s.push_to_talk_stop_time, u64::MAX);
    s.user_push_to_talk_pressed = false;
    check_muted(&mut s, MAX_DELAY_SLACK + 2, false);

    // just inside: no saturation, the exact sum
    s.user_push_to_talk_pressed = true;
    check_muted(&mut s, MAX_DELAY_SLACK - 1, false);
    assert_eq!(s.push_to_talk_stop_time, u64::MAX - 1);
    s.user_push_to_talk_pressed = false;
    check_muted(&mut s, u64::MAX - 2, false);
    check_muted(&mut s, u64::MAX - 1, true);

    let mut m = enabled_source();
    m.push_to_mute_enabled = true;
    m.push_to_mute_delay = u64::MAX;
    m.user_push_to_mute_pressed = true;
    check_muted(&mut m, T, true);
    assert_eq!(m.push_to_mute_stop_time, u64::MAX);
    m.user_push_to_mute_pressed = false;
    check_muted(&mut m, T + 100 * SEC, true);
}

#[test]
fn steps_disabled_gate_stores_nothing() {
    let mut s = enabled_source();
    s.push_to_mute_delay = 300;
    s.user_push_to_mute_pressed = true;
    check_muted(&mut s, T, false);
    assert_eq!(s.push_to_mute_stop_time, 0);

    s.push_to_mute_enabled = true;
    check_muted(&mut s, T + SEC, true);
    assert_eq!(s.push_to_mute_stop_time, T + SEC + 300 * MS);
    s.user_push_to_mute_pressed = false;
    s.push_to_mute_enabled = false;
    check_muted(&mut s, T + SEC + 1, false);
    s.push_to_mute_enabled = true;
    check_muted(&mut s, T + SEC + 2, true);
    check_muted(&mut s, T + SEC + 300 * MS, false);
}

#[test]
fn steps_stop_time_follows_the_last_packet() {
    let mut s = enabled_source();
    s.push_to_talk_enabled = true;
    s.push_to_talk_delay = 300;
    s.user_push_to_talk_pressed = true;
    check_muted(&mut s, T + 10 * SEC, false);
    check_muted(&mut s, T, false);
    assert_eq!(s.push_to_talk_stop_time, T + 300 * MS);
    s.user_push_to_talk_pressed = false;
    check_muted(&mut s, T + 300 * MS, true);
}

#[test]
fn steps_both_gates() {
    let mut s = enabled_source();
    s.push_to_mute_enabled = true;
    s.push_to_talk_enabled = true;
    s.push_to_mute_delay = 100;
    s.push_to_talk_delay = 300;

    check_muted(&mut s, T, true);
    s.user_push_to_talk_pressed = true;
    check_muted(&mut s, T, false);
    s.user_push_to_mute_pressed = true;
    check_muted(&mut s, T + MS, true);
    s.user_push_to_mute_pressed = false;
    check_muted(&mut s, T + 101 * MS - 1, true);
    check_muted(&mut s, T + 101 * MS, false);
    s.user_push_to_talk_pressed = false;
    assert_eq!(s.push_to_talk_stop_time, T + 101 * MS + 300 * MS);
    check_muted(&mut s, T + 401 * MS - 1, false);
    check_muted(&mut s, T + 401 * MS, true);
}

#[test]
fn steps_mix_and_capture_share_the_stop_time() {
    let mut s = enabled_source();
    s.push_to_talk_enabled = true;
    s.push_to_talk_delay = 300;

    // the mix's action says held; the hotkey thread has already let go
    s.push_to_talk_pressed = true;
    assert!(!mix_muted(&mut s, T));
    assert_eq!(s.push_to_talk_stop_time, T + 300 * MS);
    check_muted(&mut s, T + 300 * MS - 1, false);
    check_muted(&mut s, T + 300 * MS, true);

    // and the other way: a capture packet's store holds the mix open
    s.push_to_talk_pressed = false;
    s.user_push_to_talk_pressed = true;
    check_muted(&mut s, T + SEC, false);
    s.user_push_to_talk_pressed = false;
    assert!(!mix_muted(&mut s, T + SEC + 300 * MS - 1));
    assert!(mix_muted(&mut s, T + SEC + 300 * MS));

    // the mix saturates through the same helper
    s.push_to_talk_delay = u64::MAX;
    s.push_to_talk_pressed = true;
    assert!(!mix_muted(&mut s, T + 2 * SEC));
    assert_eq!(s.push_to_talk_stop_time, u64::MAX);

    // the mix reads source->muted, not user_muted
    s.user_muted = true;
    assert!(!mix_muted(&mut s, T + 3 * SEC));
    check_muted(&mut s, T + 3 * SEC, true);
}

// ---------------------------------------------------------------------------
// edge cases

/// The stop time in `u128`, clamped: what saturation means.
fn wide_stop(os_time: u64, delay: u64) -> u64 {
    let wide = u128::from(os_time) + u128::from(delay) * u128::from(MS);
    u64::try_from(wide).unwrap_or(u64::MAX)
}

/// Clocks and delays around every edge: 0, the slack, the product's edge,
/// the halfway points and the end of the range, each +-2.
fn special_values() -> Vec<u64> {
    let centres = [
        0,
        MS,
        SEC,
        T,
        MAX_DELAY_SLACK,
        MAX_DELAY,
        MAX_DELAY * MS,
        u64::MAX / 2,
        i64::MAX.cast_unsigned(),
        u64::MAX - MS,
        u64::MAX - MAX_DELAY_SLACK,
        u64::MAX,
    ];
    let mut values = Vec::new();
    for c in centres {
        for d in 0..=4_u64 {
            values.push(c.wrapping_add(d).wrapping_sub(2));
        }
    }
    values.sort_unstable();
    values.dedup();
    values
}

#[test]
fn edge_ns_per_ms_is_a_millisecond() {
    assert_eq!(NS_PER_MS, 1_000_000);
    check_stop(0, 1, 1_000_000);
    check_stop(0, 1000, 1_000_000_000);
}

#[test]
fn edge_stop_time_every_special_pair() {
    let values = special_values();
    for &os_time in &values {
        for &delay in &values {
            check_stop(os_time, delay, wide_stop(os_time, delay));
        }
    }
}

#[test]
fn edge_stop_time_never_before_the_clock() {
    let values = special_values();
    for &os_time in &values {
        for &delay in &values {
            let stop = stop_time(os_time, delay);
            assert!(stop >= os_time, "stop_time({os_time}, {delay}) = {stop}");
            if delay == 0 {
                assert_eq!(stop, os_time);
            }
        }
    }
}

#[test]
fn edge_held_every_special_pair() {
    let values = special_values();
    for &os_time in &values {
        for &stop in &values {
            check_held(os_time, stop, os_time < stop);
        }
    }
}

#[test]
fn edge_gates_are_disjoint_when_enabled() {
    for active in [false, true] {
        assert_ne!(mutes(true, active), talk_mutes(true, active));
        assert!(!mutes(false, active));
        assert!(!talk_mutes(false, active));
    }
}

/// `source_muted()` written another way: what each gate stores and whether
/// the source is muted.
fn rule(source: &GateSource, os_time: u64) -> (bool, u64, u64) {
    let ptm_stop = if source.push_to_mute_enabled && source.user_push_to_mute_pressed {
        wide_stop(os_time, source.push_to_mute_delay)
    } else {
        source.push_to_mute_stop_time
    };
    let ptt_stop = if source.push_to_talk_enabled && source.user_push_to_talk_pressed {
        wide_stop(os_time, source.push_to_talk_delay)
    } else {
        source.push_to_talk_stop_time
    };
    let ptm_on = source.user_push_to_mute_pressed || os_time < ptm_stop;
    let ptt_on = source.user_push_to_talk_pressed || os_time < ptt_stop;
    let gated = match (
        source.enabled,
        source.user_muted,
        source.push_to_mute_enabled && ptm_on,
        source.push_to_talk_enabled,
        ptt_on,
    ) {
        (false, ..) | (_, true, ..) | (_, _, true, ..) => true,
        (_, _, false, true, on) => !on,
        (_, _, false, false, _) => false,
    };
    (gated, ptm_stop, ptt_stop)
}

fn source_of(bits: u8, delays: (u64, u64), stops: (u64, u64)) -> GateSource {
    GateSource {
        enabled: bits & 1 != 0,
        user_muted: bits & 2 != 0,
        push_to_mute_enabled: bits & 4 != 0,
        user_push_to_mute_pressed: bits & 8 != 0,
        push_to_talk_enabled: bits & 16 != 0,
        user_push_to_talk_pressed: bits & 32 != 0,
        push_to_mute_delay: delays.0,
        push_to_talk_delay: delays.1,
        push_to_mute_stop_time: stops.0,
        push_to_talk_stop_time: stops.1,
        ..GateSource::default()
    }
}

#[track_caller]
fn check_rule(source: GateSource, os_time: u64) {
    let mut s = source;
    let muted = source_muted(&mut s, os_time);
    assert_eq!(
        (muted, s.push_to_mute_stop_time, s.push_to_talk_stop_time),
        rule(&source, os_time),
        "source_muted({source:?}, {os_time})"
    );
    // nothing else changes
    assert_eq!(
        GateSource {
            push_to_mute_stop_time: source.push_to_mute_stop_time,
            push_to_talk_stop_time: source.push_to_talk_stop_time,
            ..s
        },
        source
    );
}

#[test]
fn edge_source_muted_every_flag_and_stop_time() {
    // each stop time before, at and after the clock, and saturated
    let stops = [0, T - 1, T, T + 1, u64::MAX];
    for bits in 0_u8..64 {
        for &ptm in &stops {
            for &ptt in &stops {
                for delays in [(0, 0), (300, 500), (MAX_DELAY + 1, u64::MAX)] {
                    check_rule(source_of(bits, delays, (ptm, ptt)), T);
                }
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 20_000, ..ProptestConfig::default() })]

    #[test]
    fn edge_stop_time_full_range(os_time in any::<u64>(), delay in any::<u64>()) {
        prop_assert_eq!(stop_time(os_time, delay), wide_stop(os_time, delay));
    }

    #[test]
    fn edge_stop_time_near_the_edges(
        os_time in prop_oneof![0..=2 * MAX_DELAY_SLACK, u64::MAX - 1000 * SEC..=u64::MAX, any::<u64>()],
        delay in prop_oneof![0..=1000_u64, MAX_DELAY - 1000..=MAX_DELAY + 1000, any::<u64>()],
    ) {
        prop_assert_eq!(stop_time(os_time, delay), wide_stop(os_time, delay));
    }

    #[test]
    fn edge_held_full_range(os_time in any::<u64>(), stop in any::<u64>()) {
        prop_assert_eq!(held(os_time, stop), os_time < stop);
    }

    #[test]
    fn edge_source_muted_full_range(
        bits in 0_u8..64,
        delays in (any::<u64>(), any::<u64>()),
        stops in (any::<u64>(), any::<u64>()),
        os_time in any::<u64>(),
    ) {
        check_rule(source_of(bits, delays, stops), os_time);
    }
}
