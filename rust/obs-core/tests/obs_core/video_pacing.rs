//! Tier 1: `obs_core::video_pacing`, the frame pacing decisions of
//! `video_sleep()`.
//!
//! The `steps_*` cases port `test/cmocka/test_video_pacing_steps.c` 1:1: its
//! 5 cases under the same names, in the same order, with the same values and
//! expectations, the "characterized, not endorsed" ones included. The C calls
//! the hidden `obs_video_pacing_*` functions; here the safe core is called
//! directly.
//!
//! The `edge_*` cases are new: proptests over full-range frame times,
//! intervals, clock values and group counts against models written from the
//! C's expressions (with its conversions as the supported compilers do
//! them), and the intentional difference for an interval of 0.

use core::ffi::c_int;

use obs_core::video_pacing::{Step, group_starts, step};
use proptest::prelude::*;

/// `1e9 * 1001 / 30000` rounded, a common interval.
const FPS30: u64 = 33_333_333;
const FPS25: u64 = 40_000_000;
/// Any frame time well away from 0.
const T0: u64 = 1_000_000_000_000;

// ---------------------------------------------------------------------------
// helpers

fn check_step(
    what: &str,
    frame_time: u64,
    interval: u64,
    slept: bool,
    now: u64,
    expected_time: u64,
    expected_count: c_int,
) {
    let got = step(frame_time, interval, slept, now);
    assert_eq!(
        got,
        Step {
            frame_time: expected_time,
            count: expected_count
        },
        "{what}"
    );
}

/// The C's step, from its expressions: `(int64_t)` of the unsigned
/// difference, the clamp, `(int)` of the quotient, and the frame time plus
/// the interval times that `int` as `uint64_t`.
#[allow(
    clippy::as_conversions,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]
fn model(frame_time: u64, interval: u64, slept: bool, now: u64) -> (u64, c_int) {
    if slept {
        return (frame_time.wrapping_add(interval), 1);
    }
    let diff = now.wrapping_sub(frame_time) as i64;
    let clamped = if diff > interval as i64 {
        diff as u64
    } else {
        interval
    };
    let Some(whole) = clamped.checked_div(interval) else {
        return (frame_time, 0);
    };
    let count = whole as c_int;
    (
        frame_time.wrapping_add(interval.wrapping_mul(i64::from(count) as u64)),
        count,
    )
}

// ---------------------------------------------------------------------------
// the frame time step

#[test]
fn steps_slept_is_one_frame() {
    check_step("30 fps", T0, FPS30, true, 0, T0 + FPS30, 1);
    check_step("25 fps", T0, FPS25, true, 0, T0 + FPS25, 1);
    check_step(
        "now ignored (late)",
        T0,
        FPS25,
        true,
        T0 + 10 * FPS25,
        T0 + FPS25,
        1,
    );
    check_step(
        "now ignored (earlier)",
        T0,
        FPS25,
        true,
        T0 - 5,
        T0 + FPS25,
        1,
    );
    check_step("frame time 0", 0, FPS25, true, 0, FPS25, 1);
    check_step("interval 1 ns", T0, 1, true, 0, T0 + 1, 1);
}

#[test]
fn steps_late_wakeup_counts_whole_intervals() {
    check_step(
        "late, less than 2 intervals",
        T0,
        FPS25,
        false,
        T0 + FPS25 + 1,
        T0 + FPS25,
        1,
    );
    check_step(
        "late, exactly 1 interval",
        T0,
        FPS25,
        false,
        T0 + FPS25,
        T0 + FPS25,
        1,
    );
    check_step(
        "late, just under 2",
        T0,
        FPS25,
        false,
        T0 + 2 * FPS25 - 1,
        T0 + FPS25,
        1,
    );
    check_step(
        "late, exactly 2",
        T0,
        FPS25,
        false,
        T0 + 2 * FPS25,
        T0 + 2 * FPS25,
        2,
    );
    check_step(
        "late, 2.5 intervals",
        T0,
        FPS25,
        false,
        T0 + 5 * FPS25 / 2,
        T0 + 2 * FPS25,
        2,
    );
    check_step(
        "late, 10.9 intervals",
        T0,
        FPS30,
        false,
        T0 + 10 * FPS30 + 9 * FPS30 / 10,
        T0 + 10 * FPS30,
        10,
    );
    check_step(
        "late, 1 s at 25 fps",
        T0,
        FPS25,
        false,
        T0 + 1_000_000_000,
        T0 + 25 * FPS25,
        25,
    );
    check_step("interval 1 ns, 7 ns late", T0, 1, false, T0 + 7, T0 + 7, 7);
}

#[test]
fn steps_short_or_negative_difference_is_one_interval() {
    check_step("same time", T0, FPS25, false, T0, T0 + FPS25, 1);
    check_step(
        "half an interval",
        T0,
        FPS25,
        false,
        T0 + FPS25 / 2,
        T0 + FPS25,
        1,
    );
    check_step("1 ns earlier", T0, FPS25, false, T0 - 1, T0 + FPS25, 1);
    check_step(
        "1 s earlier",
        T0,
        FPS25,
        false,
        T0 - 1_000_000_000,
        T0 + FPS25,
        1,
    );
    // now - frame_time wraps to at least 2^63: negative as int64, so one
    // interval.
    check_step("now 0", T0, FPS25, false, 0, T0 + FPS25, 1);
    check_step("difference 2^63", 0, FPS25, false, 1 << 63, FPS25, 1);
}

/// Characterized, not endorsed: a count above `INT_MAX` is truncated to
/// `int`, and the frame time wraps.
#[test]
#[allow(
    clippy::as_conversions,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]
fn steps_large_jumps_wrap_the_count() {
    let max = c_int::MAX as u64;
    check_step(
        "INT_MAX intervals",
        0,
        FPS25,
        false,
        max * FPS25,
        max * FPS25,
        c_int::MAX,
    );
    check_step(
        "INT_MAX intervals and a bit",
        0,
        FPS25,
        false,
        max * FPS25 + FPS25 / 2,
        max * FPS25,
        c_int::MAX,
    );
    check_step(
        "INT_MAX + 1 intervals",
        0,
        FPS25,
        false,
        (max + 1) * FPS25,
        (i64::from(c_int::MIN) as u64).wrapping_mul(FPS25),
        c_int::MIN,
    );
    check_step("2^32 intervals", 0, 1, false, 1 << 32, 0, 0);
    check_step("2^32 + 3 intervals", 0, 1, false, (1 << 32) + 3, 3, 3);
}

// ---------------------------------------------------------------------------
// the encoder group start

#[test]
fn steps_group_starts_once_all_encoders_started() {
    assert!(group_starts(2, 2, 0));
    assert!(group_starts(1, 1, 0));
    assert!(!group_starts(1, 2, 0));
    assert!(!group_starts(0, 2, 0));
    assert!(!group_starts(2, 2, T0));
    assert!(!group_starts(2, 2, 1));
    assert!(!group_starts(1, 2, T0));
    assert!(group_starts(3, 2, 0));
    assert!(group_starts(0, 0, 0));
    assert!(group_starts(u32::MAX, u32::MAX as usize, 0));
}

// ---------------------------------------------------------------------------
// edges

/// Intentional difference, unreachable in libobs: an interval of 0 after a
/// late wake-up (the C divides by zero) gives a count of 0 and the frame time
/// as it is. After a sleep it is the frame time and 1, as in the C.
#[test]
fn edge_interval_zero() {
    for now in [0, 1, T0, T0 + 1, u64::MAX] {
        assert_eq!(
            step(T0, 0, false, now),
            Step {
                frame_time: T0,
                count: 0
            }
        );
    }
    assert_eq!(
        step(T0, 0, true, 0),
        Step {
            frame_time: T0,
            count: 1
        }
    );
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4096))]

    /// Any frame time, interval (not 0), sleep result and clock value.
    #[test]
    fn edge_step_matches_the_model(
        frame_time in any::<u64>(),
        interval in 1..=u64::MAX,
        slept in any::<bool>(),
        now in any::<u64>(),
    ) {
        let (time, count) = model(frame_time, interval, slept, now);
        prop_assert_eq!(step(frame_time, interval, slept, now), Step { frame_time: time, count });
    }

    /// Realistic intervals (1 to 1000 fps) and wake-ups up to a day late.
    #[test]
    fn edge_late_wakeups_stay_on_the_grid(
        frame_time in 0..u64::MAX / 2,
        interval in 1_000_000..=1_000_000_000_u64,
        late in 0..86_400_000_000_000_u64,
    ) {
        let got = step(frame_time, interval, false, frame_time + late);
        prop_assert!(got.count >= 1);
        let steps = got.frame_time - frame_time;
        prop_assert_eq!(steps % interval, 0);
        prop_assert_eq!(steps / interval, u64::try_from(got.count).unwrap_or(0));
        prop_assert!(got.frame_time <= frame_time + late.max(interval));
    }

    /// Any group counts and start timestamp.
    #[test]
    fn edge_group_starts_matches_the_model(
        started in any::<u32>(),
        encoders in any::<usize>(),
        start_timestamp in prop_oneof![Just(0_u64), any::<u64>()],
    ) {
        let expected = u64::from(started) >= encoders as u64 && start_timestamp == 0;
        prop_assert_eq!(group_starts(started, encoders, start_timestamp), expected);
    }
}
