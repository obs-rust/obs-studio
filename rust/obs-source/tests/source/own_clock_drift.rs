//! Tier 1: `obs_source::own_clock_drift`, the clock drift error of audio on
//! a clock of its own (#4182).
//!
//! New code with no C original, so there is no Tier 3 case: the error is
//! not wired into `source_output_audio_data()` yet (PR 2 of #4182).
//!
//! - `gap_*`: [`buffered_end_gap`] on plain values and its edges.
//! - `filter_*`: the start window, the baseline, the sliding minimum and
//!   [`OwnClockDrift::reset`].
//! - `loop_*`: the filter driving the resampler's loop filter
//!   (`obs_media_io::audio_resampler_drift`) in simulated time. A sender
//!   delivers 100 ms packets that arrive up to 20 ms late. The resampler
//!   shortens or stretches each packet by the loop's correction, and the
//!   packets are appended to a buffer that plays at the system rate. The
//!   cases check that the gap stays under the 70 ms step of #4143, and that
//!   it settles. No real time passes: 30 simulated minutes take
//!   milliseconds.

use obs_media_io::audio_resampler_drift::{self as drift, Compensation};
use obs_source::own_clock_drift::{
    BUCKET_NS, OwnClockDrift, OwnClockDriftRaw, WINDOW_NS, buffered_end_gap, excess,
};

const MS: i64 = 1_000_000;
const SEC: u64 = 1_000_000_000;
/// The system clock somewhere in a long session.
const OS: u64 = 5_000 * SEC;

#[test]
fn gap_is_buffer_end_less_now() {
    // 4800 frames at 48 kHz are 100 ms.
    assert_eq!(buffered_end_gap(OS, 4800, 48_000, OS), 100 * MS);
    assert_eq!(
        buffered_end_gap(OS, 4800, 48_000, OS + 40 * SEC / 1000),
        60 * MS
    );
    assert_eq!(buffered_end_gap(OS, 0, 48_000, OS), 0);
}

#[test]
fn gap_is_negative_when_the_buffer_ends_before_now() {
    assert_eq!(buffered_end_gap(OS, 4800, 48_000, OS + SEC), -900 * MS);
    assert_eq!(buffered_end_gap(OS, 0, 48_000, OS + 1), -1);
}

#[test]
fn gap_with_rate_zero_counts_no_frames() {
    assert_eq!(buffered_end_gap(OS, 4800, 0, OS), 0);
    assert_eq!(buffered_end_gap(OS + 5, u64::MAX, 0, OS), 5);
}

#[test]
fn gap_saturates() {
    assert_eq!(buffered_end_gap(u64::MAX, u64::MAX, 1, 0), i64::MAX);
    assert_eq!(buffered_end_gap(0, 0, 48_000, u64::MAX), i64::MIN);
}

#[test]
fn filter_holds_until_the_start_window_has_passed() {
    let mut f = OwnClockDrift::new();
    for i in 0..8 {
        assert_eq!(f.sample(OS + i * BUCKET_NS, 100 * MS), None, "sample {i}");
    }
    assert_eq!(f.sample(OS + WINDOW_NS, 100 * MS), Some(0));
}

#[test]
fn filter_baseline_is_the_start_windows_minimum() {
    let mut f = OwnClockDrift::new();
    let _ = f.sample(OS, 130 * MS);
    let _ = f.sample(OS + 300 * MS.unsigned_abs(), 100 * MS);
    let _ = f.sample(OS + 600 * MS.unsigned_abs(), 120 * MS);
    // The baseline is 100 ms; the window still holds the 100 ms sample.
    assert_eq!(f.sample(OS + WINDOW_NS, 125 * MS), Some(0));
    // A second later the window holds only 125 ms and 140 ms.
    assert_eq!(f.sample(OS + 2 * WINDOW_NS - 1, 140 * MS), Some(25 * MS));
}

#[test]
fn filter_follows_growth_and_ignores_late_packets() {
    let mut f = OwnClockDrift::new();
    let mut t = OS;
    let mut gap = 100 * MS;
    let mut last = None;
    // 10 s of 100 ms packets, the gap 1 ms longer each packet; every third
    // packet 20 ms late (a 20 ms shorter gap).
    for i in 0..100 {
        let late = if i % 3 == 0 { 20 * MS } else { 0 };
        last = f.sample(t, gap - late);
        t += 100 * MS.unsigned_abs();
        gap += MS;
    }
    // The window holds the packets from 9.0 s on; its minimum is the late
    // packet at 9.0 s (190 ms less 20 ms). The baseline is the late packet at
    // 0 s (100 ms less 20 ms).
    assert_eq!(last, Some(170 * MS - 80 * MS));
}

#[test]
fn filter_follows_a_shrinking_gap() {
    let mut f = OwnClockDrift::new();
    let mut t = OS;
    let mut last = None;
    for i in 0..30 {
        last = f.sample(t, 100 * MS - i * 2 * MS);
        t += 100 * MS.unsigned_abs();
    }
    // The newest sample (42 ms at 2.9 s) is the window's minimum; the
    // baseline is the start window's minimum, 82 ms at 0.9 s.
    assert_eq!(last, Some(42 * MS - 82 * MS));
}

#[test]
fn filter_ignores_a_sample_older_than_the_window() {
    let mut f = OwnClockDrift::new();
    let _ = f.sample(OS, 100 * MS);
    assert_eq!(f.sample(OS + WINDOW_NS, 100 * MS), Some(0));
    assert_eq!(f.sample(OS - WINDOW_NS, -500 * MS), None);
    assert_eq!(f.sample(OS + WINDOW_NS + 1, 100 * MS), Some(0));
}

#[test]
fn filter_reset_forgets_the_baseline() {
    let mut f = OwnClockDrift::new();
    let _ = f.sample(OS, 100 * MS);
    assert_eq!(f.sample(OS + WINDOW_NS, 100 * MS), Some(0));
    f.reset();
    assert_eq!(f, OwnClockDrift::new());
    assert_eq!(f.sample(OS + 2 * WINDOW_NS, 300 * MS), None);
    assert_eq!(f.sample(OS + 3 * WINDOW_NS, 300 * MS), Some(0));
}

/// All zeroes is a filter with no samples, as a zeroed `struct obs_source`
/// holds it.
#[test]
fn raw_zeroes_are_a_new_filter() {
    assert_eq!(
        OwnClockDrift::from_raw(&OwnClockDriftRaw::default()),
        OwnClockDrift::new()
    );
    assert_eq!(OwnClockDrift::new().to_raw(), OwnClockDriftRaw::default());
}

/// The filter keeps its state through the C struct, at every stage: empty,
/// starting, with its baseline, and with buckets of an older window.
#[test]
fn raw_round_trips() {
    let mut f = OwnClockDrift::new();
    let mut times = vec![OS, OS + 40_000_000, OS + WINDOW_NS, OS + 3 * WINDOW_NS];
    times.extend((0..20).map(|i| OS + 4 * WINDOW_NS + i * BUCKET_NS / 2));
    for (i, os) in times.into_iter().enumerate() {
        let gap = i64::try_from(i).unwrap() * MS - 5 * MS;
        let mut through_raw = OwnClockDrift::from_raw(&f.to_raw());
        assert_eq!(through_raw, f);
        assert_eq!(through_raw.sample(os, gap), f.sample(os, gap));
        assert_eq!(through_raw, f);
    }
}

/// A flag that is clear hides its field: a stale value there is not read.
#[test]
fn raw_fields_without_their_flag_are_ignored() {
    let mut raw = OwnClockDriftRaw {
        start: 7,
        start_min: -3,
        baseline: 9,
        ..OwnClockDriftRaw::default()
    };
    raw.buckets[2].id = 5;
    raw.buckets[2].min = 1;
    assert_eq!(OwnClockDrift::from_raw(&raw), OwnClockDrift::new());
}

/// How much more audio the buffer holds than right after a placement.
#[test]
fn excess_is_the_gap_less_the_placement_offset() {
    assert_eq!(excess(100 * MS, 40 * MS, 0), 60 * MS);
    assert_eq!(excess(100 * MS, 40 * MS, 5_000_000), 65 * MS);
    assert_eq!(excess(100 * MS, -40 * MS, 0), 140 * MS);
    assert_eq!(excess(i64::MIN, 1, 0), i64::MIN);
    assert_eq!(excess(i64::MAX, -1, 0), i64::MAX);
    assert_eq!(excess(0, 0, u64::MAX), i64::MAX);
    // each step saturates: MIN - 1 stays MIN, then + 10
    assert_eq!(excess(i64::MIN, 1, 10), i64::MIN + 10);
}

#[test]
fn filter_survives_extreme_values() {
    let mut f = OwnClockDrift::new();
    let _ = f.sample(0, i64::MIN);
    let _ = f.sample(u64::MAX / 2, i64::MAX);
    let _ = f.sample(u64::MAX, i64::MAX);
    let _ = f.sample(u64::MAX, i64::MIN);
}

/// The result of one simulated run.
struct Run {
    /// The largest gap over the start window's minimum, after the start
    /// window.
    max_dev_ns: i64,
    /// The smallest such gap.
    min_dev_ns: i64,
    /// The largest absolute filter error over the last 5 minutes.
    settled_err_ns: i64,
}

/// A sender whose clock runs `ppm` parts per million fast, for `minutes`.
/// Its 100 ms packets arrive up to `jitter_ms` late (a fixed pseudo-random
/// series). With `compensate`, the filter's error drives the loop filter;
/// the resampler changes each packet by the correction `delta` gives, as
/// `swr_set_compensation()` does.
#[allow(
    clippy::as_conversions,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)] // simulated times and sample counts, far inside f64's exact range
fn simulate(ppm: f64, jitter_ms: u64, minutes: u64, compensate: bool) -> Run {
    const RATE: u32 = 48_000;
    const PACKET: u32 = 4_800;
    let packets = minutes * 600;
    let mut comp = Compensation::default();
    let mut filter = OwnClockDrift::new();
    let mut appended = 0.0_f64;
    let mut seed = 0x2545_f491_4f6c_dd1d_u64;
    let start = OS as f64;
    let mut base: Option<i64> = None;
    let mut first_min = i64::MAX;
    let mut run = Run {
        max_dev_ns: i64::MIN,
        min_dev_ns: i64::MAX,
        settled_err_ns: 0,
    };

    for k in 0..packets {
        // The packet's last sample, in system time, then its arrival.
        let sent = start + (k + 1) as f64 * 1e8 / (1.0 + ppm * 1e-6);
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        let late = if jitter_ms == 0 {
            0.0
        } else {
            (seed % (jitter_ms * 1_000_000)) as f64
        };
        let arrival = (sent + late) as u64;

        let out = if comp.enabled {
            let _ = drift::step(&mut comp, RATE, PACKET);
            f64::from(PACKET)
                * (1.0 + f64::from(drift::delta(&comp, RATE)) / f64::from(drift::DISTANCE))
        } else {
            f64::from(PACKET)
        };
        appended += out;
        let buffered = appended.round() as u64;
        let gap = buffered_end_gap(OS, buffered, RATE, arrival);

        let since = arrival - OS;
        if since < WINDOW_NS {
            first_min = first_min.min(gap);
        } else {
            let b = *base.get_or_insert(first_min);
            run.max_dev_ns = run.max_dev_ns.max(gap - b);
            run.min_dev_ns = run.min_dev_ns.min(gap - b);
        }

        if let Some(e) = filter.sample(arrival, gap) {
            if compensate {
                drift::set_error(&mut comp, e);
            }
            if since + 300 * SEC >= minutes * 60 * SEC {
                run.settled_err_ns = run.settled_err_ns.max(e.abs());
            }
        }
    }
    run
}

/// The step threshold of #4143 (`TS_SMOOTHING_THRESHOLD`).
const STEP_NS: i64 = 70 * MS;
/// The largest filter error once settled: the arrival jitter moves the
/// window's minimum by up to about 14 ms against the baseline.
const SETTLED_NS: i64 = 15 * MS;

#[test]
fn loop_without_compensation_drifts_past_the_step() {
    // The control: the model sees the drift that #4143 steps over.
    let run = simulate(500.0, 20, 30, false);
    assert!(run.max_dev_ns > STEP_NS, "max {} ms", run.max_dev_ns / MS);
}

#[test]
fn loop_holds_a_fast_sender_under_the_step() {
    let run = simulate(500.0, 20, 30, true);
    assert!(run.max_dev_ns < STEP_NS, "max {} ms", run.max_dev_ns / MS);
    assert!(
        run.settled_err_ns <= SETTLED_NS,
        "settled {} ms",
        run.settled_err_ns / MS
    );
}

#[test]
fn loop_holds_a_slow_sender_over_minus_the_step() {
    let run = simulate(-500.0, 20, 30, true);
    // Late packets shorten the gap by up to the jitter.
    assert!(
        run.min_dev_ns > -STEP_NS - 20 * MS,
        "min {} ms",
        run.min_dev_ns / MS
    );
    assert!(
        run.settled_err_ns <= SETTLED_NS,
        "settled {} ms",
        run.settled_err_ns / MS
    );
}

#[test]
fn loop_leaves_a_sender_on_the_system_clock_alone() {
    let run = simulate(0.0, 20, 10, true);
    // The raw gap moves by the arrival jitter (up to 20 ms late); the filter
    // takes that out, so the loop does not follow it.
    assert!(run.max_dev_ns < 25 * MS, "max {} ms", run.max_dev_ns / MS);
    assert!(
        run.settled_err_ns <= SETTLED_NS,
        "settled {} ms",
        run.settled_err_ns / MS
    );
}

#[test]
fn loop_beyond_the_clamp_still_needs_the_step() {
    // 1200 ppm is past the loop's 1000 ppm clamp: the gap runs away, and
    // the step of #4143 stays the bound there.
    let run = simulate(1200.0, 20, 30, true);
    assert!(run.max_dev_ns > STEP_NS, "max {} ms", run.max_dev_ns / MS);
}
