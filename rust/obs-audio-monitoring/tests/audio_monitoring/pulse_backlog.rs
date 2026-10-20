//! Tier 1 mirror of `test/cmocka/test_pulse_monitor_backlog.c`, plus edge
//! cases of the window itself.
//!
//! The cases drive the detector with the C test's model of the queue: a base
//! that moves the way it does in normal playback, plus a backlog that a stall
//! or clock drift adds and that only a drop removes.

use obs_audio_monitoring::Backlog;
use obs_audio_monitoring::pulse_stream_flags::{BACKLOG_SLACK_USEC, BACKLOG_WINDOW_NS};

const MS: u64 = 1000; // usec
const NS_PER_USEC: u64 = 1000;
const PACKET_NS: u64 = 21_333_333; // 1024 frames at 48 kHz
const SECOND_NS: u64 = 1_000_000_000;

#[derive(Default)]
struct Model {
    /// Queued audio without any backlog at time t (ns), in usec.
    base: Option<fn(u64) -> u64>,
    target: u64,
    /// Backlog added at `stall_at` (0: none).
    stall_at: u64,
    stall_backlog: u64,
    /// Backlog that drains by itself within `drain_ns` after `stall_at`;
    /// 0: it stays until dropped.
    drain_ns: u64,
    /// Backlog added per second of playback (clock drift), in usec.
    drift_per_s: u64,

    backlog: u64,
    dropped: u64,
    drops: u32,
    /// Drops that would have emptied the queue.
    underruns: u32,
    max_queued_after: u64,
    /// ns; `max_queued_after` counts from here.
    check_after: u64,
}

impl Model {
    fn backlog_at(&mut self, t: u64, prev_t: u64) -> u64 {
        if self.drift_per_s != 0 {
            self.backlog += self.drift_per_s * (t - prev_t) / SECOND_NS;
        }
        if self.stall_at != 0 && prev_t < self.stall_at && t >= self.stall_at {
            self.backlog += self.stall_backlog;
        }
        if self.drain_ns != 0 && t >= self.stall_at && self.backlog != 0 {
            let rate = self.stall_backlog * PACKET_NS / self.drain_ns;
            self.backlog = self.backlog.saturating_sub(rate);
        }
        self.backlog
    }

    fn run(&mut self, duration_ns: u64) {
        let base = self.base.expect("model base");
        let mut backlog = Backlog::default();
        let mut prev_t = 0;
        let mut t = PACKET_NS;
        while t <= duration_ns {
            let mut queued = base(t) + self.backlog_at(t, prev_t);
            prev_t = t;

            let drop = backlog.excess(t, queued, self.target);
            if drop != 0 {
                self.drops += 1;
                self.dropped += drop;
                if drop >= queued {
                    self.underruns += 1;
                }
                self.backlog -= drop.min(self.backlog);
                queued = queued.saturating_sub(drop);
            }
            if t >= self.check_after && queued > self.max_queued_after {
                self.max_queued_after = queued;
            }
            t += PACKET_NS;
        }
    }
}

/// Normal playback: the queue moves between 100 and 160 ms, target 85 ms.
const STEADY_MAX: u64 = 160 * MS;

fn steady(t: u64) -> u64 {
    let phase = (t / NS_PER_USEC) % (500 * MS);
    let tri = if phase < 250 * MS {
        phase
    } else {
        500 * MS - phase
    };
    100 * MS + tri * 60 / 250
}

/// A source that delivers 1 s of audio at once, every second.
fn bursty(t: u64) -> u64 {
    let phase = (t / NS_PER_USEC) % (1000 * MS);
    1000 * MS - phase + 5 * MS
}

#[test]
fn steady_playback_is_not_cut() {
    let mut m = Model {
        base: Some(steady),
        target: 85 * MS,
        ..Model::default()
    };
    m.run(600 * SECOND_NS);
    assert_eq!(m.drops, 0);
}

#[test]
fn stall_backlog_is_dropped() {
    let mut m = Model {
        base: Some(steady),
        target: 85 * MS,
        stall_at: 20 * SECOND_NS,
        stall_backlog: 3200 * MS,
        check_after: 25 * SECOND_NS,
        ..Model::default()
    };
    m.run(60 * SECOND_NS);
    assert_eq!(m.underruns, 0);
    assert!(m.backlog < 100 * MS, "backlog left {} ms", m.backlog / MS);
    assert!(m.max_queued_after <= STEADY_MAX + 100 * MS);
}

#[test]
fn short_stall_backlog_is_dropped() {
    let mut m = Model {
        base: Some(steady),
        target: 85 * MS,
        stall_at: 20 * SECOND_NS,
        stall_backlog: 600 * MS,
        check_after: 25 * SECOND_NS,
        ..Model::default()
    };
    m.run(60 * SECOND_NS);
    assert_eq!(m.underruns, 0);
    assert!(m.max_queued_after <= STEADY_MAX + 100 * MS);
}

#[test]
fn drift_stays_bounded() {
    let mut m = Model {
        base: Some(steady),
        target: 85 * MS,
        drift_per_s: 500,
        check_after: 60 * SECOND_NS,
        ..Model::default()
    };
    m.run(900 * SECOND_NS);
    assert_eq!(m.underruns, 0);
    assert!(m.max_queued_after <= STEADY_MAX + 85 * MS + BACKLOG_SLACK_USEC);
}

#[test]
fn bursty_source_is_not_cut() {
    let mut m = Model {
        base: Some(bursty),
        target: 1000 * MS,
        ..Model::default()
    };
    m.run(300 * SECOND_NS);
    assert_eq!(m.drops, 0);
}

#[test]
fn spike_that_drains_is_not_cut() {
    let mut m = Model {
        base: Some(steady),
        target: 85 * MS,
        stall_at: 20 * SECOND_NS,
        stall_backlog: 1000 * MS,
        drain_ns: 1_500_000_000,
        ..Model::default()
    };
    m.run(60 * SECOND_NS);
    assert_eq!(m.drops, 0);
}

#[test]
fn backlog_constants() {
    assert_eq!(BACKLOG_WINDOW_NS, 2_000_000_000);
    assert_eq!(BACKLOG_SLACK_USEC, 200_000);
}

/// The first sample opens a window and drops nothing.
#[test]
fn first_sample_opens_the_window() {
    let mut b = Backlog::default();
    assert_eq!(b.excess(5, 900 * MS, 85 * MS), 0);
    assert_eq!(
        b,
        Backlog {
            window_start: 5,
            window_min: 900 * MS
        }
    );
}

/// The window closes on the first sample at least `BACKLOG_WINDOW_NS` after
/// its start, and that sample counts toward the minimum.
#[test]
fn window_closes_at_its_length() {
    let mut b = Backlog::default();
    assert_eq!(b.excess(1, 900 * MS, 85 * MS), 0);
    assert_eq!(b.excess(BACKLOG_WINDOW_NS, 900 * MS, 85 * MS), 0);
    assert_eq!(b.window_start, 1, "one ns short keeps the window open");
    assert_eq!(b.excess(BACKLOG_WINDOW_NS + 1, 700 * MS, 85 * MS), 615 * MS);
    assert_eq!(b.window_start, 0, "the next sample starts a new window");
    assert_eq!(b.window_min, 700 * MS);
}

/// The minimum is compared against the target plus a slack of at least
/// `BACKLOG_SLACK_USEC`, or the target itself when that is larger.
#[test]
fn slack_is_the_larger_of_target_and_constant() {
    let close = |b: &mut Backlog, queued: u64, target: u64| {
        *b = Backlog::default();
        b.excess(1, queued, target);
        b.excess(1 + BACKLOG_WINDOW_NS, queued, target)
    };
    let mut b = Backlog::default();
    // Small target: slack is 200 ms.
    assert_eq!(close(&mut b, 85 * MS + BACKLOG_SLACK_USEC, 85 * MS), 0);
    assert_eq!(
        close(&mut b, 85 * MS + BACKLOG_SLACK_USEC + 1, 85 * MS),
        BACKLOG_SLACK_USEC + 1
    );
    // Large target: slack is the target.
    assert_eq!(close(&mut b, 2000 * MS, 1000 * MS), 0);
    assert_eq!(close(&mut b, 2000 * MS + 1, 1000 * MS), 1000 * MS + 1);
    // Exactly the constant.
    assert_eq!(close(&mut b, 2 * BACKLOG_SLACK_USEC, BACKLOG_SLACK_USEC), 0);
    assert_eq!(
        close(&mut b, 2 * BACKLOG_SLACK_USEC + 1, BACKLOG_SLACK_USEC),
        BACKLOG_SLACK_USEC + 1
    );
}

/// Characterized, not endorsed: a sample at `now_ns == 0` is stored as an
/// open window's start of 0, which reads as "no window", so the next sample
/// opens a new one.
#[test]
fn time_zero_does_not_open_a_window() {
    let mut b = Backlog::default();
    assert_eq!(b.excess(0, 900 * MS, 85 * MS), 0);
    assert_eq!(b.excess(3 * BACKLOG_WINDOW_NS, 100 * MS, 85 * MS), 0);
    assert_eq!(
        b,
        Backlog {
            window_start: 3 * BACKLOG_WINDOW_NS,
            window_min: 100 * MS
        }
    );
}

/// Characterized, not endorsed: the C arithmetic is unsigned and wraps. A
/// clock that steps back looks like a very long window, and a target near
/// `u64::MAX` wraps `target + slack`.
#[test]
fn unsigned_arithmetic_wraps_like_c() {
    let mut b = Backlog::default();
    b.excess(10 * SECOND_NS, 900 * MS, 85 * MS);
    assert_eq!(b.excess(SECOND_NS, 900 * MS, 85 * MS), 815 * MS);

    let mut b = Backlog::default();
    let target = u64::MAX - 10;
    b.excess(1, 5, target);
    // target + target wraps to u64::MAX - 21, so min 5 is not above it.
    assert_eq!(b.excess(1 + BACKLOG_WINDOW_NS, 5, target), 0);

    let mut b = Backlog::default();
    let target = u64::MAX / 2 + 1;
    b.excess(1, 3, target);
    // target + target wraps to 0: min 3 is above it, and 3 - target wraps.
    assert_eq!(
        b.excess(1 + BACKLOG_WINDOW_NS, 3, target),
        3u64.wrapping_sub(target)
    );
}
