//! Tier 1: `obs_source::audio_timing`, the source audio timestamp
//! placement.
//!
//! The `steps_*` cases port `test/cmocka/test_source_audio_timing_steps.c`
//! 1:1, under the same case names, in the same order, with the same inputs
//! and expectations. The C test also checks that each step writes its
//! numeric outputs and leaves its inputs alone; the safe API takes its
//! inputs by value and returns every output, so there is nothing to check
//! there (Tier 3 checks it for the shims, byte for byte).
//! [`Fake`] repeats the C test's fake source: the fields the steps read and
//! write, and the model of `source_output_audio_data()` around them
//! (`handle_ts_jump()`, and the buffer start of
//! `source_output_audio_place()`).
//!
//! The `edge_*` cases are new: the distance, every step over the special
//! values of the full `u64` and `i64` ranges with every flag (no panic,
//! and what each action implies), and proptests of the same over random
//! values.

use obs_source::audio_timing::{
    Action, MAX_TS_VAR, PreLock, PreLockInput, TS_SMOOTHING_THRESHOLD, UnderLock, UnderLockInput,
    buffer_start, distance, pre_lock, under_lock,
};
use proptest::prelude::*;

const MS: u64 = 1_000_000;
const SMOOTH: u64 = TS_SMOOTHING_THRESHOLD;
const MAX_VAR: u64 = MAX_TS_VAR;
/// The system clock, far from the media timestamps below.
const OS: u64 = 1_000_000_000_000_000;
/// A media file's timestamp, more than `MAX_TS_VAR` from `OS`.
const TS: u64 = 5_000_000_000;
/// One 1024-frame packet at 48 kHz.
const DUR: u64 = 21_333_333;
/// A media source's `timing_adjust`.
const ADJ: u64 = OS - TS;

use Action::{Exceeded, Jump, Keep, Reset, Smooth};

fn ms(n: i64) -> i64 {
    n * 1_000_000
}

/// The outputs of [`pre_lock`] with no flag, no adjust, no diff and no
/// drift.
fn out(action: Action, timestamp: u64, next_audio_ts_min: u64) -> PreLock {
    PreLock {
        action,
        direct: false,
        reanchor: false,
        continuous: false,
        timestamp,
        next_audio_ts_min,
        timing_adjust: 0,
        diff: 0,
        drift_error: 0,
    }
}

fn smoothed(timestamp: u64, next_audio_ts_min: u64, drift_error: i64) -> PreLock {
    PreLock {
        continuous: true,
        drift_error,
        ..out(Smooth, timestamp, next_audio_ts_min)
    }
}

#[track_caller]
fn check_pre(input: PreLockInput, expected: PreLock) {
    assert_eq!(pre_lock(input), expected, "{input:?}");
}

fn media(timestamp: u64) -> PreLockInput {
    PreLockInput {
        timestamp,
        os_time: OS,
        duration: DUR,
        timing_set: true,
        ..PreLockInput::default()
    }
}

// --------------------------------------------------------------------------
// obs_source_audio_timing_pre_lock

/// `pre_direct_timestamps`
#[test]
fn steps_pre_direct_timestamps() {
    let mut input = PreLockInput {
        timestamp: OS + 5 * MS,
        os_time: OS,
        duration: DUR,
        ..PreLockInput::default()
    };
    let direct = |ts: u64| PreLock {
        direct: true,
        ..out(Keep, ts, ts + DUR)
    };
    check_pre(input, direct(OS + 5 * MS));
    input.timing_set = true;
    check_pre(input, direct(OS + 5 * MS));
    input.timestamp = OS - 1500 * MS;
    check_pre(input, direct(OS - 1500 * MS));
    input.timestamp = OS;
    check_pre(input, direct(OS));
}

/// `pre_direct_at_max_ts_var`
#[test]
fn steps_pre_direct_at_max_ts_var() {
    for offset in [MAX_VAR - 1, MAX_VAR, MAX_VAR + 1] {
        let direct = offset < MAX_VAR;
        for ts in [OS + offset, OS - offset] {
            let input = PreLockInput {
                timestamp: ts,
                os_time: OS,
                duration: DUR,
                ..PreLockInput::default()
            };
            let expected = if direct {
                PreLock {
                    direct: true,
                    ..out(Keep, ts, ts + DUR)
                }
            } else {
                PreLock {
                    timing_adjust: OS.wrapping_sub(ts),
                    ..out(Reset, ts, ts + DUR)
                }
            };
            check_pre(input, expected);
        }
    }
}

/// `pre_reset_when_timing_not_set`
#[test]
fn steps_pre_reset_when_timing_not_set() {
    let mut input = PreLockInput {
        timestamp: TS,
        os_time: OS,
        duration: DUR,
        ..PreLockInput::default()
    };
    let reset = PreLock {
        timing_adjust: OS - TS,
        ..out(Reset, TS, TS + DUR)
    };
    check_pre(input, reset);
    input.next_audio_ts_min = TS + 1;
    check_pre(input, reset);
    input.next_audio_ts_min = TS + 10 * MAX_VAR;
    check_pre(input, reset);
    input.async_unbuffered = true;
    input.async_decoupled = true;
    check_pre(input, reset);
}

/// `pre_reset_ahead_of_clock_wraps` (characterized, not endorsed)
#[test]
fn steps_pre_reset_ahead_of_clock_wraps() {
    let input = PreLockInput {
        timestamp: OS + 3 * MAX_VAR,
        os_time: OS,
        duration: DUR,
        ..PreLockInput::default()
    };
    check_pre(
        input,
        PreLock {
            timing_adjust: u64::MAX - 3 * MAX_VAR + 1,
            ..out(Reset, OS + 3 * MAX_VAR, OS + 3 * MAX_VAR + DUR)
        },
    );
}

/// `pre_keep_when_next_is_zero`
#[test]
fn steps_pre_keep_when_next_is_zero() {
    let mut input = media(TS);
    check_pre(input, out(Keep, TS, TS + DUR));
    input.async_unbuffered = true;
    input.async_decoupled = true;
    check_pre(input, out(Keep, TS, TS + DUR));
    input.timestamp = 0;
    check_pre(input, out(Keep, 0, DUR));
}

/// `pre_smooth`
#[test]
fn steps_pre_smooth() {
    let mut input = media(TS);
    input.next_audio_ts_min = TS;
    check_pre(input, smoothed(TS, TS + DUR, 0));
    input.next_audio_ts_min = TS + 3 * MS;
    check_pre(input, smoothed(TS + 3 * MS, TS + 3 * MS + DUR, ms(3)));
    input.next_audio_ts_min = TS - 3 * MS;
    check_pre(input, smoothed(TS - 3 * MS, TS - 3 * MS + DUR, ms(-3)));
    input.async_unbuffered = true;
    check_pre(input, smoothed(TS - 3 * MS, TS - 3 * MS + DUR, ms(-3)));
    input.async_unbuffered = false;
    input.async_decoupled = true;
    check_pre(input, smoothed(TS - 3 * MS, TS - 3 * MS + DUR, ms(-3)));
}

/// `pre_at_smoothing_threshold`
#[test]
fn steps_pre_at_smoothing_threshold() {
    let mut input = media(TS);
    let exceeded = |diff: u64| PreLock {
        diff,
        ..out(Exceeded, TS, TS + DUR)
    };
    let edge = i64::try_from(SMOOTH - 1).unwrap();

    input.next_audio_ts_min = TS + SMOOTH - 1;
    check_pre(
        input,
        smoothed(TS + SMOOTH - 1, TS + SMOOTH - 1 + DUR, edge),
    );
    input.next_audio_ts_min = TS - (SMOOTH - 1);
    check_pre(
        input,
        smoothed(TS - (SMOOTH - 1), TS - (SMOOTH - 1) + DUR, -edge),
    );

    input.next_audio_ts_min = TS + SMOOTH;
    check_pre(input, exceeded(SMOOTH));
    input.next_audio_ts_min = TS - SMOOTH;
    check_pre(input, exceeded(SMOOTH));

    input.next_audio_ts_min = TS + SMOOTH + 1;
    check_pre(input, exceeded(SMOOTH + 1));
    input.next_audio_ts_min = TS - SMOOTH - 1;
    check_pre(input, exceeded(SMOOTH + 1));
}

/// `pre_at_max_ts_var`
#[test]
fn steps_pre_at_max_ts_var() {
    let mut input = media(TS);
    let exceeded = |diff: u64| PreLock {
        diff,
        ..out(Exceeded, TS, TS + DUR)
    };
    let jump = |diff: u64| PreLock {
        diff,
        timing_adjust: OS - TS,
        ..out(Jump, TS, TS + DUR)
    };

    input.next_audio_ts_min = TS + MAX_VAR - 1;
    check_pre(input, exceeded(MAX_VAR - 1));
    input.next_audio_ts_min = TS + MAX_VAR;
    check_pre(input, exceeded(MAX_VAR));
    input.next_audio_ts_min = TS - MAX_VAR;
    check_pre(input, exceeded(MAX_VAR));

    input.next_audio_ts_min = TS + MAX_VAR + 1;
    check_pre(input, jump(MAX_VAR + 1));
    input.next_audio_ts_min = TS - MAX_VAR - 1;
    check_pre(input, jump(MAX_VAR + 1));
    input.next_audio_ts_min = TS + 1000 * MAX_VAR;
    input.async_unbuffered = true;
    input.async_decoupled = true;
    check_pre(input, jump(1000 * MAX_VAR));
}

/// `pre_direct_never_jumps`
#[test]
fn steps_pre_direct_never_jumps() {
    let mut input = media(OS);
    input.next_audio_ts_min = OS + 3 * MAX_VAR;
    check_pre(
        input,
        PreLock {
            direct: true,
            diff: 3 * MAX_VAR,
            ..out(Exceeded, OS, OS + DUR)
        },
    );
    input.next_audio_ts_min = OS + 2 * MS;
    let direct_smoothed = PreLock {
        direct: true,
        ..smoothed(OS + 2 * MS, OS + 2 * MS + DUR, ms(2))
    };
    check_pre(input, direct_smoothed);
    input.timing_set = false;
    check_pre(input, direct_smoothed);
}

/// `pre_async_decoupled_reanchor`
#[test]
fn steps_pre_async_decoupled_reanchor() {
    let mut input = PreLockInput {
        timestamp: TS,
        os_time: OS,
        duration: DUR,
        timing_set: true,
        next_audio_ts_min: TS + 4 * MS,
        async_unbuffered: true,
        async_decoupled: true,
    };
    check_pre(
        input,
        PreLock {
            reanchor: true,
            timing_adjust: OS - TS,
            ..smoothed(TS + 4 * MS, TS + 4 * MS + DUR, ms(4))
        },
    );
    input.timestamp = OS - MS;
    input.next_audio_ts_min = OS;
    check_pre(
        input,
        PreLock {
            direct: true,
            reanchor: true,
            timing_adjust: MS,
            ..smoothed(OS, OS + DUR, ms(1))
        },
    );
}

/// `pre_wraps` (characterized, not endorsed)
#[test]
fn steps_pre_wraps() {
    let mut input = media(u64::MAX - 5);
    check_pre(input, out(Keep, u64::MAX - 5, DUR - 6));
    input.duration = 0;
    check_pre(input, out(Keep, u64::MAX - 5, u64::MAX - 5));
    input.duration = DUR;
    input.next_audio_ts_min = 3;
    check_pre(
        input,
        PreLock {
            diff: u64::MAX - 8,
            timing_adjust: OS + 6,
            ..out(Jump, u64::MAX - 5, DUR - 6)
        },
    );
    input.timestamp = u64::MAX - 2 * MS;
    input.next_audio_ts_min = u64::MAX - MS;
    check_pre(input, smoothed(u64::MAX - MS, DUR - MS - 1, ms(1)));
    input.timestamp = TS;
    input.next_audio_ts_min = TS + SMOOTH - 1;
    check_pre(
        input,
        smoothed(
            TS + SMOOTH - 1,
            TS + SMOOTH - 1 + DUR,
            i64::try_from(SMOOTH).unwrap() - 1,
        ),
    );
    input.timestamp = 0;
    input.os_time = 1;
    input.next_audio_ts_min = 0;
    check_pre(
        input,
        PreLock {
            direct: true,
            ..out(Keep, 0, DUR)
        },
    );
}

// --------------------------------------------------------------------------
// obs_source_audio_timing_under_lock

fn under_out(timestamp: u64, timing_adjust: u64, next_audio_sys_ts_min: u64) -> UnderLock {
    UnderLock {
        timestamp,
        push_back: false,
        reset_timing: false,
        timing_adjust,
        next_audio_sys_ts_min,
        last_sync_offset: 0,
    }
}

fn pushed(timestamp: u64, timing_adjust: u64, next_audio_sys_ts_min: u64) -> UnderLock {
    UnderLock {
        push_back: true,
        ..under_out(timestamp, timing_adjust, next_audio_sys_ts_min)
    }
}

fn reset(timestamp: u64, timing_adjust: u64, next_audio_sys_ts_min: u64) -> UnderLock {
    UnderLock {
        reset_timing: true,
        ..under_out(timestamp, timing_adjust, next_audio_sys_ts_min)
    }
}

#[track_caller]
fn check_under(input: UnderLockInput, expected: UnderLock) {
    assert_eq!(under_lock(input), expected, "{input:?}");
}

/// A media source: timestamps from `TS`, `timing_adjust` = `OS - TS`.
fn media_packet(ts: u64) -> UnderLockInput {
    UnderLockInput {
        timestamp: ts,
        packet_timing_adjust: ADJ,
        data_timestamp: ts,
        os_time: OS,
        next_audio_ts_min: ts + DUR,
        timing_adjust: ADJ,
        ..UnderLockInput::default()
    }
}

/// `under_push_back_when_contiguous`
#[test]
fn steps_under_push_back_when_contiguous() {
    let mut input = media_packet(TS);
    input.next_audio_sys_ts_min = TS + ADJ;
    check_under(input, pushed(TS + ADJ, ADJ, TS + DUR + ADJ));
    input.continuous = true;
    check_under(input, pushed(TS + ADJ, ADJ, TS + DUR + ADJ));
}

/// `under_place_when_next_sys_is_zero`
#[test]
fn steps_under_place_when_next_sys_is_zero() {
    let mut input = media_packet(TS);
    check_under(input, under_out(TS + ADJ, ADJ, TS + DUR + ADJ));
    input.timestamp = TS + 100 * MAX_VAR;
    input.next_audio_ts_min = input.timestamp + DUR;
    check_under(
        input,
        under_out(
            TS + 100 * MAX_VAR + ADJ,
            ADJ,
            TS + 100 * MAX_VAR + DUR + ADJ,
        ),
    );
}

/// `under_at_smoothing_threshold`
#[test]
fn steps_under_at_smoothing_threshold() {
    let mut input = media_packet(TS);
    let sys = TS + ADJ;

    input.next_audio_sys_ts_min = sys + SMOOTH - 1;
    check_under(input, pushed(sys, ADJ, TS + DUR + ADJ));
    input.next_audio_sys_ts_min = sys - (SMOOTH - 1);
    check_under(input, pushed(sys, ADJ, TS + DUR + ADJ));
    input.next_audio_sys_ts_min = sys + SMOOTH;
    check_under(input, under_out(sys, ADJ, TS + DUR + ADJ));
    input.next_audio_sys_ts_min = sys - SMOOTH;
    check_under(input, under_out(sys, ADJ, TS + DUR + ADJ));
    input.next_audio_sys_ts_min = sys + SMOOTH + 1;
    check_under(input, under_out(sys, ADJ, TS + DUR + ADJ));
}

/// `under_reset_at_max_ts_var`
#[test]
fn steps_under_reset_at_max_ts_var() {
    let mut input = media_packet(TS);
    let sys = TS + ADJ;
    input.packet_timing_adjust = ADJ + 30000 * MS;
    input.timing_adjust = ADJ + 30000 * MS;
    input.os_time = OS + 7 * MS;
    let moved = sys + 30000 * MS;

    input.next_audio_sys_ts_min = moved + MAX_VAR;
    check_under(
        input,
        under_out(moved, ADJ + 30000 * MS, TS + DUR + ADJ + 30000 * MS),
    );
    input.next_audio_sys_ts_min = moved - MAX_VAR;
    check_under(
        input,
        under_out(moved, ADJ + 30000 * MS, TS + DUR + ADJ + 30000 * MS),
    );

    input.next_audio_sys_ts_min = moved + MAX_VAR + 1;
    check_under(
        input,
        reset(OS + 7 * MS, OS + 7 * MS - TS, TS + DUR + OS + 7 * MS - TS),
    );
    input.next_audio_sys_ts_min = moved - MAX_VAR - 1;
    check_under(
        input,
        reset(OS + 7 * MS, OS + 7 * MS - TS, TS + DUR + OS + 7 * MS - TS),
    );
}

/// `under_no_reset_when_continuous` (#522)
#[test]
fn steps_under_no_reset_when_continuous() {
    let mut input = media_packet(TS);
    input.continuous = true;
    input.packet_timing_adjust = ADJ + 4000 * MS;
    input.timing_adjust = ADJ + 4000 * MS;
    input.next_audio_sys_ts_min = TS + ADJ;
    check_under(
        input,
        under_out(
            TS + ADJ + 4000 * MS,
            ADJ + 4000 * MS,
            TS + DUR + ADJ + 4000 * MS,
        ),
    );
    input.continuous = false;
    input.timestamp = TS + 3 * MS;
    check_under(input, reset(OS, ADJ, TS + DUR + ADJ));
}

/// `under_offsets_after_the_comparison`
#[test]
fn steps_under_offsets_after_the_comparison() {
    let mut input = media_packet(TS);
    input.next_audio_sys_ts_min = TS + ADJ;
    input.sync_offset = ms(300);
    input.last_sync_offset = ms(300);
    input.resample_offset = MS;
    check_under(
        input,
        UnderLock {
            last_sync_offset: ms(300),
            ..pushed(TS + ADJ + 299 * MS, ADJ, TS + DUR + ADJ)
        },
    );
    input.sync_offset = ms(-300);
    input.last_sync_offset = ms(-300);
    check_under(
        input,
        UnderLock {
            last_sync_offset: ms(-300),
            ..pushed(TS + ADJ - 301 * MS, ADJ, TS + DUR + ADJ)
        },
    );
    input.next_audio_sys_ts_min = TS + ADJ + 3 * MAX_VAR;
    check_under(
        input,
        UnderLock {
            last_sync_offset: ms(-300),
            ..reset(OS - 301 * MS, ADJ, TS + DUR + ADJ)
        },
    );
}

/// `under_next_sys_uses_timing_adjust_under_lock`
#[test]
fn steps_under_next_sys_uses_timing_adjust_under_lock() {
    let mut input = media_packet(TS);
    input.next_audio_sys_ts_min = TS + ADJ;
    input.timing_adjust = ADJ + 33 * MS;
    check_under(
        input,
        pushed(TS + ADJ, ADJ + 33 * MS, TS + DUR + ADJ + 33 * MS),
    );
    input.next_audio_sys_ts_min = TS + ADJ + 3 * MAX_VAR;
    input.os_time = OS + 1;
    check_under(input, reset(OS + 1, ADJ + 1, TS + DUR + ADJ + 1));
}

/// `under_sync_offset_changed_places` (#333)
#[test]
fn steps_under_sync_offset_changed_places() {
    let mut input = media_packet(TS);
    input.next_audio_sys_ts_min = TS + ADJ;
    input.sync_offset = ms(300);
    check_under(
        input,
        UnderLock {
            last_sync_offset: ms(300),
            ..under_out(TS + ADJ + 300 * MS, ADJ, TS + DUR + ADJ)
        },
    );
    input.last_sync_offset = ms(300);
    input.sync_offset = ms(100);
    check_under(
        input,
        UnderLock {
            last_sync_offset: ms(100),
            ..under_out(TS + ADJ + 100 * MS, ADJ, TS + DUR + ADJ)
        },
    );
    input.last_sync_offset = ms(100);
    input.sync_offset = 0;
    check_under(input, under_out(TS + ADJ, ADJ, TS + DUR + ADJ));
    input.next_audio_sys_ts_min = TS + ADJ + 3 * MAX_VAR;
    input.last_sync_offset = 1;
    check_under(input, reset(OS, ADJ, TS + DUR + ADJ));
}

/// `under_negative_sync_offset_wraps` (characterized, not endorsed)
#[test]
fn steps_under_negative_sync_offset_wraps() {
    let mut input = UnderLockInput {
        timestamp: MS,
        data_timestamp: MS,
        os_time: 2 * MS,
        next_audio_ts_min: MS + DUR,
        sync_offset: ms(-5),
        last_sync_offset: ms(-5),
        ..UnderLockInput::default()
    };
    check_under(
        input,
        UnderLock {
            last_sync_offset: ms(-5),
            ..under_out(u64::MAX - 4 * MS + 1, 0, MS + DUR)
        },
    );
    input.sync_offset = i64::MIN;
    input.last_sync_offset = i64::MIN;
    check_under(
        input,
        UnderLock {
            last_sync_offset: i64::MIN,
            ..under_out(MS + 0x8000_0000_0000_0000, 0, MS + DUR)
        },
    );
    input.sync_offset = 0;
    input.last_sync_offset = 0;
    input.resample_offset = 2 * MS;
    check_under(input, under_out(u64::MAX - MS + 1, 0, MS + DUR));
}

/// `under_timestamp_plus_adjust_wraps` (characterized, not endorsed)
#[test]
fn steps_under_timestamp_plus_adjust_wraps() {
    let ts = OS + 10 * MAX_VAR;
    let adj = OS.wrapping_sub(ts);
    let mut input = UnderLockInput {
        timestamp: ts,
        packet_timing_adjust: adj,
        data_timestamp: ts,
        os_time: OS,
        next_audio_ts_min: ts + DUR,
        timing_adjust: adj,
        next_audio_sys_ts_min: OS,
        ..UnderLockInput::default()
    };
    check_under(input, pushed(OS, adj, OS + DUR));
    input.timestamp = u64::MAX;
    input.packet_timing_adjust = 2;
    input.next_audio_sys_ts_min = 1;
    check_under(input, pushed(1, adj, OS + DUR));
    input.sync_offset = i64::MAX;
    input.last_sync_offset = i64::MAX;
    check_under(
        input,
        UnderLock {
            last_sync_offset: i64::MAX,
            ..pushed(0x8000_0000_0000_0000, adj, OS + DUR)
        },
    );
    input.sync_offset = 0;
    input.last_sync_offset = 0;
    input.packet_timing_adjust = 0;
    check_under(input, reset(OS, adj, OS + DUR));
}

// --------------------------------------------------------------------------
// obs_source_audio_timing_buffer_start

/// `buffer_start` (#286)
#[test]
fn steps_buffer_start() {
    assert_eq!(buffer_start(OS, 0, 0), OS);
    assert_eq!(buffer_start(OS, ms(300), 0), OS + 300 * MS);
    assert_eq!(buffer_start(OS, ms(-300), MS), OS - 301 * MS);
    assert_eq!(buffer_start(0, ms(300), MS), 0);
    assert_eq!(buffer_start(0, i64::MIN, u64::MAX), 0);
    assert_eq!(buffer_start(1, -2, 0), u64::MAX);
    assert_eq!(buffer_start(1, 0, 2), u64::MAX);
    assert_eq!(
        buffer_start(u64::MAX, i64::MAX, 0),
        u64::try_from(i64::MAX).unwrap() - 1
    );
}

// --------------------------------------------------------------------------
// Packet sequences through a fake source

/// The fake source of the C test: the fields the steps read and write, and
/// the model of the rest of `source_output_audio_data()`.
#[derive(Debug, Default)]
struct Fake {
    timing_set: bool,
    timing_adjust: u64,
    next_audio_ts_min: u64,
    next_audio_sys_ts_min: u64,
    resample_offset: u64,
    sync_offset: i64,
    last_sync_offset: i64,
    async_unbuffered: bool,
    async_decoupled: bool,
    /// The buffer: where it starts and where its data ends, in ns.
    audio_ts: u64,
    buf_end: u64,
    /// The last packet.
    action: Option<Action>,
    pushed: bool,
    placed: u64,
    drift_error: i64,
}

impl Fake {
    /// `reset_audio_data()`: empty the buffer for packets at `os_time`.
    fn reset_audio_data(&mut self, os_time: u64) {
        self.audio_ts = os_time;
        self.buf_end = os_time;
        self.next_audio_sys_ts_min = os_time;
    }

    /// One packet of `duration` ns at `ts`, at system time `os_time`.
    fn packet(&mut self, ts: u64, os_time: u64, duration: u64) {
        let pre = pre_lock(PreLockInput {
            timestamp: ts,
            os_time,
            duration,
            next_audio_ts_min: self.next_audio_ts_min,
            timing_set: self.timing_set,
            async_unbuffered: self.async_unbuffered,
            async_decoupled: self.async_decoupled,
        });
        if pre.direct {
            self.timing_adjust = 0;
            self.timing_set = true;
        }
        match pre.action {
            Reset => {
                self.timing_set = true;
                self.timing_adjust = pre.timing_adjust;
            }
            Jump => {
                // handle_ts_jump(): reset_audio_timing(), then
                // reset_audio_data_at_sys_time()
                self.timing_set = true;
                self.timing_adjust = os_time.wrapping_sub(ts);
                self.reset_audio_data(os_time);
                self.audio_ts = buffer_start(os_time, self.sync_offset, self.resample_offset);
                self.buf_end = self.audio_ts;
            }
            Smooth => {
                if pre.reanchor {
                    self.timing_adjust = pre.timing_adjust;
                }
            }
            Keep | Exceeded => {}
        }
        self.action = Some(pre.action);
        self.drift_error = pre.drift_error;
        self.next_audio_ts_min = pre.next_audio_ts_min;

        let under = under_lock(UnderLockInput {
            timestamp: pre.timestamp,
            packet_timing_adjust: self.timing_adjust,
            data_timestamp: ts,
            os_time,
            next_audio_ts_min: self.next_audio_ts_min,
            resample_offset: self.resample_offset,
            sync_offset: self.sync_offset,
            timing_adjust: self.timing_adjust,
            next_audio_sys_ts_min: self.next_audio_sys_ts_min,
            last_sync_offset: self.last_sync_offset,
            continuous: pre.continuous,
        });
        if under.reset_timing {
            self.timing_set = true;
            self.timing_adjust = under.timing_adjust;
        }
        self.next_audio_sys_ts_min = under.next_audio_sys_ts_min;
        self.last_sync_offset = under.last_sync_offset;

        self.pushed = under.push_back && self.audio_ts != 0;
        if self.pushed {
            // source_output_audio_push_back()
            self.placed = self.buf_end;
        } else {
            // source_output_audio_place()
            if self.audio_ts == 0 || under.timestamp < self.audio_ts {
                self.reset_audio_data(under.timestamp);
            }
            self.placed = under.timestamp;
        }
        self.buf_end = self.placed.wrapping_add(duration);
    }

    #[track_caller]
    fn check(&self, action: Action, pushed: bool, placed: u64) {
        assert_eq!(
            (self.action, self.pushed, self.placed),
            (Some(action), pushed, placed),
            "{self:?}"
        );
    }
}

/// `seq_contiguous_packets`
#[test]
fn steps_seq_contiguous_packets() {
    let mut f = Fake::default();
    f.packet(TS, OS, DUR);
    f.check(Reset, false, OS);
    assert_eq!(f.timing_adjust, ADJ);
    for i in 1..5 {
        f.packet(TS + i * DUR, OS + i * DUR + MS, DUR);
        f.check(Smooth, true, OS + i * DUR);
        assert_eq!(f.drift_error, 0);
    }
    assert_eq!(f.next_audio_sys_ts_min, OS + 5 * DUR);
    f.packet(TS + 5 * DUR + 50 * MS, OS + 5 * DUR, DUR);
    f.check(Smooth, true, OS + 5 * DUR);
    assert_eq!(f.drift_error, ms(-50));
}

/// `seq_direct_timestamps`
#[test]
fn steps_seq_direct_timestamps() {
    let mut f = Fake::default();
    f.packet(OS - 10 * MS, OS, DUR);
    f.check(Keep, false, OS - 10 * MS);
    assert!(f.timing_set);
    assert_eq!(f.timing_adjust, 0);
    f.packet(OS - 10 * MS + DUR, OS + DUR, DUR);
    f.check(Smooth, true, OS - 10 * MS + DUR);
    f.packet(
        OS - 10 * MS + 2 * DUR + 100 * MS,
        OS + 2 * DUR + 100 * MS,
        DUR,
    );
    f.check(Exceeded, false, OS - 10 * MS + 2 * DUR + 100 * MS);
}

/// `seq_sync_offset_changed` (#333)
#[test]
fn steps_seq_sync_offset_changed() {
    let mut f = Fake::default();
    f.packet(TS, OS, DUR);
    f.packet(TS + DUR, OS + DUR, DUR);
    f.check(Smooth, true, OS + DUR);

    f.sync_offset = ms(300);
    f.packet(TS + 2 * DUR, OS + 2 * DUR, DUR);
    f.check(Smooth, false, OS + 2 * DUR + 300 * MS);
    assert_eq!(f.last_sync_offset, ms(300));
    f.packet(TS + 3 * DUR, OS + 3 * DUR, DUR);
    f.check(Smooth, true, OS + 3 * DUR + 300 * MS);

    f.sync_offset = ms(100);
    f.packet(TS + 4 * DUR, OS + 4 * DUR, DUR);
    f.check(Smooth, false, OS + 4 * DUR + 100 * MS);
    f.sync_offset = ms(-200);
    f.packet(TS + 5 * DUR, OS + 5 * DUR, DUR);
    f.check(Smooth, false, OS + 5 * DUR - 200 * MS);
    assert_eq!(f.next_audio_sys_ts_min, OS + 5 * DUR - 200 * MS);
    f.packet(TS + 6 * DUR, OS + 6 * DUR, DUR);
    f.check(Smooth, false, OS + 6 * DUR - 200 * MS);
    f.packet(TS + 7 * DUR, OS + 7 * DUR, DUR);
    f.check(Smooth, true, OS + 7 * DUR - 200 * MS);
}

/// `seq_jump_keeps_sync_offset` (#286)
#[test]
fn steps_seq_jump_keeps_sync_offset() {
    let mut f = Fake {
        sync_offset: ms(300),
        last_sync_offset: ms(300),
        ..Fake::default()
    };
    f.packet(TS, OS, DUR);
    f.check(Reset, false, OS + 300 * MS);
    f.packet(TS + DUR, OS + DUR, DUR);
    f.check(Smooth, false, OS + DUR + 300 * MS);
    f.packet(TS + 2 * DUR, OS + 2 * DUR, DUR);
    f.check(Smooth, true, OS + 2 * DUR + 300 * MS);
    f.packet(0, OS + 3 * DUR, DUR);
    f.check(Jump, true, OS + 3 * DUR + 300 * MS);
    assert_eq!(f.timing_adjust, OS + 3 * DUR);
    assert_eq!(f.audio_ts, OS + 3 * DUR + 300 * MS);
    f.packet(DUR, OS + 4 * DUR, DUR);
    f.check(Smooth, true, OS + 4 * DUR + 300 * MS);
}

/// `seq_async_delay_follows_timing` (#522)
#[test]
fn steps_seq_async_delay_follows_timing() {
    let mut f = Fake::default();
    f.packet(TS, OS, DUR);
    f.packet(TS + DUR, OS + DUR, DUR);
    f.check(Smooth, true, OS + DUR);
    f.timing_adjust += 4000 * MS;
    f.packet(TS + 2 * DUR, OS + 2 * DUR, DUR);
    f.check(Smooth, false, OS + 2 * DUR + 4000 * MS);
    assert!(f.timing_set);
    assert_eq!(f.timing_adjust, ADJ + 4000 * MS);
    f.packet(TS + 3 * DUR, OS + 3 * DUR, DUR);
    f.check(Smooth, true, OS + 3 * DUR + 4000 * MS);
    f.timing_adjust -= 4000 * MS;
    f.packet(TS + 4 * DUR + 100 * MS, OS + 4 * DUR, DUR);
    f.check(Exceeded, false, OS + 4 * DUR);
    assert_eq!(f.timing_adjust, OS + 4 * DUR - (TS + 4 * DUR + 100 * MS));
}

/// `seq_async_decoupled_reanchor`
#[test]
fn steps_seq_async_decoupled_reanchor() {
    let mut f = Fake {
        async_unbuffered: true,
        async_decoupled: true,
        ..Fake::default()
    };
    f.packet(TS, OS, DUR);
    f.check(Reset, false, OS);
    f.packet(TS + DUR, OS + DUR + 5 * MS, DUR);
    f.check(Smooth, true, OS + DUR);
    assert_eq!(f.timing_adjust, ADJ + 5 * MS);
    assert_eq!(f.next_audio_sys_ts_min, OS + 2 * DUR + 5 * MS);
    f.packet(TS + 2 * DUR, OS + 2 * DUR + 5 * MS, DUR);
    f.check(Smooth, true, OS + 2 * DUR);
}

/// `seq_next_audio_ts_min_zero`
#[test]
fn steps_seq_next_audio_ts_min_zero() {
    let mut f = Fake {
        timing_set: true,
        timing_adjust: ADJ,
        ..Fake::default()
    };
    f.packet(TS, OS, DUR);
    f.check(Keep, false, OS);
    f.packet(TS + DUR, OS + DUR, DUR);
    f.check(Smooth, true, OS + DUR);
    f.timing_set = false;
    f.packet(TS + 2 * DUR, OS + 3 * DUR, DUR);
    f.check(Reset, true, OS + 2 * DUR);
    assert_eq!(f.timing_adjust, OS + DUR - TS);
    f.timing_set = false;
    f.packet(TS + 3 * DUR, OS + 5 * DUR + 100 * MS, DUR);
    f.check(Reset, false, OS + 5 * DUR + 100 * MS);
}

// --------------------------------------------------------------------------
// New edge cases

/// Values at the edges of the thresholds and of the `u64` range.
const SPECIAL: [u64; 16] = [
    0,
    1,
    SMOOTH - 1,
    SMOOTH,
    SMOOTH + 1,
    MAX_VAR - 1,
    MAX_VAR,
    MAX_VAR + 1,
    TS,
    OS,
    1 << 63,
    (1 << 63) - 1,
    u64::MAX - MAX_VAR,
    u64::MAX - SMOOTH,
    u64::MAX - 1,
    u64::MAX,
];

const SPECIAL_I64: [i64; 9] = [
    0,
    1,
    -1,
    300_000_000,
    -300_000_000,
    i64::MAX,
    i64::MIN,
    i64::MAX - 1,
    i64::MIN + 1,
];

#[test]
fn edge_distance() {
    assert_eq!(distance(0, 0), 0);
    assert_eq!(distance(3, 5), 2);
    assert_eq!(distance(5, 3), 2);
    assert_eq!(distance(0, u64::MAX), u64::MAX);
    assert_eq!(distance(u64::MAX, 0), u64::MAX);
    assert_eq!(distance(u64::MAX, u64::MAX), 0);
    // not modular
    assert_eq!(distance(1, u64::MAX), u64::MAX - 1);
}

/// What each action implies about the outputs, for any input.
fn check_pre_invariants(input: PreLockInput) {
    let out = pre_lock(input);
    let direct = distance(input.timestamp, input.os_time) < MAX_TS_VAR;
    assert_eq!(out.direct, direct, "{input:?}");
    assert_eq!(
        out.next_audio_ts_min,
        out.timestamp.wrapping_add(input.duration),
        "{input:?}"
    );
    let reanchor_adjust = input.os_time.wrapping_sub(input.timestamp);
    match out.action {
        Keep => {
            assert!(input.timing_set || direct);
            assert_eq!(input.next_audio_ts_min, 0);
        }
        Reset => {
            assert!(!input.timing_set && !direct);
            assert_eq!(out.timing_adjust, reanchor_adjust);
        }
        Jump => {
            assert!(!direct);
            assert!(out.diff > MAX_TS_VAR);
            assert_eq!(out.timing_adjust, reanchor_adjust);
        }
        Smooth => {
            assert!(distance(input.next_audio_ts_min, input.timestamp) < SMOOTH);
            assert_eq!(out.timestamp, input.next_audio_ts_min);
            assert!(out.drift_error.unsigned_abs() < SMOOTH);
            assert_eq!(
                out.reanchor,
                input.async_unbuffered && input.async_decoupled
            );
            if out.reanchor {
                assert_eq!(out.timing_adjust, reanchor_adjust);
            }
        }
        Exceeded => {
            assert!(out.diff >= SMOOTH);
            assert!(out.diff <= MAX_TS_VAR || direct);
        }
    }
    if out.action != Smooth {
        assert_eq!(out.timestamp, input.timestamp, "{input:?}");
        assert_eq!(out.drift_error, 0);
        assert!(!out.continuous && !out.reanchor);
    } else {
        assert!(out.continuous);
    }
    if !matches!(out.action, Jump | Exceeded) {
        assert_eq!(out.diff, 0);
    }
    if !matches!(out.action, Reset | Jump) && !out.reanchor {
        assert_eq!(out.timing_adjust, 0);
    }
}

/// Every flag, with the timestamp, the clock, the expected timestamp and the
/// duration over the special values (and near each other): no panic, and
/// the outputs each action implies.
#[test]
fn edge_pre_lock_special_values() {
    for &timestamp in &SPECIAL {
        for &os_time in &SPECIAL {
            for delta in [0, 1, SMOOTH - 1, SMOOTH, MAX_VAR, MAX_VAR + 1] {
                for next in [
                    timestamp.wrapping_add(delta),
                    timestamp.wrapping_sub(delta),
                    0,
                ] {
                    for &duration in &[0, DUR, u64::MAX] {
                        for flags in 0..8u8 {
                            check_pre_invariants(PreLockInput {
                                timestamp,
                                os_time,
                                duration,
                                next_audio_ts_min: next,
                                timing_set: flags & 1 != 0,
                                async_unbuffered: flags & 2 != 0,
                                async_decoupled: flags & 4 != 0,
                            });
                        }
                    }
                }
            }
        }
    }
}

/// What [`under_lock`] implies about its outputs, for any input.
fn check_under_invariants(input: UnderLockInput) {
    let out = under_lock(input);
    let sys = input.timestamp.wrapping_add(input.packet_timing_adjust);
    let d = distance(input.next_audio_sys_ts_min, sys);
    let reset = input.next_audio_sys_ts_min != 0 && d > MAX_TS_VAR && !input.continuous;
    assert_eq!(out.reset_timing, reset, "{input:?}");
    let adjust = if reset {
        input.os_time.wrapping_sub(input.data_timestamp)
    } else {
        input.timing_adjust
    };
    assert_eq!(out.timing_adjust, adjust);
    let placed = if reset {
        input.data_timestamp.wrapping_add(adjust)
    } else {
        sys
    };
    assert_eq!(
        out.timestamp,
        placed
            .wrapping_add(input.sync_offset.cast_unsigned())
            .wrapping_sub(input.resample_offset)
    );
    assert_eq!(
        out.next_audio_sys_ts_min,
        input.next_audio_ts_min.wrapping_add(adjust)
    );
    assert_eq!(out.last_sync_offset, input.sync_offset);
    let contiguous =
        input.next_audio_sys_ts_min == sys || (input.next_audio_sys_ts_min != 0 && d < SMOOTH);
    assert_eq!(
        out.push_back,
        contiguous && input.last_sync_offset == input.sync_offset
    );
}

#[test]
fn edge_under_lock_special_values() {
    for &timestamp in &SPECIAL {
        for &adjust in &[0, 1, ADJ, u64::MAX, 1 << 63] {
            let sys = timestamp.wrapping_add(adjust);
            for delta in [0, 1, SMOOTH - 1, SMOOTH, MAX_VAR, MAX_VAR + 1] {
                for next_sys in [sys.wrapping_add(delta), sys.wrapping_sub(delta), 0] {
                    for &sync in &SPECIAL_I64 {
                        for last in [sync, 0, sync.wrapping_add(1)] {
                            for continuous in [false, true] {
                                for &other in &[0, OS, u64::MAX] {
                                    check_under_invariants(UnderLockInput {
                                        timestamp,
                                        packet_timing_adjust: adjust,
                                        data_timestamp: timestamp ^ other,
                                        os_time: other,
                                        next_audio_ts_min: timestamp.wrapping_add(DUR),
                                        resample_offset: other >> 40,
                                        sync_offset: sync,
                                        timing_adjust: adjust.wrapping_add(other),
                                        next_audio_sys_ts_min: next_sys,
                                        last_sync_offset: last,
                                        continuous,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn edge_buffer_start_special_values() {
    for &os_time in &SPECIAL {
        for &sync in &SPECIAL_I64 {
            for &resample in &SPECIAL {
                let start = buffer_start(os_time, sync, resample);
                if os_time == 0 {
                    assert_eq!(start, 0);
                } else {
                    assert_eq!(
                        start,
                        os_time
                            .wrapping_add(sync.cast_unsigned())
                            .wrapping_sub(resample)
                    );
                }
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4000))]

    #[test]
    fn edge_pre_lock_any_input(
        timestamp in any::<u64>(),
        os_time in any::<u64>(),
        duration in any::<u64>(),
        next in any::<u64>(),
        near in -200_000_000i64..200_000_000,
        use_near in any::<bool>(),
        flags in 0u8..8,
    ) {
        let next = if use_near { timestamp.wrapping_add_signed(near) } else { next };
        check_pre_invariants(PreLockInput {
            timestamp,
            os_time,
            duration,
            next_audio_ts_min: next,
            timing_set: flags & 1 != 0,
            async_unbuffered: flags & 2 != 0,
            async_decoupled: flags & 4 != 0,
        });
    }

    #[test]
    fn edge_under_lock_any_input(
        timestamp in any::<u64>(),
        packet_timing_adjust in any::<u64>(),
        data_timestamp in any::<u64>(),
        os_time in any::<u64>(),
        next_audio_ts_min in any::<u64>(),
        resample_offset in any::<u64>(),
        sync_offset in any::<i64>(),
        timing_adjust in any::<u64>(),
        next_sys in any::<u64>(),
        near in -3_000_000_000i64..3_000_000_000,
        use_near in any::<bool>(),
        last_same in any::<bool>(),
        last_sync_offset in any::<i64>(),
        continuous in any::<bool>(),
    ) {
        let sys = timestamp.wrapping_add(packet_timing_adjust);
        check_under_invariants(UnderLockInput {
            timestamp,
            packet_timing_adjust,
            data_timestamp,
            os_time,
            next_audio_ts_min,
            resample_offset,
            sync_offset,
            timing_adjust,
            next_audio_sys_ts_min: if use_near { sys.wrapping_add_signed(near) } else { next_sys },
            last_sync_offset: if last_same { sync_offset } else { last_sync_offset },
            continuous,
        });
    }
}
