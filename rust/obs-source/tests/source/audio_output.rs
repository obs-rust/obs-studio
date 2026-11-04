//! Tier 1 for `obs_source::audio_output`, the source audio output path of
//! `libobs/obs-source-audio-output.c.h`.
//!
//! The cases run the functions on a fake source that records every write and
//! call in order, and panics on a field or buffer touched without the lock the
//! C holds there, a lock taken twice, or the capture callback or a log line
//! with the lock held. `test_source_audio_output.c` pins the same path on the
//! real libobs.

use std::cell::{Cell, RefCell};

use obs_source::audio_output::{
    MAX_CHANNELS, Packet, Source, duration, output_audio_data, reset_audio_data,
    reset_audio_data_at_sys_time, reset_audio_timing,
};
use obs_source::audio_place::{MAX_BUF_SIZE, MONITOR_ONLY};
use proptest::prelude::*;

const RATE: u32 = 48_000;
const FRAMES: u32 = 1024;
const SIZE: usize = 4096;
/// `duration(RATE, FRAMES)`.
const DUR: u64 = 21_333_333;
/// The fake clock.
const NOW: u64 = 100_000_000_000;
/// A stamp far from the fake clock, as a media file's timeline is.
const FAR: u64 = 3_700_000_000_000;
const MSEC: u64 = 1_000_000;

#[derive(Default)]
struct Fake {
    log: RefCell<Vec<String>>,
    locked: Cell<bool>,
    timing_set: Cell<bool>,
    timing_adjust: Cell<u64>,
    next_audio_ts_min: Cell<u64>,
    next_audio_sys_ts_min: Cell<u64>,
    audio_ts: Cell<u64>,
    sync_offset: Cell<i64>,
    last_sync_offset: Cell<i64>,
    resample_offset: Cell<u64>,
    monitoring_type: Cell<i32>,
    async_unbuffered: Cell<bool>,
    async_decoupled: Cell<bool>,
    compensating: Cell<bool>,
    muted: Cell<bool>,
    sizes: RefCell<[usize; MAX_CHANNELS]>,
}

impl Fake {
    fn new() -> Self {
        Self::default()
    }

    fn rec(&self, text: impl Into<String>) {
        self.log.borrow_mut().push(text.into());
    }

    fn take(&self) -> Vec<String> {
        std::mem::take(&mut self.log.borrow_mut())
    }

    /// The buffer's sizes from a test, with no lock check.
    fn size(&self, channel: usize) -> usize {
        self.sizes.borrow()[channel]
    }

    fn set_size(&self, channel: usize, value: usize) {
        self.sizes.borrow_mut()[channel] = value;
    }

    fn guard(&self, what: &str) {
        assert!(self.locked.get(), "{what} without audio_buf_mutex");
    }

    fn free(&self, what: &str) {
        assert!(!self.locked.get(), "{what} with audio_buf_mutex held");
    }
}

impl Source for Fake {
    type Data = u32;

    fn now(&self) -> u64 {
        self.rec("now");
        NOW
    }
    fn sample_rate(&self) -> u32 {
        self.rec("rate");
        RATE
    }
    fn channels(&self) -> usize {
        self.rec("channels");
        2
    }
    fn lock_audio_buf(&self) {
        assert!(!self.locked.replace(true), "audio_buf_mutex locked twice");
        self.rec("lock");
    }
    fn unlock_audio_buf(&self) {
        assert!(self.locked.replace(false), "audio_buf_mutex not locked");
        self.rec("unlock");
    }
    fn timing_set(&self) -> bool {
        self.guard("timing_set");
        self.timing_set.get()
    }
    fn set_timing_set(&self, value: bool) {
        self.guard("timing_set");
        self.rec(format!("timing_set={value}"));
        self.timing_set.set(value);
    }
    fn timing_adjust(&self) -> u64 {
        self.guard("timing_adjust");
        self.rec("read adjust");
        self.timing_adjust.get()
    }
    fn set_timing_adjust(&self, value: u64) {
        self.guard("timing_adjust");
        self.rec(format!("adjust={value}"));
        self.timing_adjust.set(value);
    }
    fn next_audio_ts_min(&self) -> u64 {
        self.next_audio_ts_min.get()
    }
    fn set_next_audio_ts_min(&self, value: u64) {
        self.rec(format!("next_min={value}"));
        self.next_audio_ts_min.set(value);
    }
    fn next_audio_sys_ts_min(&self) -> u64 {
        self.guard("next_audio_sys_ts_min");
        self.next_audio_sys_ts_min.get()
    }
    fn set_next_audio_sys_ts_min(&self, value: u64) {
        self.guard("next_audio_sys_ts_min");
        self.rec(format!("next_sys={value}"));
        self.next_audio_sys_ts_min.set(value);
    }
    fn audio_ts(&self) -> u64 {
        self.guard("audio_ts");
        self.audio_ts.get()
    }
    fn set_audio_ts(&self, value: u64) {
        self.guard("audio_ts");
        self.rec(format!("audio_ts={value}"));
        self.audio_ts.set(value);
    }
    fn clear_last_buf_size(&self) {
        self.guard("last_audio_input_buf_size");
        self.rec("clear_last");
    }
    fn sync_offset(&self) -> i64 {
        self.guard("sync_offset");
        self.sync_offset.get()
    }
    fn last_sync_offset(&self) -> i64 {
        self.guard("last_sync_offset");
        self.last_sync_offset.get()
    }
    fn set_last_sync_offset(&self, value: i64) {
        self.guard("last_sync_offset");
        self.rec(format!("last_sync={value}"));
        self.last_sync_offset.set(value);
    }
    fn resample_offset(&self) -> u64 {
        self.guard("resample_offset");
        self.resample_offset.get()
    }
    fn monitoring_type(&self) -> i32 {
        self.guard("monitoring_type");
        self.monitoring_type.get()
    }
    fn async_unbuffered(&self) -> bool {
        self.async_unbuffered.get()
    }
    fn async_decoupled(&self) -> bool {
        self.async_decoupled.get()
    }
    fn compensating(&self) -> bool {
        self.compensating.get()
    }
    fn set_compensation_error(&self, error: i64) {
        self.rec(format!("error={error}"));
    }
    fn buf_size(&self, channel: usize) -> usize {
        self.guard("audio_input_buf");
        self.size(channel)
    }
    fn pop_front(&self, channel: usize, size: usize) {
        self.guard("audio_input_buf");
        self.rec(format!("pop_front {channel} {size}"));
        let now = self.size(channel);
        self.set_size(channel, now - size);
    }
    fn pop_back(&self, channel: usize, size: usize) {
        self.guard("audio_input_buf");
        self.rec(format!("pop_back {channel} {size}"));
        let now = self.size(channel);
        self.set_size(channel, now - size);
    }
    fn place(&self, channel: usize, placement: usize, data: u32, size: usize) {
        self.guard("audio_input_buf");
        self.rec(format!("place {channel} {placement} {data} {size}"));
        let now = self.size(channel);
        self.set_size(channel, now.max(placement + size));
    }
    fn push_back(&self, channel: usize, data: u32, size: usize) {
        self.guard("audio_input_buf");
        self.rec(format!("push {channel} {data} {size}"));
        let now = self.size(channel);
        self.set_size(channel, now + size);
    }
    fn log_ts_jump(&self, diff: u64, expected: u64, timestamp: u64) {
        self.free("the jump's log line");
        self.rec(format!("log jump {diff} {expected} {timestamp}"));
    }
    fn log_ts_exceeded(&self, diff: u64, expected: u64, timestamp: u64) {
        self.free("the exceeded log line");
        self.rec(format!("log exceeded {diff} {expected} {timestamp}"));
    }
    fn muted(&self, os_time: u64) -> bool {
        self.free("source_muted");
        self.rec(format!("muted {os_time}"));
        self.muted.get()
    }
    fn signal_audio_data(&self, data: u32, muted: bool) {
        self.free("the capture callback");
        self.rec(format!("signal {data} {muted}"));
    }
}

fn packet(timestamp: u64) -> Packet<u32> {
    Packet {
        data: 7,
        timestamp,
        frames: FRAMES,
    }
}

fn expect(fake: &Fake, lines: &[&str]) {
    assert_eq!(fake.take(), lines);
}

#[test]
fn duration_is_conv_frames_to_time() {
    assert_eq!(duration(RATE, FRAMES), DUR);
    assert_eq!(duration(48_000, 48_000), 1_000_000_000);
    assert_eq!(duration(0, FRAMES), 0);
    assert_eq!(duration(1, u32::MAX), u64::from(u32::MAX) * 1_000_000_000);
}

#[test]
fn the_first_packet_on_the_system_clock_is_placed_at_its_stamp() {
    let f = Fake::new();
    output_audio_data(&f, packet(NOW));
    let start = format!("audio_ts={NOW}");
    let sys = format!("next_sys={NOW}");
    let next_min = format!("next_min={}", NOW + DUR);
    let next_sys = format!("next_sys={}", NOW + DUR);
    expect(
        &f,
        &[
            "rate",
            "now",
            "lock",
            "adjust=0",
            "timing_set=true",
            "unlock",
            &next_min,
            "lock",
            "read adjust",
            "read adjust",
            &next_sys,
            "last_sync=0",
            "channels",
            "clear_last",
            &start,
            &sys,
            "rate",
            "place 0 0 7 4096",
            "pop_back 0 0",
            "place 1 0 7 4096",
            "pop_back 1 0",
            "clear_last",
            "unlock",
            &format!("muted {NOW}"),
            "signal 7 false",
        ],
    );
}

#[test]
fn a_continuing_packet_is_appended() {
    let f = Fake::new();
    f.timing_set.set(true);
    f.next_audio_ts_min.set(NOW + DUR);
    f.next_audio_sys_ts_min.set(NOW + DUR);
    f.audio_ts.set(NOW);
    f.set_size(0, SIZE);
    f.set_size(1, SIZE);

    // 1 ms late: smoothed onto the expected stamp
    output_audio_data(&f, packet(NOW + DUR + MSEC));

    let next_min = format!("next_min={}", NOW + 2 * DUR);
    let next_sys = format!("next_sys={}", NOW + 2 * DUR);
    expect(
        &f,
        &[
            "rate",
            "now",
            "lock",
            "adjust=0",
            "timing_set=true",
            "unlock",
            &next_min,
            "lock",
            "read adjust",
            "read adjust",
            &next_sys,
            "last_sync=0",
            "channels",
            "push 0 7 4096",
            "push 1 7 4096",
            "clear_last",
            "unlock",
            &format!("muted {NOW}"),
            "signal 7 false",
        ],
    );
    assert_eq!(f.size(0), 2 * SIZE);
    assert_eq!(f.audio_ts.get(), NOW);
}

#[test]
fn a_media_stamp_sets_the_timing() {
    let f = Fake::new();
    output_audio_data(&f, packet(FAR));
    let adjust = NOW.wrapping_sub(FAR);
    let log = f.take();
    let set = log.iter().position(|l| l == "timing_set=true").unwrap();
    assert_eq!(log[set + 1], format!("adjust={adjust}"));
    assert_eq!(log[set + 2], "unlock");
    assert_eq!(f.timing_adjust.get(), adjust);
    assert_eq!(f.audio_ts.get(), NOW);
    assert_eq!(f.next_audio_ts_min.get(), FAR + DUR);
}

#[test]
fn a_timestamp_jump_logs_then_resets_in_its_own_lock_region() {
    let f = Fake::new();
    f.timing_set.set(true);
    f.next_audio_ts_min.set(FAR);
    f.next_audio_sys_ts_min.set(NOW - 5);
    f.audio_ts.set(NOW - 100);
    f.sync_offset.set(50);
    f.last_sync_offset.set(50);
    f.resample_offset.set(8);
    f.set_size(0, SIZE);
    f.set_size(1, SIZE);

    let ts = FAR + 10_000_000_000;
    output_audio_data(&f, packet(ts));

    let adjust = NOW.wrapping_sub(ts);
    let log = f.take();
    let first = log.iter().position(|l| l == "unlock").unwrap();
    assert_eq!(
        log[first + 1..=first + 12],
        [
            format!("log jump 10000000000 {FAR} {ts}"),
            "lock".to_owned(),
            "timing_set=true".to_owned(),
            format!("adjust={adjust}"),
            "pop_front 0 4096".to_owned(),
            "pop_front 1 4096".to_owned(),
            "clear_last".to_owned(),
            format!("audio_ts={NOW}"),
            format!("next_sys={NOW}"),
            format!("audio_ts={}", NOW + 50 - 8),
            "unlock".to_owned(),
            format!("next_min={}", ts + DUR),
        ]
    );
    // the packet goes in the emptied buffer
    assert!(log.contains(&"push 0 7 4096".to_owned()));
    assert_eq!(f.size(0), SIZE);
    assert_eq!(f.audio_ts.get(), NOW + 42);
}

#[test]
fn past_the_threshold_logs_and_places_with_no_lock_held() {
    let f = Fake::new();
    f.timing_set.set(true);
    f.next_audio_ts_min.set(NOW);
    f.next_audio_sys_ts_min.set(NOW);
    f.audio_ts.set(NOW);

    let ts = NOW + 100 * MSEC;
    output_audio_data(&f, packet(ts));

    let log = f.take();
    assert!(log.contains(&format!("log exceeded {} {NOW} {ts}", 100 * MSEC)));
    // 100 ms at 48 kHz, in bytes
    assert!(log.contains(&"place 0 19200 7 4096".to_owned()));
    assert_eq!(f.size(1), 19200 + SIZE);
    assert_eq!(f.audio_ts.get(), NOW);
}

#[test]
fn a_packet_older_than_the_buffer_starts_it_again() {
    let f = Fake::new();
    f.timing_set.set(true);
    f.audio_ts.set(NOW);
    f.next_audio_ts_min.set(NOW + DUR);
    f.next_audio_sys_ts_min.set(NOW + DUR);
    f.set_size(0, SIZE);
    f.set_size(1, SIZE);

    let ts = NOW - 100 * MSEC;
    output_audio_data(&f, packet(ts));

    let log = f.take();
    assert!(log.contains(&"pop_front 0 4096".to_owned()));
    assert!(log.contains(&"place 0 0 7 4096".to_owned()));
    assert_eq!(f.audio_ts.get(), ts);
    assert_eq!(f.next_audio_sys_ts_min.get(), ts);
    assert_eq!(f.size(0), SIZE);
}

#[test]
fn a_changed_sync_offset_places_the_next_packet() {
    let f = Fake::new();
    f.timing_set.set(true);
    f.audio_ts.set(NOW);
    f.next_audio_ts_min.set(NOW + DUR);
    f.next_audio_sys_ts_min.set(NOW + DUR);
    f.sync_offset.set(30 * MSEC.cast_signed());
    f.set_size(0, SIZE);
    f.set_size(1, SIZE);

    output_audio_data(&f, packet(NOW + DUR));

    let log = f.take();
    assert!(log.iter().all(|l| !l.starts_with("push")));
    assert!(log.iter().any(|l| l.starts_with("place 0 ")));
    assert_eq!(f.last_sync_offset.get(), 30 * MSEC.cast_signed());
}

#[test]
fn a_monitor_only_source_buffers_nothing_and_still_signals() {
    let f = Fake::new();
    f.monitoring_type.set(MONITOR_ONLY);
    f.muted.set(true);

    output_audio_data(&f, packet(NOW));

    let log = f.take();
    assert!(!log.iter().any(|l| l.starts_with("place")));
    assert!(!log.iter().any(|l| l.starts_with("push")));
    assert_eq!(log.last().unwrap(), "signal 7 true");
    assert_eq!(f.next_audio_sys_ts_min.get(), NOW + DUR);
    assert_eq!(f.audio_ts.get(), 0);
}

#[test]
fn a_packet_past_the_maximum_is_dropped_on_both_paths() {
    // appended
    let f = Fake::new();
    f.timing_set.set(true);
    f.audio_ts.set(NOW);
    f.next_audio_ts_min.set(NOW + DUR);
    f.next_audio_sys_ts_min.set(NOW + DUR);
    f.set_size(0, MAX_BUF_SIZE);
    output_audio_data(&f, packet(NOW + DUR));
    let log = f.take();
    assert!(!log.iter().any(|l| l.starts_with("push")));
    assert!(!log.contains(&"clear_last".to_owned()));
    assert_eq!(log[log.len() - 3], "unlock");

    // placed
    let f = Fake::new();
    f.timing_set.set(true);
    f.audio_ts.set(1);
    f.next_audio_ts_min.set(NOW);
    output_audio_data(&f, packet(NOW));
    let log = f.take();
    assert!(!log.iter().any(|l| l.starts_with("place")));
    assert!(!log.contains(&"clear_last".to_owned()));
}

#[test]
fn a_moved_system_time_resets_a_discontinuous_packet() {
    let f = Fake::new();
    let adjust = NOW.wrapping_sub(FAR) + 3_000_000_000;
    f.timing_set.set(true);
    f.timing_adjust.set(adjust);
    f.next_audio_ts_min.set(FAR);
    f.next_audio_sys_ts_min.set(NOW);
    f.audio_ts.set(NOW);
    f.last_sync_offset.set(0);

    let ts = FAR + 100 * MSEC;
    output_audio_data(&f, packet(ts));

    let want = NOW.wrapping_sub(ts);
    let log = f.take();
    let at = log.iter().rposition(|l| l == "timing_set=true").unwrap();
    assert_eq!(log[at + 1], format!("adjust={want}"));
    assert_eq!(f.timing_adjust.get(), want);
    assert_eq!(
        f.next_audio_sys_ts_min.get(),
        ts.wrapping_add(DUR).wrapping_add(want)
    );
}

#[test]
fn a_moved_system_time_keeps_a_continuous_packet() {
    let f = Fake::new();
    let adjust = NOW.wrapping_sub(FAR) + 3_000_000_000;
    f.timing_set.set(true);
    f.timing_adjust.set(adjust);
    f.next_audio_ts_min.set(FAR);
    f.next_audio_sys_ts_min.set(NOW);
    f.audio_ts.set(NOW);

    output_audio_data(&f, packet(FAR));

    assert_eq!(f.timing_adjust.get(), adjust);
    let log = f.take();
    assert!(log.iter().any(|l| l.starts_with("place 0 ")));
    assert!(!log.iter().any(|l| l.starts_with("timing_set")));
}

#[test]
fn a_decoupled_unbuffered_source_reanchors_before_the_unlock() {
    let f = Fake::new();
    f.timing_set.set(true);
    f.async_unbuffered.set(true);
    f.async_decoupled.set(true);
    f.next_audio_ts_min.set(FAR);
    f.next_audio_sys_ts_min.set(NOW);
    f.audio_ts.set(NOW);

    let ts = FAR + MSEC;
    output_audio_data(&f, packet(ts));

    let want = NOW.wrapping_sub(ts);
    let log = f.take();
    let at = log
        .iter()
        .position(|l| *l == format!("adjust={want}"))
        .unwrap();
    assert_eq!(log[at + 1], "unlock");
    assert_eq!(f.next_audio_ts_min.get(), FAR + DUR);
}

#[test]
fn only_decoupled_or_only_unbuffered_does_not_reanchor() {
    for (unbuffered, decoupled) in [(true, false), (false, true)] {
        let f = Fake::new();
        f.timing_set.set(true);
        f.timing_adjust.set(5);
        f.async_unbuffered.set(unbuffered);
        f.async_decoupled.set(decoupled);
        f.next_audio_ts_min.set(FAR);
        f.next_audio_sys_ts_min.set(FAR + 5);
        f.audio_ts.set(NOW);
        output_audio_data(&f, packet(FAR + MSEC));
        assert_eq!(f.timing_adjust.get(), 5);
    }
}

#[test]
fn the_resampler_gets_the_error_after_the_jump_and_before_the_store() {
    let f = Fake::new();
    f.compensating.set(true);
    f.timing_set.set(true);
    f.next_audio_ts_min.set(FAR);
    f.next_audio_sys_ts_min.set(FAR);
    f.audio_ts.set(NOW);

    output_audio_data(&f, packet(FAR + MSEC));

    let log = f.take();
    let at = log.iter().position(|l| l.starts_with("error=")).unwrap();
    assert_eq!(log[at], format!("error=-{MSEC}"));
    assert!(log[at + 1].starts_with("next_min="));
    assert_eq!(log[at - 1], "unlock");

    // no resampler: no call
    let f = Fake::new();
    output_audio_data(&f, packet(NOW));
    assert!(!f.take().iter().any(|l| l.starts_with("error=")));
}

/// Moves `timing_adjust` at each unlock, as the graphics thread can.
struct Moves(Fake);
impl Source for Moves {
    type Data = u32;
    fn now(&self) -> u64 {
        self.0.now()
    }
    fn sample_rate(&self) -> u32 {
        self.0.sample_rate()
    }
    fn channels(&self) -> usize {
        self.0.channels()
    }
    fn lock_audio_buf(&self) {
        self.0.lock_audio_buf();
    }
    fn unlock_audio_buf(&self) {
        self.0.unlock_audio_buf();
        self.0.timing_adjust.set(1000);
    }
    fn timing_set(&self) -> bool {
        self.0.timing_set()
    }
    fn set_timing_set(&self, value: bool) {
        self.0.set_timing_set(value);
    }
    fn timing_adjust(&self) -> u64 {
        self.0.timing_adjust()
    }
    fn set_timing_adjust(&self, value: u64) {
        self.0.set_timing_adjust(value);
    }
    fn next_audio_ts_min(&self) -> u64 {
        self.0.next_audio_ts_min()
    }
    fn set_next_audio_ts_min(&self, value: u64) {
        self.0.set_next_audio_ts_min(value);
    }
    fn next_audio_sys_ts_min(&self) -> u64 {
        self.0.next_audio_sys_ts_min()
    }
    fn set_next_audio_sys_ts_min(&self, value: u64) {
        self.0.set_next_audio_sys_ts_min(value);
    }
    fn audio_ts(&self) -> u64 {
        self.0.audio_ts()
    }
    fn set_audio_ts(&self, value: u64) {
        self.0.set_audio_ts(value);
    }
    fn clear_last_buf_size(&self) {
        self.0.clear_last_buf_size();
    }
    fn sync_offset(&self) -> i64 {
        self.0.sync_offset()
    }
    fn last_sync_offset(&self) -> i64 {
        self.0.last_sync_offset()
    }
    fn set_last_sync_offset(&self, value: i64) {
        self.0.set_last_sync_offset(value);
    }
    fn resample_offset(&self) -> u64 {
        self.0.resample_offset()
    }
    fn monitoring_type(&self) -> i32 {
        self.0.monitoring_type()
    }
    fn async_unbuffered(&self) -> bool {
        self.0.async_unbuffered()
    }
    fn async_decoupled(&self) -> bool {
        self.0.async_decoupled()
    }
    fn compensating(&self) -> bool {
        self.0.compensating()
    }
    fn set_compensation_error(&self, error: i64) {
        self.0.set_compensation_error(error);
    }
    fn buf_size(&self, channel: usize) -> usize {
        self.0.buf_size(channel)
    }
    fn pop_front(&self, channel: usize, size: usize) {
        self.0.pop_front(channel, size);
    }
    fn pop_back(&self, channel: usize, size: usize) {
        self.0.pop_back(channel, size);
    }
    fn place(&self, channel: usize, placement: usize, data: u32, size: usize) {
        self.0.place(channel, placement, data, size);
    }
    fn push_back(&self, channel: usize, data: u32, size: usize) {
        self.0.push_back(channel, data, size);
    }
    fn log_ts_jump(&self, diff: u64, expected: u64, timestamp: u64) {
        self.0.log_ts_jump(diff, expected, timestamp);
    }
    fn log_ts_exceeded(&self, diff: u64, expected: u64, timestamp: u64) {
        self.0.log_ts_exceeded(diff, expected, timestamp);
    }
    fn muted(&self, os_time: u64) -> bool {
        self.0.muted(os_time)
    }
    fn signal_audio_data(&self, data: u32, muted: bool) {
        self.0.signal_audio_data(data, muted);
    }
}

/// A value the graphics thread sets between the regions is the one used.
#[test]
fn the_second_region_reads_the_timing_again() {
    let f = Moves(Fake::new());
    f.0.timing_set.set(true);
    f.0.next_audio_ts_min.set(FAR);
    f.0.audio_ts.set(FAR);
    output_audio_data(&f, packet(FAR));
    assert_eq!(f.0.next_audio_sys_ts_min.get(), FAR + DUR + 1000);
}

#[test]
fn reset_audio_timing_stores_the_flag_then_the_value() {
    let f = Fake::new();
    f.lock_audio_buf();
    reset_audio_timing(&f, 30, 10);
    f.unlock_audio_buf();
    let adjust = 10u64.wrapping_sub(30);
    expect(
        &f,
        &[
            "lock",
            "timing_set=true",
            &format!("adjust={adjust}"),
            "unlock",
        ],
    );
}

#[test]
fn reset_audio_data_pops_every_channel_with_data() {
    let f = Fake::new();
    f.set_size(0, 8);
    f.set_size(MAX_CHANNELS - 1, 16);
    f.lock_audio_buf();
    reset_audio_data(&f, 55);
    f.unlock_audio_buf();
    expect(
        &f,
        &[
            "lock",
            "pop_front 0 8",
            "pop_front 7 16",
            "clear_last",
            "audio_ts=55",
            "next_sys=55",
            "unlock",
        ],
    );
}

#[test]
fn reset_audio_data_at_sys_time_starts_the_buffer_with_the_offsets() {
    let f = Fake::new();
    f.sync_offset.set(-5);
    f.resample_offset.set(2);
    f.lock_audio_buf();
    reset_audio_data_at_sys_time(&f, 100);
    f.unlock_audio_buf();
    expect(
        &f,
        &[
            "lock",
            "clear_last",
            "audio_ts=100",
            "next_sys=100",
            "audio_ts=93",
            "unlock",
        ],
    );

    // 0 stays 0: a monitor-only source's preload passes 0
    let f = Fake::new();
    f.sync_offset.set(-5);
    f.lock_audio_buf();
    reset_audio_data_at_sys_time(&f, 0);
    f.unlock_audio_buf();
    assert_eq!(f.audio_ts.get(), 0);
}

proptest! {
    /// Any packet on any state keeps to the locks (the fake panics otherwise),
    /// ends unlocked, and always reaches the capture callback, last.
    #[test]
    fn any_packet_keeps_the_locks_and_signals_last(
        timestamp in any::<u64>(),
        frames in 0u32..4096,
        timing_set in any::<bool>(),
        next_min in any::<u64>(),
        next_sys in any::<u64>(),
        audio_ts in prop_oneof![Just(0u64), any::<u64>()],
        adjust in any::<u64>(),
        sync in any::<i64>(),
        last_sync in any::<i64>(),
        resample in any::<u64>(),
        monitoring in 0i32..3,
        unbuffered in any::<bool>(),
        decoupled in any::<bool>(),
        compensating in any::<bool>(),
        size in 0usize..=MAX_BUF_SIZE,
    ) {
        let f = Fake::new();
        f.timing_set.set(timing_set);
        f.next_audio_ts_min.set(next_min);
        f.next_audio_sys_ts_min.set(next_sys);
        f.audio_ts.set(audio_ts);
        f.timing_adjust.set(adjust);
        f.sync_offset.set(sync);
        f.last_sync_offset.set(last_sync);
        f.resample_offset.set(resample);
        f.monitoring_type.set(monitoring);
        f.async_unbuffered.set(unbuffered);
        f.async_decoupled.set(decoupled);
        f.compensating.set(compensating);
        f.set_size(0, size);
        f.set_size(1, size);

        output_audio_data(&f, Packet { data: 7, timestamp, frames });

        prop_assert!(!f.locked.get());
        let log = f.take();
        prop_assert!(log.last().unwrap().starts_with("signal 7 "));
        let locks = log.iter().filter(|l| *l == "lock").count();
        prop_assert!(locks == 2 || locks == 3);
    }
}
